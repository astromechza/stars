use crate::store::Store;
use std::collections::HashSet;

impl Store {
    pub async fn toggle_day(
        &self,
        board_id: i64,
        year: i32,
        month: u32,
        day: u32,
    ) -> Result<bool, sqlx::Error> {
        let deleted = sqlx::query(
            "DELETE FROM toggles
             WHERE board_id = ? AND year = ? AND month = ? AND day = ?",
        )
        .bind(board_id)
        .bind(year)
        .bind(month as i64)
        .bind(day as i64)
        .execute(&self.pool)
        .await?
        .rows_affected();

        if deleted > 0 {
            return Ok(false);
        }

        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO toggles (board_id, year, month, day, toggled_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(board_id)
        .bind(year)
        .bind(month as i64)
        .bind(day as i64)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(true)
    }

    pub async fn toggled_days(
        &self,
        board_id: i64,
        year: i32,
    ) -> Result<HashSet<(u32, u32)>, sqlx::Error> {
        let rows: Vec<(i64, i64)> =
            sqlx::query_as("SELECT month, day FROM toggles WHERE board_id = ? AND year = ?")
                .bind(board_id)
                .bind(year)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|(m, d)| (m as u32, d as u32))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn board(store: &Store) -> i64 {
        let u = store.upsert_user("s", None, None).await.unwrap().id;
        store.create_board(u, "b").await.unwrap().id
    }

    #[sqlx::test]
    async fn toggle_is_idempotent_flip(pool: SqlitePool) {
        let store = Store { pool };
        let b = board(&store).await;

        assert!(store.toggle_day(b, 2026, 3, 15).await.unwrap()); // on
        assert!(!store.toggle_day(b, 2026, 3, 15).await.unwrap()); // off
        assert!(store.toggle_day(b, 2026, 3, 15).await.unwrap()); // on again
    }

    #[sqlx::test]
    async fn toggled_days_scoped_by_year(pool: SqlitePool) {
        let store = Store { pool };
        let b = board(&store).await;
        store.toggle_day(b, 2026, 1, 1).await.unwrap();
        store.toggle_day(b, 2026, 12, 31).await.unwrap();
        store.toggle_day(b, 2025, 6, 1).await.unwrap();

        let set: HashSet<(u32, u32)> = store.toggled_days(b, 2026).await.unwrap();
        assert_eq!(set.len(), 2);
        assert!(set.contains(&(1, 1)));
        assert!(set.contains(&(12, 31)));
        assert!(!set.contains(&(6, 1)));
    }
}
