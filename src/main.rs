mod app;
mod builtins;
mod core;
pub mod extension;

fn main() -> eframe::Result<()> {
    app::run()
}
