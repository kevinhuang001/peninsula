//! In-process services for Peninsula extensions.
//!
//! The SDK is a convenience boundary, not a sandbox. Built-in extensions keep
//! their normal process and filesystem permissions while sharing consistent
//! storage, secret, process, task, and host-service APIs.

pub mod auth;
pub mod cache;
pub mod context;
pub mod diagnostics;
pub mod events;
pub mod http;
pub mod i18n;
pub mod notifications;
pub mod process;
pub mod resources;
pub mod secrets;
pub mod settings;
pub mod storage;
pub mod system;
pub mod tasks;

pub use context::SdkContext;
