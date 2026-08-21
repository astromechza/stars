use crate::auth::auth_middleware;
use crate::store::Store;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Router, middleware};

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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
        .merge(crate::assets::router())
}
