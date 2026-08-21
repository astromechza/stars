use super::{AppError, AppState, html};
use crate::auth::UserId;
use crate::calendar::is_valid_day;
use crate::templates::CellTemplate;
use askama::Template;
use axum::Form;
use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct TogglePayload {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

pub async fn toggle(
    State(state): State<AppState>,
    UserId(uid): UserId,
    Path(id): Path<i64>,
    Form(p): Form<TogglePayload>,
) -> Result<Response, AppError> {
    // Ownership check (404 if not the user's board).
    let board = state
        .store
        .get_board(uid, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if !(1..=12).contains(&p.month) || !is_valid_day(p.year, p.month, p.day) {
        return Err(AppError::BadRequest);
    }
    let on = state
        .store
        .toggle_day(board.id, p.year, p.month, p.day)
        .await?;
    let cell = CellTemplate {
        board_id: board.id,
        year: p.year,
        month: p.month,
        day: p.day,
        on,
    };
    Ok(html(cell.render()?))
}

#[cfg(test)]
mod tests {
    use crate::handlers::{AppState, router};
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
        let state = AppState {
            store,
            dev_user: Some("dev".into()),
        };
        (router(state), board.id)
    }

    #[sqlx::test]
    async fn toggle_roundtrip_returns_on_cell(pool: SqlitePool) {
        let (app, bid) = app_with_board(pool).await;
        let resp = app
            .oneshot(
                Request::post(format!("/boards/{bid}/toggle"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("year=2026&month=3&day=15"))
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
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("year=2026&month=2&day=30"))
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
        let state = AppState {
            store,
            dev_user: Some("dev".into()),
        };
        let app = router(state);
        let resp = app
            .oneshot(
                Request::post(format!("/boards/{}/toggle", board.id))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("year=2026&month=1&day=1"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
