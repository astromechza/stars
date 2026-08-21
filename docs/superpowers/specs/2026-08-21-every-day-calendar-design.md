# Every-Day Calendar — Design

**Date:** 2026-08-21
**Status:** Approved for implementation planning

## Summary

A small SQLite-backed Rust web application inspired by Simone Giertz's
"Every Day Calendar" Kickstarter. Each *board* is a year-scoped grid with
one vertical column per month and one togglable cell per valid day. Users
toggle a cell to mark a day "done" (glowing gold, like the physical
device). Multiple boards per user are presented as whole-page tabs that can
be created, renamed, and archived. Authentication is delegated to an
upstream Authelia forward-auth proxy; users are auto-provisioned on first
sight.

## Goals

- Faithful digital analogue of the physical device: 12-column grid, one
  cell per day, toggle on/off.
- Per-year boards with history preserved; navigate to previous/next years
  within bounds.
- Multiple boards per user via tabs (create / rename / archive).
- Per-user data isolation via Authelia-provided identity headers.
- Ship as a small static-binary container, deployed to local Kubernetes via
  a Helm chart, built and published through a GitHub Actions pipeline into
  GHCR.

## Non-Goals

- No in-app authentication UI, password storage, or session management —
  Authelia owns that.
- No streaks, reminders, notifications, or analytics in the first version
  (the `toggled_at` timestamp is stored to keep those options open later).
- No multi-writer scaling. SQLite single-writer with one replica is
  sufficient.
- No mobile-native app; responsive web only.

## Tech Stack

| Concern      | Choice                                             |
|--------------|----------------------------------------------------|
| Language     | Rust (latest stable, 2024 edition)                 |
| Web          | axum                                               |
| DB access    | sqlx (async, compile-checked SQL, migrations)      |
| Database     | SQLite (WAL mode)                                   |
| Templates    | askama (compile-checked)                            |
| Frontend     | HTMX + server-rendered fragments                   |
| Styling      | normalize.css + small custom `app.css`, CSS vars   |
| Static assets| `rust-embed` (htmx, css embedded in binary)        |
| Container    | Multi-stage Dockerfile, static musl, distroless/scratch |
| CI/CD        | GitHub Actions → GHCR                               |
| Deploy       | Helm chart → local Kubernetes behind Ingress       |

## Architecture

```
        Authelia (at Ingress) --forward-auth headers-->  axum service
                                                            │
   ┌────────────────────────────────────────────────────────┤
   │  auth middleware:                                        │
   │    read Remote-User / Remote-Email / Remote-Name         │
   │    upsert user by subject -> inject UserId extension      │
   ├──────────────────────────────────────────────────────────┤
   │  handlers: render HTML / fragments (askama) <-HTMX-> UA   │
   ├──────────────────────────────────────────────────────────┤
   │  store: sqlx SqlitePool (WAL)                            │
   └──────────────────────────────────────────────────────────┘
                              │
                     SQLite file on a PVC
```

Layered, with clear boundaries:

- **store** — owns all SQL. Exposes typed functions (`upsert_user`,
  `list_boards`, `create_board`, `rename_board`, `archive_board`,
  `toggle_day`, `board_year_bounds`, `toggled_days`). Testable against a
  temporary SQLite file with no HTTP involved.
- **auth** — middleware that turns proxy headers into a `UserId`. The only
  place that trusts request headers.
- **handlers** — thin; parse request, call store, render template. No SQL.
- **templates** — askama structs mirror what each view needs.
- **assets** — embedded static files served under `/static`.

### Configuration (env)

| Var            | Purpose                                                    |
|----------------|------------------------------------------------------------|
| `DATABASE_URL` | SQLite path, e.g. `sqlite:///data/stars.db`                |
| `BIND_ADDR`    | Listen address, default `0.0.0.0:8080`                     |
| `DEV_USER`     | Optional. When set and proxy headers absent, use this subject/email for local development. Never set in production. |

## Data Model

```sql
CREATE TABLE users (
    id           INTEGER PRIMARY KEY,
    subject      TEXT NOT NULL UNIQUE,   -- Authelia Remote-User
    email        TEXT,
    display_name TEXT,
    created_at   TEXT NOT NULL           -- RFC3339 UTC
);

CREATE TABLE boards (
    id          INTEGER PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id),
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,           -- RFC3339 UTC; year = lower nav bound
    archived_at TEXT,                    -- NULL = active
    sort_order  INTEGER NOT NULL
);
CREATE INDEX idx_boards_user ON boards(user_id);

CREATE TABLE toggles (
    board_id   INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    year       INTEGER NOT NULL,
    month      INTEGER NOT NULL,         -- 1..12
    day        INTEGER NOT NULL,         -- 1..31, only valid days
    toggled_at TEXT NOT NULL,            -- RFC3339 UTC
    PRIMARY KEY (board_id, year, month, day)
);
```

Rules:

- A toggle is represented by the *presence* of a row. Untoggling deletes the
  row. There is no boolean column.
- `boards.created_at` year is the lower bound for the `<` year navigation.
  The current calendar year is the upper bound.
- Archiving sets `archived_at`; archived boards are hidden from the tab bar
  but never deleted, and their toggles are preserved.
- Only valid calendar days are ever rendered, so invalid combinations
  (e.g. Feb 30, Apr 31) can never be inserted. Feb 29 renders only in leap
  years.

## Routes and HTMX Flow

| Method | Path                              | Purpose                                          |
|--------|-----------------------------------|--------------------------------------------------|
| GET    | `/`                               | Redirect to the user's most recent active board (or an empty state if none). |
| GET    | `/boards/:id?year=YYYY`           | Full page: tab bar + grid for the given/current year. |
| GET    | `/boards/:id/grid?year=YYYY`      | Grid fragment only (for year `<`/`>` navigation). |
| POST   | `/boards/:id/toggle`              | Body `{year, month, day}`; flip cell; return updated cell fragment. |
| POST   | `/boards`                         | Create board; redirect to it.                    |
| POST   | `/boards/:id/rename`              | Body `{name}`; return updated tab fragment.      |
| POST   | `/boards/:id/archive`             | Soft-archive; return updated tab bar / redirect. |

Interaction details:

- **Cell** — `<button hx-post="/boards/:id/toggle" hx-vals='{...}'
  hx-swap="outerHTML">`. The server flips state and returns the same button
  with the on/off class toggled. No client JS state.
- **Year navigation** — `<` and `>` issue `hx-get` for the grid fragment and
  swap the grid container. Buttons are disabled at the bounds (creation year
  and current year).
- **Tabs** — whole-page top bar. `+` creates a board. Rename is an inline
  small form swapping the tab fragment. Archive removes the tab.

## Auth Middleware

1. Read `Remote-User` (subject), `Remote-Email`, `Remote-Name` from the
   request.
2. If the subject header is absent:
   - if `DEV_USER` is set, synthesize a development identity from it;
   - otherwise respond `401 Unauthorized`.
3. Upsert the user by `subject` (updating email/display name if changed);
   inject `UserId` into request extensions.
4. Every board and toggle query is scoped `WHERE user_id = ?`. A request for
   another user's board returns `404 Not Found` (not `403`) so existence is
   not leaked.

Security note: the app trusts identity headers unconditionally, so it MUST
only ever be reachable through the Authelia forward-auth proxy. The Ingress
must strip these headers from inbound client requests and set them only from
Authelia. `DEV_USER` must never be set in a deployed environment.

## UI / Styling

- `normalize.css` plus a small hand-written `app.css`.
- Palette via CSS custom properties. Dark/light driven by
  `prefers-color-scheme`, with a manual toggle persisted (cookie or
  localStorage).
- Grid: 12 columns (months, labelled jan…dec), up to 31 rows. A toggled cell
  glows gold (via `box-shadow`), echoing the physical device; an off cell is
  dim. Month labels across the top, day numbers down the side.
- Responsive: the grid scrolls horizontally on narrow viewports; tabs wrap or
  scroll.
- No inline styles beyond genuinely one-off cases; reusable CSS classes and
  variables otherwise.

## Error Handling

- Store functions return `Result`; handlers map errors to HTTP status via a
  single error type implementing `IntoResponse`.
- Not found / cross-user access → `404`.
- Missing auth → `401`.
- Malformed toggle input (bad month/day) → `400`; validated server-side
  against real calendar days regardless of client.
- SQLite busy under WAL is retried briefly; otherwise `500` with a logged
  cause.

## Testing Strategy

- **Store layer** — unit tests against a temporary SQLite database: toggle
  create/delete idempotency, day validity, year-bound computation, archive
  filtering, per-user scoping.
- **Handlers** — axum `oneshot` tests: auth extraction (headers present,
  absent + `DEV_USER`, absent + none), cross-user access returns 404, a
  toggle round-trip returns the flipped fragment, year-nav bounds.
- Focus on our own logic (bounds, day validity, scoping, archive filtering).
  No tests over the standard library or framework. No coverage targets.

## Deployment

- **Dockerfile** — multi-stage with `cargo-chef` dependency caching,
  producing a static musl binary in a `scratch`/distroless final image.
- **GitHub Actions** — on push/PR: `cargo fmt --check`, `cargo clippy -D
  warnings`, `cargo test`; on push to main / tags: `docker buildx` build and
  push to `ghcr.io/astromechza/stars`.
- **Helm chart** — `Deployment` (1 replica, SQLite single-writer),
  `Service`, `Ingress` (Authelia forward-auth annotations), and a `PVC` for
  the SQLite file. Values expose image tag, ingress host, and storage size.

## Open Questions / Future Work

- Streaks and per-month completion counts (data already captured via
  `toggled_at`).
- Board reordering UI (schema already has `sort_order`).
- Export / import of a board's history.
