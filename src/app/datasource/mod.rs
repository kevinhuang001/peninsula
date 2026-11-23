//! Data source management rendered directly inside the expanded capsule.
use crate::core::datasource::{self, DataSourceManager, Definition, InputType, SourceSnapshot};
use crate::ui::typography::Typography;
use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Vec2};
use serde_json::Value;
use std::{collections::HashMap, fs, time::Duration};

mod components;
mod dialogs;
mod forms;
mod support;
mod view;
use components::*;
use support::*;

const LEFT_WIDTH: f32 = 212.0;
const COLUMN_GAP: f32 = 12.0;
const CARD_CONTENT_WIDTH: f32 = 470.0;
const VIEW_WIDTH: f32 = 790.0;
const VIEW_HEIGHT: f32 = 570.0;

pub(super) fn preferred_size(limit: [f32; 2]) -> Vec2 {
    Vec2::new(VIEW_WIDTH.min(limit[0]).max(1.0), VIEW_HEIGHT.min(limit[1]).max(1.0))
}

#[derive(Default)]
pub(super) struct DataSourceView {
    selected: Option<String>,
    candidate: Option<Definition>,
    candidate_icon: Option<egui::TextureHandle>,
    allow_private_network: bool,
    status: String,
    status_source: Option<String>,
    preferences_dirty: bool,
    delete_candidate: Option<String>,
    text_drafts: HashMap<String, String>,
    runtime_drafts: HashMap<String, String>,
}
