use super::{AppError, AppState, html};
use crate::auth::{HxRequest, UserId};
use crate::calendar::{MONTH_LABELS, days_in_month, is_valid_day};
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
