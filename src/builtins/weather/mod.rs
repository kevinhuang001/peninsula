mod alerts;
mod client;
mod extension;
mod model;
mod preferences;

pub use client::{search_cities, CityResult};
pub use extension::{descriptors, WeatherExtension};
pub use preferences::{WeatherLocation, WeatherPreferences};
