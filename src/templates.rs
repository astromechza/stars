use askama::Template;

#[derive(Clone)]
pub struct TabView {
    pub id: i64,
    pub name: String,
}

pub struct Cell {
    pub day: u32,
    pub valid: bool,
    /// 0 = cleared, 1 = outline glow, 2 = full glow.
    pub state: u8,
    /// True when this cell is the current date (only in the current year).
    pub today: bool,
    /// True when the day falls on a Saturday or Sunday.
    pub weekend: bool,
}

pub struct Column {
    pub month: u32,
    pub label: &'static str,
    pub cells: Vec<Cell>,
}

/// Per-year summary shown above the grid columns.
#[derive(Default)]
pub struct Stats {
    /// Number of full-glow (state 2) cells: the "true" days.
    pub true_count: u32,
    /// true / (true + outline) as a whole percent; 0 when nothing is set.
    pub true_ratio_pct: u32,
    /// (true + outline) / days-in-year as a whole percent: how much of the
    /// year has been marked either way.
    pub year_coverage_pct: u32,
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
    pub stats: Stats,
    /// Grid renders the stats inline, so no out-of-band swap here.
    pub stats_oob: bool,
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

#[derive(Template)]
#[template(path = "cell.html")]
pub struct CellTemplate {
    pub board_id: i64,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    /// 0 = cleared, 1 = outline glow, 2 = full glow.
    pub state: u8,
    /// True when this cell is the current date (only in the current year).
    pub today: bool,
    /// True when the day falls on a Saturday or Sunday.
    pub weekend: bool,
    /// Refreshed year stats, swapped out-of-band alongside the cell.
    pub stats: Stats,
    /// Always true here: the toggle response carries the stats OOB.
    pub stats_oob: bool,
}
