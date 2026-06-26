pub mod detail;
pub mod projects;
pub mod tasks;

pub use detail::{asset_panel_height, detail_asset_panel_rect, draw_detail, DetailParams};
pub use projects::draw_projects;
pub use tasks::draw_tasks;
