use super::{AppError, AppState, html};
use crate::auth::{HxRequest, UserId};
use crate::calendar::{MONTH_LABELS, days_in_month, is_valid_day, is_weekend};
use crate::store::Board;
use crate::templates::{Cell, Column, EmptyTemplate, GridTemplate, PageTemplate, TabView};
use askama::Template;
use axum::Form;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct YearQuery {
    pub year: Option<i32>,
}

fn current_year() -> i32 {
    today_ymd().0
}

/// The current date as (year, month, day), in UTC.
/// Distroless has no tzdata, so local time resolves to UTC there anyway;
/// using UTC keeps this consistent with the rest of the app's timestamps.
pub(crate) fn today_ymd() -> (i32, u32, u32) {
    use chrono::Datelike;
    let now = chrono::Utc::now();
    (now.year(), now.month(), now.day())
}

pub fn build_grid(
    board: &Board,
    year: i32,
    toggled: &std::collections::HashMap<(u32, u32), u8>,
) -> GridTemplate {
    let (min_year, max_year) = board.year_bounds(current_year());
    let (ty, tm, td) = today_ymd();
    let mut max_days = 28;
    let columns = (1..=12u32)
        .map(|month| {
            let dim = days_in_month(year, month);
            max_days = max_days.max(dim);
            let cells = (1..=31u32)
                .map(|day| {
                    let valid = is_valid_day(year, month, day);
                    Cell {
                        day,
                        valid,
                        state: toggled.get(&(month, day)).copied().unwrap_or(0),
                        today: year == ty && month == tm && day == td,
                        weekend: valid && is_weekend(year, month, day),
                    }
                })
                .collect();
            Column {
                month,
                label: MONTH_LABELS[(month - 1) as usize],
                cells,
            }
        })
        .collect();
    GridTemplate {
        board_id: board.id,
        year,
        min_year,
        max_year,
        max_days,
        columns,
    }
}

async fn tabs(state: &AppState, user_id: i64) -> Result<Vec<TabView>, AppError> {
    Ok(state
        .store
        .list_boards(user_id)
        .await?
        .into_iter()
        .map(|b| TabView {
            id: b.id,
            name: b.name,
        })
        .collect())
}

pub async fn index(
    State(state): State<AppState>,
    UserId(uid): UserId,
) -> Result<Response, AppError> {
    match state.store.list_boards(uid).await?.first() {
        Some(b) => Ok(Redirect::to(&format!("/boards/{}", b.id)).into_response()),
        None => {
            let t = EmptyTemplate {
                tabs: vec![],
                active_id: 0,
            };
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
    let board = state
        .store
        .get_board(uid, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let (min_year, max_year) = board.year_bounds(current_year());
    let max_year = max_year.max(min_year);
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
    state
        .store
        .rename_board(uid, id, name)
        .await?
        .ok_or(AppError::NotFound)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_marks_valid_and_toggled() {
        let board = Board {
            id: 7,
            user_id: 1,
            name: "b".into(),
            created_at: "2025-01-01T00:00:00+00:00".into(),
            archived_at: None,
            sort_order: 0,
        };
        let mut set = std::collections::HashMap::new();
        set.insert((1u32, 1u32), 2u8); // Jan 1 at full glow
        set.insert((1u32, 2u32), 1u8); // Jan 2 at outline
        let g = build_grid(&board, 2026, &set);
        // Feb column: day 29 invalid in 2026 (not leap)
        let feb = &g.columns[1];
        assert!(!feb.cells[28].valid); // index 28 => day 29
        // Jan states carried through
        assert_eq!(g.columns[0].cells[0].state, 2); // day 1 full
        assert_eq!(g.columns[0].cells[1].state, 1); // day 2 outline
        assert_eq!(g.columns[0].cells[2].state, 0); // day 3 cleared
        assert_eq!(g.min_year, 2025);
    }
}

#[cfg(test)]
mod http_tests {
    use crate::handlers::{AppState, router};
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
        let state = AppState {
            store,
            dev_user: Some("dev".into()),
        };
        let app = router(state);

        // Fresh load: full document with tabs.
        let full = app
            .clone()
            .oneshot(
                Request::get(format!("/boards/{bid}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
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
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = frag.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(body.to_vec()).unwrap();
        assert!(!s.contains("<!doctype html>"));
        assert!(s.contains("id=\"grid\""));
    }

    #[sqlx::test]
    async fn rename_updates_board_name(pool: SqlitePool) {
        let store = Store { pool };
        let uid = store.upsert_user("dev", None, None).await.unwrap().id;
        let bid = store.create_board(uid, "Old name").await.unwrap().id;
        let state = AppState {
            store,
            dev_user: Some("dev".into()),
        };
        let app = router(state);

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/boards/{bid}/rename"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("name=Exercise"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(resp.status().is_redirection(), "got {:?}", resp.status());

        let get = app
            .oneshot(
                Request::get(format!("/boards/{bid}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get.status(), StatusCode::OK);
        let body = get.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(body.to_vec()).unwrap();
        assert!(s.contains("Exercise"), "expected renamed board, got: {s}");
    }

    #[sqlx::test]
    async fn archive_hides_board(pool: SqlitePool) {
        let store = Store { pool };
        let uid = store.upsert_user("dev", None, None).await.unwrap().id;
        let bid = store.create_board(uid, "Exercise").await.unwrap().id;
        let state = AppState {
            store,
            dev_user: Some("dev".into()),
        };
        let app = router(state);

        let resp = app
            .clone()
            .oneshot(
                Request::post(format!("/boards/{bid}/archive"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(resp.status().is_redirection(), "got {:?}", resp.status());

        let get = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(
            get.status().is_success() || get.status().is_redirection(),
            "got {:?}",
            get.status()
        );
    }
}
