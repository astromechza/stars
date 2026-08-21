use crate::store::Store;
use std::collections::HashMap;

impl Store {
    /// Advance a day's cell through the tri-state cycle and return the new state:
    /// 0 (cleared) -> 1 (outline) -> 2 (full) -> 0.
    /// State 0 is represented by the absence of a row.
    pub async fn cycle_day(
        &self,
        board_id: i64,
        year: i32,
        month: u32,
        day: u32,
    ) -> Result<u8, sqlx::Error> {
        let current: Option<i64> = sqlx::query_scalar(
            "SELECT state FROM toggles
             WHERE board_id = ? AND year = ? AND month = ? AND day = ?",
        )
        .bind(board_id)
        .bind(year)
        .bind(month as i64)
        .bind(day as i64)
        .fetch_optional(&self.pool)
        .await?;

        match current {
            None => {
                let now = chrono::Utc::now().to_rfc3339();
                sqlx::query(
                    "INSERT INTO toggles (board_id, year, month, day, toggled_at, state)
                     VALUES (?, ?, ?, ?, ?, 1)",
                )
                .bind(board_id)
                .bind(year)
                .bind(month as i64)
                .bind(day as i64)
                .bind(&now)
                .execute(&self.pool)
                .await?;
                Ok(1)
            }
            Some(1) => {
                sqlx::query(
                    "UPDATE toggles SET state = 2
                     WHERE board_id = ? AND year = ? AND month = ? AND day = ?",
                )
                .bind(board_id)
                .bind(year)
                .bind(month as i64)
                .bind(day as i64)
                .execute(&self.pool)
                .await?;
                Ok(2)
            }
            // State 2 (or any unexpected value) clears the cell.
            _ => {
                sqlx::query(
                    "DELETE FROM toggles
                     WHERE board_id = ? AND year = ? AND month = ? AND day = ?",
                )
                .bind(board_id)
                .bind(year)
                .bind(month as i64)
                .bind(day as i64)
                .execute(&self.pool)
                .await?;
                Ok(0)
            }
        }
    }

    /// Map of (month, day) -> state for every set cell in the given year.
    pub async fn toggled_days(
        &self,
        board_id: i64,
        year: i32,
    ) -> Result<HashMap<(u32, u32), u8>, sqlx::Error> {
        let rows: Vec<(i64, i64, i64)> =
            sqlx::query_as("SELECT month, day, state FROM toggles WHERE board_id = ? AND year = ?")
                .bind(board_id)
                .bind(year)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|(m, d, s)| ((m as u32, d as u32), s as u8))
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
    async fn cycle_advances_through_tri_state(pool: SqlitePool) {
        let store = Store { pool };
        let b = board(&store).await;

        assert_eq!(store.cycle_day(b, 2026, 3, 15).await.unwrap(), 1); // -> outline
        assert_eq!(store.cycle_day(b, 2026, 3, 15).await.unwrap(), 2); // -> full
        assert_eq!(store.cycle_day(b, 2026, 3, 15).await.unwrap(), 0); // -> cleared
        assert_eq!(store.cycle_day(b, 2026, 3, 15).await.unwrap(), 1); // wraps to outline
    }

    #[sqlx::test]
    async fn toggled_days_reports_state_and_scopes_by_year(pool: SqlitePool) {
        let store = Store { pool };
        let b = board(&store).await;
        store.cycle_day(b, 2026, 1, 1).await.unwrap(); // state 1
        store.cycle_day(b, 2026, 12, 31).await.unwrap(); // state 1
        store.cycle_day(b, 2026, 12, 31).await.unwrap(); // -> state 2
        store.cycle_day(b, 2025, 6, 1).await.unwrap(); // other year

        let map = store.toggled_days(b, 2026).await.unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&(1, 1)), Some(&1));
        assert_eq!(map.get(&(12, 31)), Some(&2));
        assert_eq!(map.get(&(6, 1)), None);
    }
}
