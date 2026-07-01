pub mod asset_panel;
pub mod detail;
pub mod projects;
pub mod tasks;

pub use detail::{draw_detail, DetailParams};
pub use projects::draw_projects;
pub use tasks::draw_tasks;
