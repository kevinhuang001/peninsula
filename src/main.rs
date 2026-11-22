#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod builtins;
mod core;
pub mod extension;
mod single_instance;
pub mod ui;

fn main() -> eframe::Result<()> {
    let _instance = match single_instance::InstanceGuard::acquire()
        .map_err(|error| eframe::Error::AppCreation(Box::new(error)))?
    {
        Some(instance) => instance,
        None => return Ok(()),
    };
    app::run()
}
