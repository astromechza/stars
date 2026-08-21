use crate::store::Store;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub subject: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub created_at: String,
}

impl Store {
    pub async fn upsert_user(
        &self,
        subject: &str,
        email: Option<&str>,
        name: Option<&str>,
    ) -> Result<User, sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO users (subject, email, display_name, created_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(subject) DO UPDATE SET
                email = excluded.email,
                display_name = excluded.display_name",
        )
        .bind(subject)
        .bind(email)
        .bind(name)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, User>(
            "SELECT id, subject, email, display_name, created_at
             FROM users WHERE subject = ?",
        )
        .bind(subject)
        .fetch_one(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[sqlx::test]
    async fn upsert_creates_then_updates(pool: SqlitePool) {
        let store = Store { pool };
        let a = store
            .upsert_user("sub1", Some("a@x"), Some("A"))
            .await
            .unwrap();
        assert_eq!(a.subject, "sub1");
        assert_eq!(a.email.as_deref(), Some("a@x"));

        let b = store
            .upsert_user("sub1", Some("new@x"), Some("A2"))
            .await
            .unwrap();
        assert_eq!(a.id, b.id, "same subject reuses row");
        assert_eq!(b.email.as_deref(), Some("new@x"));
    }
}
