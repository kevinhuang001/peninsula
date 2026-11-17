mod alerts;
mod client;
mod extension;
mod model;
mod preferences;

pub use client::{search_cities, CityResult};
pub use extension::{descriptors, WeatherExtension, COMPACT_ID, WIDGET_ID};
pub use preferences::{WeatherLocation, WeatherPreferences, MAX_REFRESH_MINUTES};
