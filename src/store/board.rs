use crate::store::Store;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Board {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub created_at: String,
    pub archived_at: Option<String>,
    pub sort_order: i64,
}

impl Board {
    pub fn created_year(&self) -> i32 {
        self.created_at
            .get(0..4)
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| chrono::Utc::now().format("%Y").to_string().parse().unwrap())
    }

    pub fn year_bounds(&self, current_year: i32) -> (i32, i32) {
        (self.created_year(), current_year)
    }
}

impl Store {
    pub async fn list_boards(&self, user_id: i64) -> Result<Vec<Board>, sqlx::Error> {
        sqlx::query_as::<_, Board>(
            "SELECT id, user_id, name, created_at, archived_at, sort_order
             FROM boards
             WHERE user_id = ? AND archived_at IS NULL
             ORDER BY sort_order, id",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn create_board(&self, user_id: i64, name: &str) -> Result<Board, sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let next: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM boards WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO boards (user_id, name, created_at, archived_at, sort_order)
             VALUES (?, ?, ?, NULL, ?) RETURNING id",
        )
        .bind(user_id)
        .bind(name)
        .bind(&now)
        .bind(next)
        .fetch_one(&self.pool)
        .await?;

        Ok(self.get_board(user_id, id).await?.expect("just inserted"))
    }

    pub async fn get_board(
        &self,
        user_id: i64,
        board_id: i64,
    ) -> Result<Option<Board>, sqlx::Error> {
        sqlx::query_as::<_, Board>(
            "SELECT id, user_id, name, created_at, archived_at, sort_order
             FROM boards WHERE id = ? AND user_id = ?",
        )
        .bind(board_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn rename_board(
        &self,
        user_id: i64,
        board_id: i64,
        name: &str,
    ) -> Result<Option<Board>, sqlx::Error> {
        let rows = sqlx::query("UPDATE boards SET name = ? WHERE id = ? AND user_id = ?")
            .bind(name)
            .bind(board_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        if rows == 0 {
            return Ok(None);
        }
        self.get_board(user_id, board_id).await
    }

    pub async fn archive_board(&self, user_id: i64, board_id: i64) -> Result<bool, sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let rows = sqlx::query(
            "UPDATE boards SET archived_at = ?
             WHERE id = ? AND user_id = ? AND archived_at IS NULL",
        )
        .bind(&now)
        .bind(board_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(rows > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn user(store: &Store) -> i64 {
        store.upsert_user("s", None, None).await.unwrap().id
    }

    #[sqlx::test]
    async fn create_list_scopes_and_orders(pool: SqlitePool) {
        let store = Store { pool };
        let u = user(&store).await;
        let b1 = store.create_board(u, "Exercise").await.unwrap();
        let b2 = store.create_board(u, "Read").await.unwrap();
        assert!(b2.sort_order > b1.sort_order);

        let other = store.upsert_user("other", None, None).await.unwrap().id;
        store.create_board(other, "Theirs").await.unwrap();

        let mine = store.list_boards(u).await.unwrap();
        assert_eq!(mine.len(), 2);
        assert_eq!(mine[0].name, "Exercise");
    }

    #[sqlx::test]
    async fn cross_user_get_is_none(pool: SqlitePool) {
        let store = Store { pool };
        let u = user(&store).await;
        let b = store.create_board(u, "Mine").await.unwrap();
        let other = store.upsert_user("other", None, None).await.unwrap().id;
        assert!(store.get_board(other, b.id).await.unwrap().is_none());
        assert!(store.get_board(u, b.id).await.unwrap().is_some());
    }

    #[sqlx::test]
    async fn rename_and_archive(pool: SqlitePool) {
        let store = Store { pool };
        let u = user(&store).await;
        let b = store.create_board(u, "Old").await.unwrap();

        let r = store.rename_board(u, b.id, "New").await.unwrap().unwrap();
        assert_eq!(r.name, "New");

        assert!(store.archive_board(u, b.id).await.unwrap());
        assert_eq!(store.list_boards(u).await.unwrap().len(), 0);
        // still fetchable directly
        assert!(
            store
                .get_board(u, b.id)
                .await
                .unwrap()
                .unwrap()
                .archived_at
                .is_some()
        );
    }

    #[test]
    fn year_bounds_uses_created_year() {
        let b = Board {
            id: 1,
            user_id: 1,
            name: "x".into(),
            created_at: "2023-05-01T00:00:00+00:00".into(),
            archived_at: None,
            sort_order: 0,
        };
        assert_eq!(b.year_bounds(2026), (2023, 2026));
    }
}
