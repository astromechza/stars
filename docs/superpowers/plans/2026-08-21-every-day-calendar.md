# Every-Day Calendar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a SQLite-backed Rust web app where each user has multiple year-scoped "boards" (12-month grids of togglable day cells), served as HTMX fragments behind an Authelia forward-auth proxy.

**Architecture:** A single axum binary in layers — a pure `calendar` day-validity module, a `store` module owning all sqlx SQL, an `auth` middleware turning proxy headers into a `UserId`, and thin askama-rendering handlers. Static assets are embedded in the binary. Deployed as a small static-musl container to Kubernetes via a Helm chart, built by GitHub Actions into GHCR.

**Tech Stack:** Rust (2024 edition, latest stable), axum 0.8, sqlx 0.8 (SQLite, WAL), askama 0.12, HTMX, chrono, rust-embed, tower.

**Spec:** `docs/superpowers/specs/2026-08-21-every-day-calendar-design.md`

## Global Constraints

- Rust 2024 edition, latest stable toolchain.
- All timestamps stored as RFC3339 UTC strings (`chrono`).
- A toggle is row *presence*; untoggle *deletes* the row. No boolean column.
- Every board/toggle query is scoped `WHERE user_id = ?`; cross-user access returns `404`, never `403`.
- The app trusts identity headers unconditionally — it must only run behind the Authelia proxy. `DEV_USER` is for local dev only.
- Only valid calendar days are ever rendered or inserted (Feb 29 only in leap years).
- Year navigation lower bound = board creation year; upper bound = current calendar year.
- Lint clean: `cargo fmt --check` and `cargo clippy -- -D warnings` must pass.
- `#[sqlx::test]` fixtures use a fresh temp SQLite DB per test.

## File Structure

```
Cargo.toml
migrations/0001_init.sql
src/
  main.rs            -- bootstrap: config, store, router, listener
  config.rs          -- Config::from_env()
  calendar.rs        -- pure day-validity + month names (no deps on store)
  store/
    mod.rs           -- Store struct, connect(), re-exports
    user.rs          -- User model + upsert_user
    board.rs         -- Board model + CRUD, year_bounds
    toggle.rs        -- toggle_day, toggled_days
  auth.rs            -- UserId, HxRequest extractor, auth_middleware
  handlers/
    mod.rs           -- AppState, AppError (IntoResponse), router()
    board.rs         -- board page/grid, create/rename/archive
    toggle.rs        -- toggle handler
  templates.rs       -- askama template structs
  assets.rs          -- rust-embed + static handler
templates/
  layout.html  page.html  grid.html  cell.html  tabs.html  tab.html  empty.html
static/
  htmx.min.js  normalize.css  app.css
Dockerfile
.github/workflows/ci.yml
charts/stars/...     -- Helm chart
```

---

### Task 1: Project scaffold + calendar module

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `src/calendar.rs`
- Test: inline `#[cfg(test)]` in `src/calendar.rs`

**Interfaces:**
- Produces:
  - `calendar::is_leap_year(year: i32) -> bool`
  - `calendar::days_in_month(year: i32, month: u32) -> u32` — 0 if month not in 1..=12
  - `calendar::is_valid_day(year: i32, month: u32, day: u32) -> bool`
  - `calendar::MONTH_LABELS: [&str; 12]` — `["jan", ..., "dec"]`

- [ ] **Step 1: Create `Cargo.toml`**

```toml
[package]
name = "stars"
version = "0.1.0"
edition = "2024"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }
tower = "0.5"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "macros", "migrate"] }
askama = "0.12"
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }
rust-embed = "8"
serde = { version = "1", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
mime_guess = "2"

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

- [ ] **Step 2: Write failing tests in `src/calendar.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_years() {
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2023));
    }

    #[test]
    fn days_per_month() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2024, 4), 30);
        assert_eq!(days_in_month(2024, 1), 31);
        assert_eq!(days_in_month(2024, 13), 0);
        assert_eq!(days_in_month(2024, 0), 0);
    }

    #[test]
    fn valid_days() {
        assert!(is_valid_day(2024, 2, 29));
        assert!(!is_valid_day(2023, 2, 29));
        assert!(!is_valid_day(2024, 4, 31));
        assert!(is_valid_day(2024, 12, 31));
        assert!(!is_valid_day(2024, 1, 0));
    }
}
```

- [ ] **Step 3: Run to verify fail**

Run: `cargo test --lib calendar`
Expected: FAIL — functions not defined (or compile error).

- [ ] **Step 4: Implement `src/calendar.rs`**

```rust
pub const MONTH_LABELS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun",
    "jul", "aug", "sep", "oct", "nov", "dec",
];

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap_year(year) { 29 } else { 28 },
        _ => 0,
    }
}

pub fn is_valid_day(year: i32, month: u32, day: u32) -> bool {
    day >= 1 && day <= days_in_month(year, month)
}
```

- [ ] **Step 5: Create minimal `src/main.rs` so the crate builds**

```rust
mod calendar;

fn main() {
    println!("stars");
}
```

- [ ] **Step 6: Run tests + lint**

Run: `cargo test --lib calendar && cargo fmt --check && cargo clippy -- -D warnings`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/
git commit -m "feat: scaffold crate and calendar day-validity module"
```

---

### Task 2: Config module

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs` (add `mod config;`)
- Test: inline `#[cfg(test)]` in `src/config.rs`

**Interfaces:**
- Produces:
  - `struct Config { pub database_url: String, pub bind_addr: String, pub dev_user: Option<String> }`
  - `Config::from_env() -> Config` — reads `DATABASE_URL` (default `sqlite://stars.db`), `BIND_ADDR` (default `0.0.0.0:8080`), `DEV_USER` (optional).

- [ ] **Step 1: Write failing test in `src/config.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_from_map() {
        let cfg = Config::from_map(|_| None);
        assert_eq!(cfg.database_url, "sqlite://stars.db");
        assert_eq!(cfg.bind_addr, "0.0.0.0:8080");
        assert!(cfg.dev_user.is_none());
    }

    #[test]
    fn overrides_from_map() {
        let cfg = Config::from_map(|k| match k {
            "DATABASE_URL" => Some("sqlite:///data/x.db".into()),
            "DEV_USER" => Some("alice".into()),
            _ => None,
        });
        assert_eq!(cfg.database_url, "sqlite:///data/x.db");
        assert_eq!(cfg.dev_user.as_deref(), Some("alice"));
    }
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test --lib config`
Expected: FAIL — `Config` not defined.

- [ ] **Step 3: Implement `src/config.rs`**

```rust
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub dev_user: Option<String>,
}

impl Config {
    pub fn from_env() -> Config {
        Config::from_map(|k| std::env::var(k).ok())
    }

    // Injectable for tests.
    pub fn from_map(get: impl Fn(&str) -> Option<String>) -> Config {
        Config {
            database_url: get("DATABASE_URL")
                .unwrap_or_else(|| "sqlite://stars.db".to_string()),
            bind_addr: get("BIND_ADDR")
                .unwrap_or_else(|| "0.0.0.0:8080".to_string()),
            dev_user: get("DEV_USER").filter(|s| !s.is_empty()),
        }
    }
}
```

- [ ] **Step 4: Add `mod config;` to `src/main.rs`**

Add the line `mod config;` beside `mod calendar;`.

- [ ] **Step 5: Run tests + lint**

Run: `cargo test --lib config && cargo clippy -- -D warnings`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: add env-driven config"
```

---

### Task 3: Store connect + migrations + user upsert

**Files:**
- Create: `migrations/0001_init.sql`, `src/store/mod.rs`, `src/store/user.rs`
- Modify: `src/main.rs` (add `mod store;`)
- Test: inline `#[cfg(test)]` in `src/store/user.rs`

**Interfaces:**
- Produces:
  - `store::Store { pub pool: sqlx::SqlitePool }`
  - `Store::connect(database_url: &str) -> Result<Store, sqlx::Error>` — connects, enables WAL + foreign keys, runs migrations.
  - `store::User { pub id: i64, pub subject: String, pub email: Option<String>, pub display_name: Option<String>, pub created_at: String }`
  - `Store::upsert_user(&self, subject: &str, email: Option<&str>, name: Option<&str>) -> Result<User, sqlx::Error>`

- [ ] **Step 1: Create `migrations/0001_init.sql`**

```sql
CREATE TABLE users (
    id           INTEGER PRIMARY KEY,
    subject      TEXT NOT NULL UNIQUE,
    email        TEXT,
    display_name TEXT,
    created_at   TEXT NOT NULL
);

CREATE TABLE boards (
    id          INTEGER PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id),
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    archived_at TEXT,
    sort_order  INTEGER NOT NULL
);
CREATE INDEX idx_boards_user ON boards(user_id);

CREATE TABLE toggles (
    board_id   INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    year       INTEGER NOT NULL,
    month      INTEGER NOT NULL,
    day        INTEGER NOT NULL,
    toggled_at TEXT NOT NULL,
    PRIMARY KEY (board_id, year, month, day)
);
```

- [ ] **Step 2: Write `src/store/mod.rs` skeleton**

```rust
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

pub mod user;

pub use user::User;

#[derive(Clone)]
pub struct Store {
    pub pool: SqlitePool,
}

impl Store {
    pub async fn connect(database_url: &str) -> Result<Store, sqlx::Error> {
        let opts = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new().connect_with(opts).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Store { pool })
    }
}
```

- [ ] **Step 3: Write failing test in `src/store/user.rs`**

```rust
use crate::store::Store;
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub subject: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn upsert_creates_then_updates(pool: SqlitePool) {
        let store = Store { pool };
        let a = store.upsert_user("sub1", Some("a@x"), Some("A")).await.unwrap();
        assert_eq!(a.subject, "sub1");
        assert_eq!(a.email.as_deref(), Some("a@x"));

        let b = store.upsert_user("sub1", Some("new@x"), Some("A2")).await.unwrap();
        assert_eq!(a.id, b.id, "same subject reuses row");
        assert_eq!(b.email.as_deref(), Some("new@x"));
    }
}
```

Note: `#[sqlx::test]` requires migrations to be discoverable; it auto-applies `./migrations` against a fresh temp DB. Because the temp DB is created by sqlx (not via `Store::connect`), this test exercises `upsert_user` directly on the injected pool.

- [ ] **Step 4: Run to verify fail**

Run: `cargo test --lib store::user`
Expected: FAIL — `upsert_user` not defined.

- [ ] **Step 5: Implement `upsert_user` in `src/store/user.rs`**

Append to the file (after the `User` struct):

```rust
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
```

- [ ] **Step 6: Add `mod store;` to `src/main.rs`; run tests + lint**

Run: `cargo test --lib store && cargo clippy -- -D warnings`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add migrations/ src/store/ src/main.rs
git commit -m "feat: store connect, migrations, user upsert"
```

---

### Task 4: Board CRUD + year bounds

**Files:**
- Create: `src/store/board.rs`
- Modify: `src/store/mod.rs` (add `pub mod board;` + `pub use board::Board;`)
- Test: inline `#[cfg(test)]` in `src/store/board.rs`

**Interfaces:**
- Consumes: `Store`, `User` (Task 3).
- Produces:
  - `store::Board { pub id: i64, pub user_id: i64, pub name: String, pub created_at: String, pub archived_at: Option<String>, pub sort_order: i64 }`
  - `Board::created_year(&self) -> i32` — parses `created_at` year (fallback to current year on parse failure).
  - `Board::year_bounds(&self, current_year: i32) -> (i32, i32)` — `(created_year, current_year)`.
  - `Store::list_boards(&self, user_id: i64) -> Result<Vec<Board>, sqlx::Error>` — active only, ordered by `sort_order, id`.
  - `Store::create_board(&self, user_id: i64, name: &str) -> Result<Board, sqlx::Error>` — `sort_order` = current max+1.
  - `Store::get_board(&self, user_id: i64, board_id: i64) -> Result<Option<Board>, sqlx::Error>` — scoped, active or archived.
  - `Store::rename_board(&self, user_id: i64, board_id: i64, name: &str) -> Result<Option<Board>, sqlx::Error>`.
  - `Store::archive_board(&self, user_id: i64, board_id: i64) -> Result<bool, sqlx::Error>`.

- [ ] **Step 1: Write failing tests in `src/store/board.rs`**

```rust
use crate::store::Store;
use sqlx::SqlitePool;

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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(store.get_board(u, b.id).await.unwrap().unwrap().archived_at.is_some());
    }

    #[test]
    fn year_bounds_uses_created_year() {
        let b = Board {
            id: 1, user_id: 1, name: "x".into(),
            created_at: "2023-05-01T00:00:00+00:00".into(),
            archived_at: None, sort_order: 0,
        };
        assert_eq!(b.year_bounds(2026), (2023, 2026));
    }
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test --lib store::board`
Expected: FAIL — `create_board` etc. not defined.

- [ ] **Step 3: Implement CRUD in `src/store/board.rs`**

Append after the `Board` impl:

```rust
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

    pub async fn archive_board(
        &self,
        user_id: i64,
        board_id: i64,
    ) -> Result<bool, sqlx::Error> {
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
```

- [ ] **Step 4: Wire module in `src/store/mod.rs`**

Add `pub mod board;` and `pub use board::Board;` beside the user lines.

- [ ] **Step 5: Run tests + lint**

Run: `cargo test --lib store::board && cargo clippy -- -D warnings`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/store/
git commit -m "feat: board CRUD, scoping, archive, year bounds"
```

---

### Task 5: Toggle store logic

**Files:**
- Create: `src/store/toggle.rs`
- Modify: `src/store/mod.rs` (add `pub mod toggle;`)
- Test: inline `#[cfg(test)]` in `src/store/toggle.rs`

**Interfaces:**
- Consumes: `Store`, `create_board` (Task 4).
- Produces:
  - `Store::toggle_day(&self, board_id: i64, year: i32, month: u32, day: u32) -> Result<bool, sqlx::Error>` — flips presence; returns the new state (`true` = now on).
  - `Store::toggled_days(&self, board_id: i64, year: i32) -> Result<std::collections::HashSet<(u32, u32)>, sqlx::Error>` — set of `(month, day)` toggled in that year.

- [ ] **Step 1: Write failing tests in `src/store/toggle.rs`**

```rust
use crate::store::Store;
use sqlx::SqlitePool;
use std::collections::HashSet;

#[cfg(test)]
mod tests {
    use super::*;

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
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test --lib store::toggle`
Expected: FAIL — `toggle_day` not defined.

- [ ] **Step 3: Implement `src/store/toggle.rs`**

Prepend the real implementation above the test module:

```rust
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
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT month, day FROM toggles WHERE board_id = ? AND year = ?",
        )
        .bind(board_id)
        .bind(year)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(m, d)| (m as u32, d as u32)).collect())
    }
}
```

- [ ] **Step 4: Wire module + run tests + lint**

Add `pub mod toggle;` to `src/store/mod.rs`.
Run: `cargo test --lib store::toggle && cargo clippy -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/store/
git commit -m "feat: toggle store logic with per-year query"
```

---

### Task 6: Auth middleware + HxRequest extractor

**Files:**
- Create: `src/auth.rs`
- Modify: `src/main.rs` (add `mod auth;`)
- Test: inline `#[cfg(test)]` in `src/auth.rs`

**Interfaces:**
- Consumes: `Store::upsert_user` (Task 3).
- Produces:
  - `auth::UserId(pub i64)` — request extension; implements `FromRequestParts` (reads the extension, `401` if absent).
  - `auth::HxRequest(pub bool)` — extractor reading the `HX-Request` header.
  - `auth::auth_middleware(State<AppState>, Request, Next) -> Response` — extracts headers or `DEV_USER`, upserts user, inserts `UserId` extension; `401` when unauthenticated.
- Note: `AppState` is defined in Task 7 (`handlers::mod`). Task 6 references it via `crate::handlers::AppState`; implement `AppState` first as a minimal struct if Task 7 is not yet done, or sequence Task 7 before running the middleware test. To keep tasks independent, this task defines the extractors and a pure header-parsing helper with tests; the middleware is wired in Task 7's router test.

- [ ] **Step 1: Write failing tests in `src/auth.rs`**

```rust
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header::HeaderMap, StatusCode};

#[derive(Clone, Copy, Debug)]
pub struct UserId(pub i64);

#[derive(Clone, Copy, Debug)]
pub struct HxRequest(pub bool);

/// Identity resolved from proxy headers, or the dev fallback.
#[derive(Debug, PartialEq, Eq)]
pub struct Identity {
    pub subject: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

/// Pure resolution: proxy headers win; else dev_user; else None (=> 401).
pub fn resolve_identity(headers: &HeaderMap, dev_user: Option<&str>) -> Option<Identity> {
    let get = |k: &str| headers.get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
    if let Some(subject) = get("Remote-User").filter(|s| !s.is_empty()) {
        return Some(Identity {
            subject,
            email: get("Remote-Email"),
            name: get("Remote-Name"),
        });
    }
    dev_user.filter(|s| !s.is_empty()).map(|d| Identity {
        subject: d.to_string(),
        email: None,
        name: Some(d.to_string()),
    })
}

pub fn is_hx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn hm(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn proxy_headers_win() {
        let id = resolve_identity(&hm(&[("Remote-User", "sub"), ("Remote-Email", "e@x")]), Some("dev"));
        assert_eq!(id.unwrap().subject, "sub");
    }

    #[test]
    fn dev_fallback_when_no_headers() {
        let id = resolve_identity(&hm(&[]), Some("dev")).unwrap();
        assert_eq!(id.subject, "dev");
    }

    #[test]
    fn none_when_unauthenticated() {
        assert!(resolve_identity(&hm(&[]), None).is_none());
    }

    #[test]
    fn hx_header_detection() {
        assert!(is_hx(&hm(&[("HX-Request", "true")])));
        assert!(!is_hx(&hm(&[])));
    }
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test --lib auth`
Expected: FAIL — module not wired / functions not defined.

- [ ] **Step 3: Implement the extractors + middleware**

Append to `src/auth.rs`:

```rust
use axum::extract::State;
use axum::middleware::Next;
use axum::response::Response;
use axum::http::Request;

impl<S: Send + Sync> FromRequestParts<S> for HxRequest {
    type Rejection = std::convert::Infallible;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(HxRequest(is_hx(&parts.headers)))
    }
}

impl<S: Send + Sync> FromRequestParts<S> for UserId {
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<UserId>()
            .copied()
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

pub async fn auth_middleware(
    State(state): State<crate::handlers::AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let identity = resolve_identity(req.headers(), state.dev_user.as_deref());
    let Some(identity) = identity else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    match state
        .store
        .upsert_user(&identity.subject, identity.email.as_deref(), identity.name.as_deref())
        .await
    {
        Ok(user) => {
            req.extensions_mut().insert(UserId(user.id));
            next.run(req).await
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
```

Add the needed `use axum::response::IntoResponse;` import at the top.

- [ ] **Step 4: Add `mod auth;` to `src/main.rs`**

The middleware references `crate::handlers::AppState`, so this task compiles only after Task 7 creates `handlers::mod`. Sequence Task 7 immediately after; run this task's *unit* tests (pure functions) now, which do not need `AppState`:

Run: `cargo test --lib auth::tests`
Expected: the four pure-function tests PASS. (Full-crate build completes after Task 7.)

- [ ] **Step 5: Commit**

```bash
git add src/auth.rs src/main.rs
git commit -m "feat: auth identity resolution, HxRequest and UserId extractors"
```

---

### Task 7: AppState, error type, templates, board page + grid handler

**Files:**
- Create: `src/handlers/mod.rs`, `src/handlers/board.rs`, `src/templates.rs`, `templates/layout.html`, `templates/page.html`, `templates/grid.html`, `templates/empty.html`, `templates/tabs.html`
- Modify: `src/main.rs` (add `mod handlers; mod templates;`)
- Test: inline `#[cfg(test)]` in `src/handlers/board.rs`

**Interfaces:**
- Consumes: `Store` (Tasks 3-5), `auth::{UserId, HxRequest, auth_middleware}` (Task 6), `calendar` (Task 1).
- Produces:
  - `handlers::AppState { pub store: Store, pub dev_user: Option<String> }`
  - `handlers::AppError` implementing `IntoResponse` — variants `NotFound`, `BadRequest`, `Internal(sqlx::Error)`.
  - `handlers::router(state: AppState) -> axum::Router` — all routes + auth middleware layer.
  - `templates::{PageTemplate, GridTemplate, EmptyTemplate}` + a `GridView` helper describing months/days/toggled state.
  - `handlers::board::show_board` — `GET /boards/:id` (full page or grid fragment by `HxRequest`).

- [ ] **Step 1: Create templates**

`templates/layout.html`:

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>stars</title>
  <link rel="stylesheet" href="/static/normalize.css">
  <link rel="stylesheet" href="/static/app.css">
  <script src="/static/htmx.min.js" defer></script>
</head>
<body>
  <main class="app">{% block content %}{% endblock %}</main>
</body>
</html>
```

`templates/tabs.html`:

```html
<nav class="tabs">
  {% for t in tabs %}
    <a class="tab {% if t.id == active_id %}tab--active{% endif %}"
       href="/boards/{{ t.id }}">{{ t.name }}</a>
  {% endfor %}
  <form class="tab-new" method="post" action="/boards">
    <button type="submit" title="New board">+</button>
  </form>
</nav>
```

`templates/grid.html`:

```html
<div id="grid" class="grid"
     style="--cols: 12; --rows: {{ max_days }};">
  <div class="grid__nav">
    <button {% if year <= min_year %}disabled{% endif %}
      hx-get="/boards/{{ board_id }}?year={{ year - 1 }}"
      hx-target="#grid" hx-swap="outerHTML">&lt;</button>
    <span class="grid__year">{{ year }}</span>
    <button {% if year >= max_year %}disabled{% endif %}
      hx-get="/boards/{{ board_id }}?year={{ year + 1 }}"
      hx-target="#grid" hx-swap="outerHTML">&gt;</button>
  </div>
  <div class="grid__cols">
    {% for col in columns %}
    <div class="col">
      <div class="col__label">{{ col.label }}</div>
      {% for cell in col.cells %}
        {% if cell.valid %}
          <button class="cell {% if cell.on %}cell--on{% endif %}"
            hx-post="/boards/{{ board_id }}/toggle"
            hx-vals='{"year": {{ year }}, "month": {{ col.month }}, "day": {{ cell.day }}}'
            hx-swap="outerHTML">{{ cell.day }}</button>
        {% else %}
          <span class="cell cell--empty"></span>
        {% endif %}
      {% endfor %}
    </div>
    {% endfor %}
  </div>
</div>
```

`templates/page.html`:

```html
{% extends "layout.html" %}
{% block content %}
  {% include "tabs.html" %}
  <h1 class="board-title">{{ board_name }}</h1>
  {{ grid_html|safe }}
{% endblock %}
```

`templates/empty.html`:

```html
{% extends "layout.html" %}
{% block content %}
  {% include "tabs.html" %}
  <section class="empty">
    <p>No boards yet.</p>
    <form method="post" action="/boards"><button>Create your first board</button></form>
  </section>
{% endblock %}
```

- [ ] **Step 2: Create `src/templates.rs`**

```rust
use askama::Template;

#[derive(Clone)]
pub struct TabView {
    pub id: i64,
    pub name: String,
}

pub struct Cell {
    pub day: u32,
    pub valid: bool,
    pub on: bool,
}

pub struct Column {
    pub month: u32,
    pub label: &'static str,
    pub cells: Vec<Cell>,
}

#[derive(Template)]
#[template(path = "grid.html")]
pub struct GridTemplate {
    pub board_id: i64,
    pub year: i32,
    pub min_year: i32,
    pub max_year: i32,
    pub max_days: u32,
    pub columns: Vec<Column>,
}

#[derive(Template)]
#[template(path = "page.html")]
pub struct PageTemplate {
    pub tabs: Vec<TabView>,
    pub active_id: i64,
    pub board_name: String,
    pub grid_html: String,
}

#[derive(Template)]
#[template(path = "empty.html")]
pub struct EmptyTemplate {
    pub tabs: Vec<TabView>,
    pub active_id: i64,
}
```

- [ ] **Step 3: Create `src/handlers/mod.rs` (AppState, AppError, router)**

```rust
use crate::auth::auth_middleware;
use crate::store::Store;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{middleware, Router};

pub mod board;
pub mod toggle;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub dev_user: Option<String>,
}

pub enum AppError {
    NotFound,
    BadRequest,
    Internal,
}

impl From<sqlx::Error> for AppError {
    fn from(_: sqlx::Error) -> Self {
        AppError::Internal
    }
}

impl From<askama::Error> for AppError {
    fn from(_: askama::Error) -> Self {
        AppError::Internal
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND.into_response(),
            AppError::BadRequest => StatusCode::BAD_REQUEST.into_response(),
            AppError::Internal => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

pub fn html(s: String) -> Response {
    Html(s).into_response()
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(board::index))
        .route("/boards", post(board::create))
        .route("/boards/{id}", get(board::show_board))
        .route("/boards/{id}/rename", post(board::rename))
        .route("/boards/{id}/archive", post(board::archive))
        .route("/boards/{id}/toggle", post(toggle::toggle))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state)
        .merge(crate::assets::router())
}
```

Note: the `assets::router()` (static files) is merged *outside* the auth layer so CSS/JS load without auth — implemented in Task 9. For now, add a temporary `pub fn router() -> Router { Router::new() }` in a stub `src/assets.rs` with `mod assets;` in main, to be fleshed out in Task 9.

- [ ] **Step 4: Create `src/handlers/board.rs` with `show_board`, `index`, and grid-building helper + tests**

```rust
use super::{html, AppError, AppState};
use crate::auth::{HxRequest, UserId};
use crate::calendar::{days_in_month, is_valid_day, MONTH_LABELS};
use crate::store::Board;
use crate::templates::{Cell, Column, EmptyTemplate, GridTemplate, PageTemplate, TabView};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct YearQuery {
    pub year: Option<i32>,
}

fn current_year() -> i32 {
    chrono::Utc::now().format("%Y").to_string().parse().unwrap()
}

pub fn build_grid(
    board: &Board,
    year: i32,
    toggled: &std::collections::HashSet<(u32, u32)>,
) -> GridTemplate {
    let (min_year, max_year) = board.year_bounds(current_year());
    let mut max_days = 28;
    let columns = (1..=12u32)
        .map(|month| {
            let dim = days_in_month(year, month);
            max_days = max_days.max(dim);
            let cells = (1..=31u32)
                .map(|day| Cell {
                    day,
                    valid: is_valid_day(year, month, day),
                    on: toggled.contains(&(month, day)),
                })
                .collect();
            Column { month, label: MONTH_LABELS[(month - 1) as usize], cells }
        })
        .collect();
    GridTemplate { board_id: board.id, year, min_year, max_year, max_days, columns }
}

async fn tabs(state: &AppState, user_id: i64) -> Result<Vec<TabView>, AppError> {
    Ok(state
        .store
        .list_boards(user_id)
        .await?
        .into_iter()
        .map(|b| TabView { id: b.id, name: b.name })
        .collect())
}

pub async fn index(State(state): State<AppState>, UserId(uid): UserId) -> Result<Response, AppError> {
    match state.store.list_boards(uid).await?.first() {
        Some(b) => Ok(Redirect::to(&format!("/boards/{}", b.id)).into_response()),
        None => {
            let t = EmptyTemplate { tabs: vec![], active_id: 0 };
            Ok(html(t.render()?))
        }
    }
}

pub async fn show_board(
    State(state): State<AppState>,
    UserId(uid): UserId,
    HxRequest(is_hx): HxRequest,
    Path(id): Path<i64>,
    Query(q): Query<YearQuery>,
) -> Result<Response, AppError> {
    let board = state.store.get_board(uid, id).await?.ok_or(AppError::NotFound)?;
    let (min_year, max_year) = board.year_bounds(current_year());
    let year = q.year.unwrap_or(max_year).clamp(min_year, max_year);
    let toggled = state.store.toggled_days(board.id, year).await?;
    let grid = build_grid(&board, year, &toggled).render()?;

    if is_hx {
        return Ok(html(grid));
    }
    let page = PageTemplate {
        tabs: tabs(&state, uid).await?,
        active_id: board.id,
        board_name: board.name.clone(),
        grid_html: grid,
    };
    Ok(html(page.render()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_marks_valid_and_toggled() {
        let board = Board {
            id: 7, user_id: 1, name: "b".into(),
            created_at: "2025-01-01T00:00:00+00:00".into(),
            archived_at: None, sort_order: 0,
        };
        let mut set = std::collections::HashSet::new();
        set.insert((1u32, 1u32));
        let g = build_grid(&board, 2026, &set);
        // Feb column: day 29 invalid in 2026 (not leap)
        let feb = &g.columns[1];
        assert!(!feb.cells[28].valid); // index 28 => day 29
        // Jan day 1 on
        assert!(g.columns[0].cells[0].on);
        assert_eq!(g.min_year, 2025);
    }
}
```

- [ ] **Step 5: Create rename/create/archive handler stubs in `src/handlers/board.rs`**

Append (full behaviour tested in Task 8, but signatures must exist for the router to compile):

```rust
use axum::Form;

#[derive(Deserialize)]
pub struct NameForm {
    pub name: String,
}

pub async fn create(
    State(state): State<AppState>,
    UserId(uid): UserId,
) -> Result<Response, AppError> {
    let b = state.store.create_board(uid, "New board").await?;
    Ok(Redirect::to(&format!("/boards/{}", b.id)).into_response())
}

pub async fn rename(
    State(state): State<AppState>,
    UserId(uid): UserId,
    Path(id): Path<i64>,
    Form(form): Form<NameForm>,
) -> Result<Response, AppError> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest);
    }
    state.store.rename_board(uid, id, name).await?.ok_or(AppError::NotFound)?;
    Ok(Redirect::to(&format!("/boards/{}", id)).into_response())
}

pub async fn archive(
    State(state): State<AppState>,
    UserId(uid): UserId,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    if !state.store.archive_board(uid, id).await? {
        return Err(AppError::NotFound);
    }
    Ok(Redirect::to("/").into_response())
}
```

- [ ] **Step 6: Add `mod handlers; mod templates; mod assets;` to `src/main.rs` and create stub `src/assets.rs`**

`src/assets.rs` stub:

```rust
use axum::Router;
pub fn router() -> Router {
    Router::new()
}
```

- [ ] **Step 7: Run tests + lint (full crate now compiles)**

Run: `cargo test --lib && cargo clippy -- -D warnings`
Expected: PASS (all store, calendar, auth, and grid tests).

- [ ] **Step 8: Commit**

```bash
git add src/ templates/
git commit -m "feat: app state, error type, templates, board page and grid handler"
```

---

### Task 8: Toggle handler + management handler tests (oneshot)

**Files:**
- Create: `src/handlers/toggle.rs`
- Modify: `src/handlers/mod.rs` (already references `toggle::toggle`)
- Test: inline `#[cfg(test)]` in `src/handlers/toggle.rs` (full HTTP round-trips via `tower::ServiceExt::oneshot`)

**Interfaces:**
- Consumes: `AppState`, `build_grid`? No — returns a single cell fragment. Uses `Store::toggle_day`, `calendar::is_valid_day`.
- Produces:
  - `handlers::toggle::toggle` — `POST /boards/:id/toggle`, body `{year, month, day}` (form-encoded via `hx-vals` JSON → axum `Json`), validates the day, flips it, returns the updated cell `<button>` fragment.
  - `templates::CellTemplate` for the single-cell fragment.

- [ ] **Step 1: Add `CellTemplate` to `src/templates.rs` and `templates/cell.html`**

`templates/cell.html`:

```html
<button class="cell {% if on %}cell--on{% endif %}"
  hx-post="/boards/{{ board_id }}/toggle"
  hx-vals='{"year": {{ year }}, "month": {{ month }}, "day": {{ day }}}'
  hx-swap="outerHTML">{{ day }}</button>
```

Add to `src/templates.rs`:

```rust
#[derive(Template)]
#[template(path = "cell.html")]
pub struct CellTemplate {
    pub board_id: i64,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub on: bool,
}
```

- [ ] **Step 2: Write failing oneshot tests in `src/handlers/toggle.rs`**

```rust
use super::{html, AppError, AppState};
use crate::auth::UserId;
use crate::calendar::is_valid_day;
use crate::templates::CellTemplate;
use askama::Template;
use axum::extract::{Path, State};
use axum::response::Response;
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct TogglePayload {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::{router, AppState};
    use crate::store::Store;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use sqlx::SqlitePool;
    use tower::ServiceExt;

    async fn app_with_board(pool: SqlitePool) -> (axum::Router, i64) {
        let store = Store { pool };
        let uid = store.upsert_user("dev", None, None).await.unwrap().id;
        let board = store.create_board(uid, "b").await.unwrap();
        let state = AppState { store, dev_user: Some("dev".into()) };
        (router(state), board.id)
    }

    #[sqlx::test]
    async fn toggle_roundtrip_returns_on_cell(pool: SqlitePool) {
        let (app, bid) = app_with_board(pool).await;
        let resp = app
            .oneshot(
                Request::post(format!("/boards/{bid}/toggle"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"year":2026,"month":3,"day":15}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(body.to_vec()).unwrap();
        assert!(s.contains("cell--on"), "expected on cell, got: {s}");
    }

    #[sqlx::test]
    async fn invalid_day_is_400(pool: SqlitePool) {
        let (app, bid) = app_with_board(pool).await;
        let resp = app
            .oneshot(
                Request::post(format!("/boards/{bid}/toggle"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"year":2026,"month":2,"day":30}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test]
    async fn cross_user_board_is_404(pool: SqlitePool) {
        let store = Store { pool };
        let owner = store.upsert_user("owner", None, None).await.unwrap().id;
        let board = store.create_board(owner, "b").await.unwrap();
        // request runs as dev_user "dev", a different subject
        let state = AppState { store, dev_user: Some("dev".into()) };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::post(format!("/boards/{}/toggle", board.id))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"year":2026,"month":1,"day":1}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
```

- [ ] **Step 3: Run to verify fail**

Run: `cargo test --lib handlers::toggle`
Expected: FAIL — `toggle` handler not defined.

- [ ] **Step 4: Implement the handler in `src/handlers/toggle.rs`**

Prepend above the test module:

```rust
pub async fn toggle(
    State(state): State<AppState>,
    UserId(uid): UserId,
    Path(id): Path<i64>,
    Json(p): Json<TogglePayload>,
) -> Result<Response, AppError> {
    // Ownership check (404 if not the user's board).
    let board = state.store.get_board(uid, id).await?.ok_or(AppError::NotFound)?;
    if !(1..=12).contains(&p.month) || !is_valid_day(p.year, p.month, p.day) {
        return Err(AppError::BadRequest);
    }
    let on = state.store.toggle_day(board.id, p.year, p.month, p.day).await?;
    let cell = CellTemplate {
        board_id: board.id,
        year: p.year,
        month: p.month,
        day: p.day,
        on,
    };
    Ok(html(cell.render()?))
}
```

- [ ] **Step 5: Run tests + lint**

Run: `cargo test --lib handlers && cargo clippy -- -D warnings`
Expected: PASS (toggle round-trip, 400, cross-user 404).

- [ ] **Step 6: Add a full-page vs fragment test for `show_board` in `src/handlers/board.rs`**

Append to the `tests` module in `board.rs`:

```rust
#[cfg(test)]
mod http_tests {
    use crate::handlers::{router, AppState};
    use crate::store::Store;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use sqlx::SqlitePool;
    use tower::ServiceExt;

    #[sqlx::test]
    async fn full_page_vs_fragment(pool: SqlitePool) {
        let store = Store { pool };
        let uid = store.upsert_user("dev", None, None).await.unwrap().id;
        let bid = store.create_board(uid, "Exercise").await.unwrap().id;
        let state = AppState { store, dev_user: Some("dev".into()) };
        let app = router(state);

        // Fresh load: full document with tabs.
        let full = app.clone()
            .oneshot(Request::get(format!("/boards/{bid}")).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(full.status(), StatusCode::OK);
        let body = full.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(body.to_vec()).unwrap();
        assert!(s.contains("<!doctype html>"));
        assert!(s.contains("Exercise"));

        // HTMX request: grid fragment only.
        let frag = app
            .oneshot(
                Request::get(format!("/boards/{bid}"))
                    .header("HX-Request", "true")
                    .body(Body::empty()).unwrap(),
            )
            .await.unwrap();
        let body = frag.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(body.to_vec()).unwrap();
        assert!(!s.contains("<!doctype html>"));
        assert!(s.contains("id=\"grid\""));
    }
}
```

- [ ] **Step 7: Run + commit**

Run: `cargo test --lib && cargo clippy -- -D warnings`
Expected: PASS.

```bash
git add src/ templates/
git commit -m "feat: toggle handler and HTTP round-trip tests"
```

---

### Task 9: Static assets embed + main wiring + run

**Files:**
- Modify: `src/assets.rs` (replace stub), `src/main.rs` (full bootstrap)
- Create: `static/app.css`, `static/normalize.css`, `static/htmx.min.js`
- Test: inline `#[cfg(test)]` in `src/assets.rs`

**Interfaces:**
- Consumes: `handlers::{router, AppState}`, `config::Config`, `store::Store`.
- Produces: `assets::router() -> Router` serving `GET /static/{*path}` from embedded files with correct content types; `404` for unknown paths.

- [ ] **Step 1: Download vendored assets**

```bash
curl -fsSL https://unpkg.com/htmx.org@2.0.4/dist/htmx.min.js -o static/htmx.min.js
curl -fsSL https://unpkg.com/normalize.css@8.0.1/normalize.css -o static/normalize.css
```

(If offline, place the files manually; both are permissively licensed. Pin exact versions.)

- [ ] **Step 2: Write `static/app.css`**

```css
:root {
  --bg: #f6f6f4; --fg: #222; --tab-bg: #e6e6e2; --tab-active: #fff;
  --cell-off: #d9d9d4; --cell-border: #c7c7c0;
  --gold: #f2c14e; --gold-glow: rgba(242,193,78,.75);
  --accent: #2a9d8f;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #16171a; --fg: #e8e8e6; --tab-bg: #222; --tab-active: #2c2c31;
    --cell-off: #26262b; --cell-border: #34343b;
    --gold: #f2c14e; --gold-glow: rgba(242,193,78,.6); --accent: #2a9d8f;
  }
}
body { background: var(--bg); color: var(--fg); font-family: system-ui, sans-serif; }
.app { max-width: 1100px; margin: 0 auto; padding: 1rem; }
.tabs { display: flex; gap: .25rem; flex-wrap: wrap; align-items: center; margin-bottom: 1rem; }
.tab { padding: .4rem .8rem; background: var(--tab-bg); border-radius: 6px 6px 0 0;
  text-decoration: none; color: var(--fg); }
.tab--active { background: var(--tab-active); font-weight: 600; }
.tab-new button { border: none; background: var(--tab-bg); border-radius: 6px;
  width: 2rem; height: 2rem; cursor: pointer; color: var(--fg); }
.grid__nav { display: flex; gap: 1rem; align-items: center; justify-content: center;
  margin-bottom: .75rem; }
.grid__nav button { min-width: 2rem; cursor: pointer; }
.grid__nav button:disabled { opacity: .35; cursor: not-allowed; }
.grid__cols { display: flex; gap: .35rem; overflow-x: auto; }
.col { display: flex; flex-direction: column; gap: .25rem; }
.col__label { text-align: center; font-size: .8rem; text-transform: uppercase; }
.cell { width: 2rem; height: 2rem; border: 1px solid var(--cell-border);
  border-radius: 6px; background: var(--cell-off); color: var(--fg);
  font-size: .7rem; cursor: pointer; padding: 0; }
.cell--empty { visibility: hidden; }
.cell--on { background: var(--gold); box-shadow: 0 0 8px 2px var(--gold-glow);
  color: #3a2d00; font-weight: 700; }
.empty { text-align: center; padding: 3rem 0; }
.board-title { text-align: center; }
```

- [ ] **Step 3: Write failing test in `src/assets.rs`**

```rust
use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::extract::Path;
use axum::Router;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn serves_css_with_type() {
        let app = router();
        let resp = app
            .oneshot(Request::get("/static/app.css").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
        assert!(ct.starts_with("text/css"));
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert!(!body.is_empty());
    }

    #[tokio::test]
    async fn unknown_is_404() {
        let app = router();
        let resp = app
            .oneshot(Request::get("/static/nope.js").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
```

- [ ] **Step 4: Implement `assets::router` + static handler**

Add above the tests (replacing the Task-7 stub):

```rust
async fn serve(Path(path): Path<String>) -> Response {
    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn router() -> Router {
    Router::new().route("/static/{*path}", get(serve))
}
```

- [ ] **Step 5: Write full `src/main.rs` bootstrap**

```rust
mod assets;
mod auth;
mod calendar;
mod config;
mod handlers;
mod store;
mod templates;

use config::Config;
use handlers::AppState;
use store::Store;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = Config::from_env();
    let store = Store::connect(&cfg.database_url)
        .await
        .expect("failed to connect/migrate database");
    let state = AppState { store, dev_user: cfg.dev_user.clone() };

    let app = handlers::router(state);
    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr)
        .await
        .expect("failed to bind");
    tracing::info!("listening on {}", cfg.bind_addr);
    axum::serve(listener, app).await.expect("server error");
}
```

- [ ] **Step 6: Run tests, lint, and a manual smoke run**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS.

Manual smoke:

```bash
DEV_USER=dev DATABASE_URL=sqlite://dev.db cargo run
```

Then in another shell:

```bash
curl -s localhost:8080/ -i | head -n1          # 302 -> empty state has no board yet; expect redirect logic
curl -s -X POST localhost:8080/boards -i | head -n1   # 303/302 redirect to new board
```

Verify in a browser at `http://localhost:8080`: create a board, toggle cells (gold glow), use `<`/`>` (bounded), create a second tab. Remove `dev.db` afterward.

- [ ] **Step 7: Commit**

```bash
git add src/ static/
git commit -m "feat: embed static assets, serve them, full bootstrap and run"
```

---

### Task 10: Dockerfile + GitHub Actions → GHCR

**Files:**
- Create: `Dockerfile`, `.dockerignore`, `.github/workflows/ci.yml`

**Interfaces:** none (infra). Deliverable: a locally-building image and a CI workflow.

- [ ] **Step 1: Create `.dockerignore`**

```
target
*.db
*.db-wal
*.db-shm
.git
docs
charts
```

- [ ] **Step 2: Create `Dockerfile` (static musl, distroless)**

```dockerfile
FROM rust:1-slim AS build
RUN rustup target add x86_64-unknown-linux-musl && \
    apt-get update && apt-get install -y musl-tools && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN cp target/x86_64-unknown-linux-musl/release/stars /stars

FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /stars /stars
ENV BIND_ADDR=0.0.0.0:8080 DATABASE_URL=sqlite:///data/stars.db
EXPOSE 8080
VOLUME ["/data"]
USER nonroot
ENTRYPOINT ["/stars"]
```

Note: `SQLX_OFFLINE` is not needed because all queries use the runtime `query`/`query_as` APIs (no compile-time `query!` macros), so no live DB is required at build time.

- [ ] **Step 3: Build locally to verify**

Run: `docker build -t stars:dev .`
Expected: image builds; `docker run --rm -e DEV_USER=dev -p 8080:8080 stars:dev` serves the app.

- [ ] **Step 4: Create `.github/workflows/ci.yml`**

```yaml
name: ci
on:
  push:
    branches: [main]
    tags: ["v*"]
  pull_request:

permissions:
  contents: read
  packages: write

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test

  image:
    needs: test
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: docker/setup-buildx-action@v3
      - uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - uses: docker/metadata-action@v5
        id: meta
        with:
          images: ghcr.io/astromechza/stars
          tags: |
            type=ref,event=branch
            type=semver,pattern={{version}}
            type=sha
      - uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
```

- [ ] **Step 5: Commit**

```bash
git add Dockerfile .dockerignore .github/
git commit -m "chore: container build and GitHub Actions pipeline to GHCR"
```

---

### Task 11: Helm chart

**Files:**
- Create: `charts/stars/Chart.yaml`, `charts/stars/values.yaml`, `charts/stars/templates/{deployment,service,ingress,pvc,_helpers.tpl}.yaml`

**Interfaces:** none (infra). Deliverable: `helm template` renders valid manifests.

- [ ] **Step 1: `charts/stars/Chart.yaml`**

```yaml
apiVersion: v2
name: stars
description: Every-day calendar web app
type: application
version: 0.1.0
appVersion: "0.1.0"
```

- [ ] **Step 2: `charts/stars/values.yaml`**

```yaml
image:
  repository: ghcr.io/astromechza/stars
  tag: main
  pullPolicy: IfNotPresent

service:
  port: 80
  targetPort: 8080

persistence:
  size: 1Gi
  storageClass: ""

ingress:
  enabled: true
  className: nginx
  host: stars.local
  # Authelia forward-auth annotations (adjust URLs to your Authelia install).
  annotations:
    nginx.ingress.kubernetes.io/auth-url: "http://authelia.authelia.svc.cluster.local/api/verify"
    nginx.ingress.kubernetes.io/auth-signin: "https://auth.example.com?rm=$request_method"
    nginx.ingress.kubernetes.io/auth-response-headers: "Remote-User,Remote-Email,Remote-Name,Remote-Groups"

env: {}
```

- [ ] **Step 3: `charts/stars/templates/_helpers.tpl`**

```yaml
{{- define "stars.fullname" -}}
{{- printf "%s-%s" .Release.Name .Chart.Name | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "stars.labels" -}}
app.kubernetes.io/name: {{ .Chart.Name }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}
```

- [ ] **Step 4: `charts/stars/templates/pvc.yaml`**

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: {{ include "stars.fullname" . }}-data
  labels: {{- include "stars.labels" . | nindent 4 }}
spec:
  accessModes: ["ReadWriteOnce"]
  resources:
    requests:
      storage: {{ .Values.persistence.size }}
  {{- with .Values.persistence.storageClass }}
  storageClassName: {{ . }}
  {{- end }}
```

- [ ] **Step 5: `charts/stars/templates/deployment.yaml`**

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: {{ include "stars.fullname" . }}
  labels: {{- include "stars.labels" . | nindent 4 }}
spec:
  replicas: 1
  strategy:
    type: Recreate
  selector:
    matchLabels: {{- include "stars.labels" . | nindent 6 }}
  template:
    metadata:
      labels: {{- include "stars.labels" . | nindent 8 }}
    spec:
      securityContext:
        runAsNonRoot: true
        fsGroup: 65532
      containers:
        - name: stars
          image: "{{ .Values.image.repository }}:{{ .Values.image.tag }}"
          imagePullPolicy: {{ .Values.image.pullPolicy }}
          ports:
            - containerPort: {{ .Values.service.targetPort }}
          env:
            - name: DATABASE_URL
              value: "sqlite:///data/stars.db"
            {{- range $k, $v := .Values.env }}
            - name: {{ $k }}
              value: {{ $v | quote }}
            {{- end }}
          volumeMounts:
            - name: data
              mountPath: /data
      volumes:
        - name: data
          persistentVolumeClaim:
            claimName: {{ include "stars.fullname" . }}-data
```

- [ ] **Step 6: `charts/stars/templates/service.yaml`**

```yaml
apiVersion: v1
kind: Service
metadata:
  name: {{ include "stars.fullname" . }}
  labels: {{- include "stars.labels" . | nindent 4 }}
spec:
  selector: {{- include "stars.labels" . | nindent 4 }}
  ports:
    - port: {{ .Values.service.port }}
      targetPort: {{ .Values.service.targetPort }}
```

- [ ] **Step 7: `charts/stars/templates/ingress.yaml`**

```yaml
{{- if .Values.ingress.enabled }}
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: {{ include "stars.fullname" . }}
  labels: {{- include "stars.labels" . | nindent 4 }}
  annotations:
    {{- toYaml .Values.ingress.annotations | nindent 4 }}
spec:
  ingressClassName: {{ .Values.ingress.className }}
  rules:
    - host: {{ .Values.ingress.host }}
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: {{ include "stars.fullname" . }}
                port:
                  number: {{ .Values.service.port }}
{{- end }}
```

- [ ] **Step 8: Validate rendering**

Run: `helm template charts/stars`
Expected: valid YAML for Deployment, Service, Ingress, PVC. Optionally `helm lint charts/stars`.

- [ ] **Step 9: Commit**

```bash
git add charts/
git commit -m "chore: helm chart with Authelia ingress and SQLite PVC"
```

---

## Self-Review

**Spec coverage:**
- Per-year boards, grid, toggle → Tasks 1, 4, 5, 7, 8. ✓
- Year `<`/`>` bounds (creation year … current year) → `year_bounds` (Task 4), clamp + disabled buttons (Task 7). ✓
- Tabs create/rename/archive → Task 7 (create/rename/archive handlers), Task 4 (store). ✓
- Authelia auto-provision + header trust + dev override → Task 6 (`resolve_identity`, `auth_middleware`), Task 2 (`DEV_USER`). ✓
- Cross-user 404 not 403 → Tasks 4, 8 tests. ✓
- `HX-Request` full-page vs fragment → Task 6 (`is_hx`/`HxRequest`), Task 7 (`show_board`), Task 8 test. ✓
- `toggled_at` timestamp stored → Task 5. ✓
- Only valid days rendered/insertable → Task 1 (`is_valid_day`), Task 7 (grid), Task 8 (400 on invalid). ✓
- SQLite WAL + foreign keys → Task 3. ✓
- Embedded assets, dark/light CSS vars → Task 9. ✓
- Static musl container, GH Actions → GHCR → Task 10. ✓
- Helm chart (Deployment 1 replica, Service, Ingress w/ Authelia, PVC) → Task 11. ✓

**Placeholder scan:** No TBD/TODO; all code steps contain real code. Vendored asset download uses pinned versions.

**Type consistency:** `Store` methods, `Board`/`User` fields, `AppState { store, dev_user }`, `AppError`, `UserId`/`HxRequest`, `TogglePayload`, template struct fields are consistent across tasks 3–9. Router paths use axum 0.8 `{id}`/`{*path}` capture syntax throughout.

**Sequencing note:** Task 6 (auth) references `crate::handlers::AppState` created in Task 7; its pure-function tests run independently, and the full crate compiles once Task 7 lands. Execute Task 7 immediately after Task 6.
