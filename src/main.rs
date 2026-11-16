mod app;
mod builtins;
mod downloads;
pub mod extension;
mod metrics;

fn main() -> eframe::Result<()> {
    app::run()
}
