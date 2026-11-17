#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod builtins;
mod core;
pub mod extension;
pub mod ui;

fn main() -> eframe::Result<()> {
    app::run()
}
