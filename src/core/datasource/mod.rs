mod schema;

use crate::{
    extension::{CompactView, ExtensionId, Notification, NotificationPriority, ViewNode},
    sdk::secrets::SecretStore,
    ui::declarative::{ColorSpec, CompactLayout, FontSize, FontWeight, TextStyle},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use eframe::egui;
use image::GenericImageView;
use schema::input_numeric_value;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::Read,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::Url;

pub use schema::{
    parse_duration, AlertOperator, Definition, HttpMethod, IconSpec, InputDefinition, InputType,
    ValueDefinition,
};

const MAX_RESPONSE_BYTES: usize = 1_048_576;
const MAX_ICON_BYTES: usize = 1_000_000;
const MAX_ICON_PIXELS: u64 = 1_000_000;
const MAX_CONCURRENT_REQUESTS: usize = 4;
const DATA_SOURCE_COMPACT_PREFIX: &str = "datasource/";
const DEFAULT_REFRESH_INTERVAL_MS: u64 = 5 * 60 * 1000;
const DEFAULT_MIN_ALERT_INTERVAL_MS: u64 = 60 * 60 * 1000;
const MIN_ALERT_INTERVAL_MS: u64 = 60 * 1000;
const MAX_ALERT_INTERVAL_MS: u64 = 30 * 24 * 60 * 60 * 1000;
const MAX_REFRESH_INTERVAL_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceConfig {
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    alerts: bool,
    #[serde(default = "default_refresh_interval_ms")]
    refresh_interval_ms: u64,
    #[serde(default = "default_min_alert_interval_ms")]
    min_alert_interval_ms: u64,
    #[serde(default)]
    allow_private_network: bool,
    #[serde(default)]
    origin: String,
    #[serde(default)]
    inputs: BTreeMap<String, Value>,
    // Legacy fields from the pre-`$名称$` store format. They are still accepted so
    // existing instances.json files load, but are never written or consulted again.
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    allow_plaintext_secrets: bool,
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    secret_refs: BTreeMap<String, String>,
}

fn default_true() -> bool {
    true
}
fn default_refresh_interval_ms() -> u64 {
    DEFAULT_REFRESH_INTERVAL_MS
}
fn default_min_alert_interval_ms() -> u64 {
    DEFAULT_MIN_ALERT_INTERVAL_MS
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedState {
    #[serde(default)]
    value: Option<f64>,
    #[serde(default)]
    formatted: Option<String>,
    #[serde(default)]
    updated_ms: Option<u64>,
    #[serde(default)]
    last_request_ms: Option<u64>,
    #[serde(default)]
    retry_at_ms: Option<u64>,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    http_status: Option<u16>,
    #[serde(default)]
    last_alert_ms: Option<u64>,
    #[serde(default)]
    pending_alert: Option<PendingAlertMarker>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PendingAlertMarker {
    notification_id: String,
    alerted_at_ms: u64,
}

struct ManagedSource {
    definition: Definition,
    config: SourceConfig,
    state: SavedState,
    compact: bool,
    next_refresh: Instant,
    last_request: Option<Instant>,
    in_flight: bool,
    generation: u64,
    retry_count: u8,
    force_refresh: bool,
    cancellation: Option<crate::sdk::tasks::CancellationToken>,
    icon_warning: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SourceSnapshot {
    pub id: String,
    pub name: String,
    pub description: String,
    pub origin: String,
    pub enabled: bool,
    pub compact: bool,
    pub alerts: bool,
    pub refresh_interval_ms: u64,
    pub min_alert_interval_ms: u64,
    pub next_refresh_in_ms: u64,
    pub inputs: BTreeMap<String, Value>,
    pub input_schema: BTreeMap<String, InputDefinition>,
    pub value_path: String,
    pub value: Option<f64>,
    pub formatted: Option<String>,
    pub updated_ms: Option<u64>,
    pub last_request_ms: Option<u64>,
    pub last_alert_ms: Option<u64>,
    pub alert_status: String,
    pub last_error: Option<String>,
    pub http_status: Option<u16>,
    pub in_flight: bool,
    pub icon_warning: Option<String>,
    pub alert_enabled: bool,
    pub storage_error: Option<String>,
}

struct FinishedRequest {
    id: String,
    generation: u64,
    result: Result<FetchResult, FetchError>,
}

#[derive(Debug)]
struct FetchResult {
    value: f64,
    formatted: String,
    status: u16,
}

struct FetchError {
    message: String,
    status: Option<u16>,
    retry_after: Option<Duration>,
}

pub struct DataSourceManager {
    root: PathBuf,
    sources: BTreeMap<String, ManagedSource>,
    sender: mpsc::Sender<FinishedRequest>,
    receiver: mpsc::Receiver<FinishedRequest>,
    active_requests: usize,
    registered_icons: HashMap<String, (u64, Option<String>)>,
    icon_textures: HashMap<String, egui::TextureHandle>,
    storage_read_error: Option<String>,
    storage_write_error: Option<String>,
    definition_errors: Vec<String>,
}

pub struct PendingAlert {
    pub notification: Notification,
    pub source_id: String,
    pub alerted_at_ms: u64,
}

impl DataSourceManager {
    pub fn open(data_dir: &Path) -> Self {
        let root = data_dir.join("data-sources");
        let definitions_dir = root.join("definitions");
        let definitions_dir_error = fs::create_dir_all(&definitions_dir)
            .err()
            .map(|error| format!("无法准备定义目录：{error}"));
        let (configs, config_error): (BTreeMap<String, SourceConfig>, _) =
            read_json(&root.join("instances.json"));
        let (states, state_error): (BTreeMap<String, SavedState>, _) =
            read_json(&root.join("state.json"));
        let mut storage_read_error = config_error.or(state_error).or(definitions_dir_error);
        let mut definition_errors = Vec::new();
        let now = Instant::now();
        let mut sources = BTreeMap::new();
        if let Err(error) = fs::read_dir(&definitions_dir) {
            storage_read_error.get_or_insert_with(|| format!("无法列出定义目录：{error}"));
        }
        if let Ok(entries) = fs::read_dir(&definitions_dir) {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .strip_suffix(".pds.json")
                    .is_none()
                {
                    continue;
                }
                let Ok(text) = fs::read_to_string(entry.path()) else {
                    definition_errors.push(format!("无法读取定义文件：{}", entry.path().display()));
                    continue;
                };
                let definition = match Definition::parse(&text) {
                    Ok(definition) => definition,
                    Err(error) => {
                        definition_errors
                            .push(format!("定义文件 {} 无效：{error}", entry.path().display()));
                        continue;
                    }
                };
                if entry.file_name().to_string_lossy() != format!("{}.pds.json", definition.id) {
                    definition_errors.push(format!(
                        "定义文件名与数据源 ID 不匹配：{}",
                        entry.path().display()
                    ));
                    continue;
                }
                let id = definition.id.clone();
                let mut config = configs.get(&id).cloned().unwrap_or_else(|| {
                    new_config(&definition, definition.origin().unwrap_or_default(), false)
                });
                if !(schema::MIN_REFRESH.as_millis() as u64..=MAX_REFRESH_INTERVAL_MS)
                    .contains(&config.refresh_interval_ms)
                {
                    storage_read_error.get_or_insert_with(|| {
                        format!("数据源 {id} 的刷新间隔无效，实例文件已保留")
                    });
                    config.refresh_interval_ms = DEFAULT_REFRESH_INTERVAL_MS;
                }
                if !(MIN_ALERT_INTERVAL_MS..=MAX_ALERT_INTERVAL_MS)
                    .contains(&config.min_alert_interval_ms)
                {
                    storage_read_error.get_or_insert_with(|| {
                        format!("数据源 {id} 的最短提醒间隔无效，实例文件已保留")
                    });
                    config.min_alert_interval_ms = DEFAULT_MIN_ALERT_INTERVAL_MS;
                }
                config
                    .inputs
                    .retain(|key, _| definition.inputs.contains_key(key));
                for (key, input) in &definition.inputs {
                    if !config.inputs.contains_key(key) {
                        if let Some(value) = &input.default {
                            config.inputs.insert(key.clone(), value.clone());
                        }
                    }
                }
                let icon_warning = definition
                    .icon
                    .as_ref()
                    .and_then(|icon| validate_icon_data(icon).err());
                let state = states.get(&id).cloned().unwrap_or_default();
                let next_refresh = restored_next_refresh(now, &config, &state);
                sources.insert(
                    id.clone(),
                    ManagedSource {
                        definition,
                        config,
                        state,
                        compact: true,
                        next_refresh,
                        last_request: None,
                        in_flight: false,
                        generation: 0,
                        retry_count: 0,
                        force_refresh: false,
                        cancellation: None,
                        icon_warning,
                    },
                );
            }
        }
        let (sender, receiver) = mpsc::channel();
        Self {
            root,
            sources,
            sender,
            receiver,
            active_requests: 0,
            registered_icons: HashMap::new(),
            icon_textures: HashMap::new(),
            storage_read_error,
            storage_write_error: None,
            definition_errors,
        }
    }

    pub fn snapshots(&self) -> Vec<SourceSnapshot> {
        self.sources
            .iter()
            .map(|(id, source)| {
                let now_ms = timestamp_ms();
                let alert_status = alert_diagnostic(source, now_ms);
                SourceSnapshot {
                    id: id.clone(),
                    name: source.definition.name.clone(),
                    description: source.definition.description.clone(),
                    origin: source.config.origin.clone(),
                    enabled: source.config.enabled,
                    compact: source.compact,
                    alerts: source.config.alerts,
                    refresh_interval_ms: source.config.refresh_interval_ms,
                    min_alert_interval_ms: source.config.min_alert_interval_ms,
                    next_refresh_in_ms: source
                        .next_refresh
                        .saturating_duration_since(Instant::now())
                        .as_millis() as u64,
                    inputs: source.config.inputs.clone(),
                    input_schema: source.definition.inputs.clone(),
                    value_path: source.definition.value.path.clone(),
                    value: source.state.value,
                    formatted: source.state.formatted.clone(),
                    updated_ms: source.state.updated_ms,
                    last_request_ms: source.state.last_request_ms,
                    last_alert_ms: source.state.last_alert_ms,
                    alert_status,
                    last_error: source.state.last_error.clone(),
                    http_status: source.state.http_status,
                    in_flight: source.in_flight,
                    icon_warning: source.icon_warning.clone(),
                    alert_enabled: source.definition.alert.is_some(),
                    storage_error: self.storage_error().map(ToOwned::to_owned),
                }
            })
            .collect()
    }

    pub fn compact_ids(&self) -> Vec<ExtensionId> {
        self.sources
            .keys()
            .filter_map(|id| compact_id(id).ok())
            .collect()
    }

    pub fn storage_error(&self) -> Option<&str> {
        self.storage_read_error
            .as_deref()
            .or(self.storage_write_error.as_deref())
            .or_else(|| self.definition_errors.first().map(String::as_str))
    }

    pub fn compact_views(&self) -> Vec<CompactView> {
        self.sources
            .iter()
            .filter(|(_, source)| source.config.enabled && source.compact)
            .filter_map(|(id, source)| {
                let id = compact_id(id).ok()?;
                let icon_key = format!(
                    "datasource-icon/{}/{}",
                    source.definition.id,
                    icon_hash(source.definition.icon.as_ref())
                );
                let leading = if source.definition.icon.is_some() && source.icon_warning.is_none() {
                    ViewNode::new(crate::ui::declarative::Node::Image { asset: icon_key })
                } else {
                    ViewNode::icon("chart", 18.0)
                };
                let value = source.state.formatted.clone().unwrap_or_else(|| "—".into());
                let text = ViewNode::text(
                    value,
                    TextStyle {
                        font_size: FontSize::Custom(13.0),
                        font_weight: FontWeight::Bold,
                        wrap: false,
                        ..Default::default()
                    },
                )
                .color(ColorSpec::PrimaryText);
                let content = ViewNode::compact(
                    leading,
                    text,
                    CompactLayout {
                        leading_size: [18.0, 18.0],
                        gap: 5.0,
                        content_size: crate::ui::declarative::SizeSpec {
                            width: crate::ui::declarative::Size::Auto,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                );
                Some(CompactView {
                    id: id.clone(),
                    source_extension_id: id.clone(),
                    content,
                    visibility: Default::default(),
                    min_width: 48.0,
                    preferred_width: 78.0,
                    fit_width: true,
                    priority: 0,
                    tooltip: Some(source.definition.name.clone()),
                })
            })
            .collect()
    }

    pub fn register_icons(&mut self, context: &egui::Context) {
        for (id, source) in &mut self.sources {
            let Some(icon) = &source.definition.icon else {
                continue;
            };
            let signature = icon_hash(source.definition.icon.as_ref());
            if self
                .registered_icons
                .get(id)
                .is_some_and(|(previous, _)| *previous == signature)
            {
                continue;
            }
            let key = format!("datasource-icon/{id}/{signature}");
            let previous_key = self
                .registered_icons
                .get(id)
                .and_then(|(_, key)| key.clone());
            match decode_icon(context, &key, icon) {
                Ok(texture) => {
                    source.icon_warning = None;
                    if let Some(previous_key) = previous_key.filter(|old| old != &key) {
                        self.icon_textures.remove(&previous_key);
                    }
                    self.registered_icons
                        .insert(id.clone(), (signature, Some(key.clone())));
                    self.icon_textures.insert(key, texture);
                }
                Err(error) => {
                    source.icon_warning = Some(error.clone());
                    self.registered_icons.insert(id.clone(), (signature, None));
                }
            }
        }
    }

    pub fn icon_texture(&self, id: &str) -> Option<&egui::TextureHandle> {
        let (_, key) = self.registered_icons.get(id)?;
        self.icon_textures.get(key.as_ref()?)
    }

    pub fn import(
        &mut self,
        definition: Definition,
        allow_private_network: bool,
    ) -> Result<bool, String> {
        definition.validate()?;
        let id = definition.id.clone();
        let origin = definition.origin()?;
        let mut config = self.sources.get(&id).map_or_else(
            || new_config(&definition, origin.clone(), allow_private_network),
            |old| {
                let mut config = old.config.clone();
                config.origin = origin.clone();
                config.allow_private_network = allow_private_network;
                config.inputs.retain(|key, value| {
                    definition
                        .inputs
                        .get(key)
                        .is_some_and(|input| validate_runtime_input(input, value).is_ok())
                });
                for (key, input) in &definition.inputs {
                    if !config.inputs.contains_key(key) {
                        if let Some(default) = &input.default {
                            config.inputs.insert(key.clone(), default.clone());
                        }
                    }
                }
                config
            },
        );
        config.alerts &= definition.alert.is_some();
        let previous = self.sources.remove(&id);
        let replaced = previous.is_some();
        let in_flight = previous.as_ref().is_some_and(|source| source.in_flight);
        let generation = previous
            .as_ref()
            .map_or(0, |source| source.generation.wrapping_add(1));
        let state = previous.as_ref().map_or_else(SavedState::default, |old| {
            if old.definition.origin().ok() == Some(origin.clone())
                && old.definition.value.path == definition.value.path
            {
                old.state.clone()
            } else {
                SavedState {
                    last_alert_ms: old.state.last_alert_ms,
                    ..SavedState::default()
                }
            }
        });
        let compact = previous.as_ref().is_none_or(|old| old.compact);
        let icon_warning = definition
            .icon
            .as_ref()
            .and_then(|icon| validate_icon_data(icon).err());
        if let Some(token) = previous
            .as_ref()
            .and_then(|source| source.cancellation.as_ref())
        {
            token.cancel();
        }
        let now = Instant::now();
        self.sources.insert(
            id.clone(),
            ManagedSource {
                definition,
                config,
                state,
                compact,
                next_refresh: now,
                last_request: None,
                in_flight,
                generation,
                retry_count: 0,
                force_refresh: false,
                cancellation: None,
                icon_warning,
            },
        );
        if let Err(error) = self.save_all() {
            self.sources.remove(&id);
            if let Some(previous) = previous {
                self.sources.insert(id.clone(), previous);
            } else {
                let _ = fs::remove_file(self.definition_path(&id));
            }
            let _ = self.save_all();
            return Err(error);
        }
        self.definition_errors
            .retain(|error| !error.contains(&format!("{id}.pds.json")));
        Ok(replaced)
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        let source = self.source_mut(id)?;
        if source.config.enabled == enabled {
            return Ok(());
        }
        let previous = source.config.enabled;
        let previous_next_refresh = source.next_refresh;
        let previous_force_refresh = source.force_refresh;
        source.config.enabled = enabled;
        source.generation = source.generation.wrapping_add(1);
        if !enabled {
            source.force_refresh = false;
            if let Some(token) = &source.cancellation {
                token.cancel();
            }
        }
        source.next_refresh = Instant::now();
        if let Err(error) = self.save_all() {
            if let Some(source) = self.sources.get_mut(id) {
                source.config.enabled = previous;
                source.next_refresh = previous_next_refresh;
                source.force_refresh = previous_force_refresh;
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn set_compact(&mut self, id: &str, compact: bool) -> Result<(), String> {
        self.source_mut(id)?.compact = compact;
        Ok(())
    }

    pub fn set_alerts(&mut self, id: &str, alerts: bool) -> Result<(), String> {
        let source = self.source_mut(id)?;
        let alerts = alerts && source.definition.alert.is_some();
        if source.config.alerts == alerts {
            return Ok(());
        }
        let previous = source.config.alerts;
        let previous_next_refresh = source.next_refresh;
        source.config.alerts = alerts;
        source.generation = source.generation.wrapping_add(1);
        if let Some(token) = &source.cancellation {
            token.cancel();
        }
        source.next_refresh = Instant::now();
        if let Err(error) = self.save_all() {
            if let Some(source) = self.sources.get_mut(id) {
                source.config.alerts = previous;
                source.next_refresh = previous_next_refresh;
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn set_refresh_interval(&mut self, id: &str, interval: Duration) -> Result<(), String> {
        let millis = interval
            .as_millis()
            .try_into()
            .map_err(|_| "刷新间隔无效")?;
        if !(schema::MIN_REFRESH.as_millis() as u64..=MAX_REFRESH_INTERVAL_MS).contains(&millis) {
            return Err("刷新间隔必须在 30 秒到 24 小时之间".into());
        }
        let source = self.source_mut(id)?;
        let previous = source.config.refresh_interval_ms;
        let previous_next_refresh = source.next_refresh;
        source.config.refresh_interval_ms = millis;
        if !source.in_flight {
            source.next_refresh = Instant::now() + interval;
        }
        if let Err(error) = self.save_all() {
            if let Some(source) = self.sources.get_mut(id) {
                source.config.refresh_interval_ms = previous;
                source.next_refresh = previous_next_refresh;
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn set_min_alert_interval(&mut self, id: &str, interval: Duration) -> Result<(), String> {
        let millis = interval
            .as_millis()
            .try_into()
            .map_err(|_| "提醒间隔无效")?;
        if !(MIN_ALERT_INTERVAL_MS..=MAX_ALERT_INTERVAL_MS).contains(&millis) {
            return Err("最短提醒间隔必须在 1 分钟到 30 天之间".into());
        }
        let source = self.source_mut(id)?;
        let previous = source.config.min_alert_interval_ms;
        source.config.min_alert_interval_ms = millis;
        if let Err(error) = self.save_all() {
            if let Some(source) = self.sources.get_mut(id) {
                source.config.min_alert_interval_ms = previous;
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn set_input(&mut self, id: &str, key: &str, value: Value) -> Result<(), String> {
        let source = self.source_mut(id)?;
        let input = source
            .definition
            .inputs
            .get(key)
            .ok_or_else(|| "参数不存在".to_owned())?;
        validate_runtime_input(input, &value)?;
        let input_kind = input.kind;
        let previous = source.config.inputs.insert(key.to_owned(), value);
        let previous_next_refresh = source.next_refresh;
        let previous_force_refresh = source.force_refresh;
        let previous_retry_at = source.state.retry_at_ms;
        let previous_retry_count = source.retry_count;
        source.generation = source.generation.wrapping_add(1);
        if let Some(token) = &source.cancellation {
            token.cancel();
        }
        // A user edit should refresh immediately and drop any backoff caused by the previous
        // value, such as the "required parameter is empty" preflight failure. Disabled sources
        // keep the new value and request once they are enabled again.
        source.force_refresh = source.config.enabled;
        source.state.retry_at_ms = None;
        source.retry_count = 0;
        // String fields save on every keystroke, so wait briefly for typing to settle before
        // the forced request; other field types already commit on Enter/blur.
        source.next_refresh = Instant::now()
            + if input_kind == InputType::String {
                Duration::from_millis(350)
            } else {
                Duration::ZERO
            };
        if let Err(error) = self.save_all() {
            if let Some(source) = self.sources.get_mut(id) {
                if let Some(previous) = previous {
                    source.config.inputs.insert(key.to_owned(), previous);
                } else {
                    source.config.inputs.remove(key);
                }
                source.next_refresh = previous_next_refresh;
                source.force_refresh = previous_force_refresh;
                source.state.retry_at_ms = previous_retry_at;
                source.retry_count = previous_retry_count;
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        let source = self.sources.remove(id).ok_or("数据源不存在")?;
        if let Some(token) = &source.cancellation {
            token.cancel();
        }
        if let Err(error) = self.save_all() {
            self.sources.insert(id.to_owned(), source);
            let _ = self.save_all();
            return Err(error);
        }
        let path = self.definition_path(id);
        if let Err(error) = fs::remove_file(&path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                self.sources.insert(id.to_owned(), source);
                let _ = self.save_all();
                return Err(error.to_string());
            }
        }
        if let Some((_, Some(key))) = self.registered_icons.remove(id) {
            self.icon_textures.remove(&key);
        }
        Ok(())
    }

    pub fn next_wake(&self) -> Option<Instant> {
        if self.active_requests >= MAX_CONCURRENT_REQUESTS {
            return None;
        }
        self.sources
            .values()
            .filter(|source| {
                !source.in_flight
                    && (source.force_refresh
                        || (source.config.enabled && (source.compact || source.config.alerts)))
            })
            .map(|source| source.next_refresh)
            .min()
    }

    pub fn poll(
        &mut self,
        now: Instant,
        store: &SecretStore,
        context: &egui::Context,
    ) -> Vec<PendingAlert> {
        let mut notifications = Vec::new();
        let mut changed = false;
        while let Ok(finished) = self.receiver.try_recv() {
            self.active_requests = self.active_requests.saturating_sub(1);
            let Some(source) = self.sources.get_mut(&finished.id) else {
                continue;
            };
            if source.generation != finished.generation {
                source.in_flight = false;
                source.cancellation = None;
                continue;
            }
            source.in_flight = false;
            source.cancellation = None;
            source.last_request = Some(now);
            source.force_refresh = false;
            match finished.result {
                Ok(result) => {
                    source.state.value = Some(result.value);
                    source.state.formatted = Some(result.formatted);
                    source.state.updated_ms = Some(timestamp_ms());
                    source.state.last_error = None;
                    source.state.http_status = Some(result.status);
                    source.state.retry_at_ms = None;
                    source.retry_count = 0;
                    source.next_refresh = now + configured_interval(source);
                    if source.config.alerts {
                        if let Some(alert) = evaluate_alert(source, timestamp_ms()) {
                            notifications.push(alert);
                        }
                    }
                    changed = true;
                }
                Err(error) => {
                    source.state.last_error = Some(error.message);
                    source.state.http_status = error.status;
                    source.retry_count = source.retry_count.saturating_add(1);
                    if source.retry_count <= 3 {
                        let delay = retry_delay(source.retry_count, error.retry_after);
                        source.state.retry_at_ms =
                            Some(timestamp_ms().saturating_add(delay.as_millis() as u64));
                        source.next_refresh = now + delay;
                    } else {
                        source.retry_count = 0;
                        source.state.retry_at_ms = None;
                        source.next_refresh = now + configured_interval(source);
                    }
                    changed = true;
                }
            }
        }
        if changed {
            let _ = self.save_state();
        }

        let due: Vec<String> = self
            .sources
            .iter()
            .filter(|(_, source)| {
                !source.in_flight
                    && self.active_requests < MAX_CONCURRENT_REQUESTS
                    && (source.force_refresh
                        || (source.config.enabled
                            && (source.compact || source.config.alerts)
                            && source.next_refresh <= now))
            })
            .map(|(id, _)| id.clone())
            .take(MAX_CONCURRENT_REQUESTS.saturating_sub(self.active_requests))
            .collect();
        for id in due {
            if let Err(error) = self.start_request(&id, store.clone(), context.clone(), now) {
                if let Some(source) = self.sources.get_mut(&id) {
                    source.state.last_error = Some(error);
                    source.state.http_status = None;
                    source.force_refresh = false;
                    let retry_wait = source
                        .state
                        .retry_at_ms
                        .map_or(0, |retry_at| retry_at.saturating_sub(timestamp_ms()));
                    if retry_wait == 0 {
                        let delay = schema::MIN_REFRESH;
                        source.state.retry_at_ms =
                            Some(timestamp_ms().saturating_add(delay.as_millis() as u64));
                        source.next_refresh = now + delay;
                    } else {
                        source.next_refresh = now + Duration::from_millis(retry_wait);
                    }
                }
                let _ = self.save_state();
            }
        }
        notifications
    }

    fn start_request(
        &mut self,
        id: &str,
        store: SecretStore,
        context: egui::Context,
        now: Instant,
    ) -> Result<(), String> {
        let now_ms = timestamp_ms();
        let (task, generation, previous_last_request, previous_last_request_ms, previous_retry_at) = {
            let source = self.sources.get_mut(id).ok_or("数据源不存在")?;
            if !source.config.enabled {
                return Err("数据源已停用".into());
            }
            // Explicit parameter edits refresh immediately and may skip the local 30 s
            // rate limit; scheduled refreshes still respect it.
            if !source.force_refresh {
                if source
                    .last_request
                    .is_some_and(|last| last.elapsed() < schema::MIN_REFRESH)
                {
                    source.next_refresh = now + schema::MIN_REFRESH;
                    source.force_refresh = false;
                    return Err("请求频率受限，至少间隔 30 秒".into());
                }
                if let Some(last) = source.state.last_request_ms {
                    let wait_ms = minimum_request_wait_ms(last, now_ms);
                    if wait_ms > 0 {
                        source.next_refresh = now + Duration::from_millis(wait_ms);
                        source.force_refresh = false;
                        return Err("请求频率受限，至少间隔 30 秒".into());
                    }
                }
            }
            if let Some(retry_at) = source.state.retry_at_ms {
                let wait_ms = retry_at.saturating_sub(now_ms);
                if wait_ms > 0 {
                    source.next_refresh = now + Duration::from_millis(wait_ms);
                    source.force_refresh = false;
                    return Err("失败重试等待时间尚未结束".into());
                }
            }
            if source.config.origin != source.definition.origin()? {
                return Err("请求目标没有获得授权".into());
            }
            let mut inputs = source.config.inputs.clone();
            resolve_input_secret_refs(&mut inputs, &store)?;
            validate_required_inputs(&source.definition, &inputs)?;
            let cancellation = crate::sdk::tasks::CancellationToken::default();
            let task = RequestTask {
                definition: source.definition.clone(),
                inputs,
                origin: source.config.origin.clone(),
                allow_private_network: source.config.allow_private_network,
                cancellation: cancellation.clone(),
            };
            let previous_last_request = source.last_request;
            let previous_last_request_ms = source.state.last_request_ms;
            let previous_retry_at = source.state.retry_at_ms;
            source.in_flight = true;
            source.last_request = Some(now);
            source.state.last_request_ms = Some(now_ms);
            source.state.retry_at_ms = None;
            source.force_refresh = false;
            source.cancellation = Some(cancellation);
            (
                task,
                source.generation,
                previous_last_request,
                previous_last_request_ms,
                previous_retry_at,
            )
        };
        if let Err(error) = self.save_state() {
            if let Some(source) = self.sources.get_mut(id) {
                source.in_flight = false;
                source.last_request = previous_last_request;
                source.state.last_request_ms = previous_last_request_ms;
                source.state.retry_at_ms = previous_retry_at;
                source.cancellation = None;
            }
            return Err(error);
        }
        let sender = self.sender.clone();
        let request_id = id.to_owned();
        std::thread::spawn(move || {
            let result = execute_request(task);
            let _ = sender.send(FinishedRequest {
                id: request_id,
                generation,
                result,
            });
            context.request_repaint();
        });
        self.active_requests += 1;
        Ok(())
    }

    fn source_mut(&mut self, id: &str) -> Result<&mut ManagedSource, String> {
        self.sources
            .get_mut(id)
            .ok_or_else(|| "数据源不存在".to_owned())
    }

    fn definition_path(&self, id: &str) -> PathBuf {
        self.root.join("definitions").join(format!("{id}.pds.json"))
    }

    fn save_all(&mut self) -> Result<(), String> {
        if let Some(error) = &self.storage_read_error {
            return Err(format!("存储文件有错误，已保留原文件且暂停写入：{error}"));
        }
        let mut configs = BTreeMap::new();
        let mut states = BTreeMap::new();
        for (id, source) in &self.sources {
            configs.insert(id.clone(), source.config.clone());
            states.insert(id.clone(), source.state.clone());
        }
        let mut files = vec![
            json_file(&self.root.join("instances.json"), &configs)?,
            json_file(&self.root.join("state.json"), &states)?,
        ];
        for source in self.sources.values() {
            files.push(json_file(
                &self.definition_path(&source.definition.id),
                &source.definition,
            )?);
        }
        match write_batch_atomic(files) {
            Ok(()) => {
                self.storage_write_error = None;
                Ok(())
            }
            Err(error) => {
                self.storage_write_error = Some(error.clone());
                Err(error)
            }
        }
    }

    fn save_state(&mut self) -> Result<(), String> {
        if let Some(error) = &self.storage_read_error {
            return Err(format!("存储文件有错误，已保留原文件且暂停写入：{error}"));
        }
        let states = self
            .sources
            .iter()
            .map(|(id, source)| (id.clone(), source.state.clone()))
            .collect::<BTreeMap<_, _>>();
        let result = write_json_atomic(&self.root.join("state.json"), &states);
        if let Err(error) = &result {
            self.storage_write_error = Some(error.clone());
        } else {
            self.storage_write_error = None;
        }
        result
    }

    pub fn alert_accepted(
        &mut self,
        id: &str,
        notification_id: &str,
        alerted_at_ms: u64,
    ) -> Result<(), String> {
        let Some(source) = self.sources.get_mut(id) else {
            return Ok(());
        };
        if source
            .state
            .pending_alert
            .as_ref()
            .is_none_or(|pending| pending.notification_id != notification_id)
        {
            return Ok(());
        }
        source.state.last_alert_ms = Some(
            source
                .state
                .last_alert_ms
                .map_or(alerted_at_ms, |previous| previous.max(alerted_at_ms)),
        );
        source.state.pending_alert = None;
        self.save_state()
    }

    pub fn prepare_alert(
        &mut self,
        id: &str,
        notification_id: &str,
        alerted_at_ms: u64,
    ) -> Result<(), String> {
        let source = self.sources.get_mut(id).ok_or("数据源不存在")?;
        let previous = source.state.pending_alert.clone();
        source.state.pending_alert = Some(PendingAlertMarker {
            notification_id: notification_id.to_owned(),
            alerted_at_ms,
        });
        if let Err(error) = self.save_state() {
            if let Some(source) = self.sources.get_mut(id) {
                source.state.pending_alert = previous;
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn cancel_pending_alert(&mut self, id: &str, notification_id: &str) {
        if let Some(source) = self.sources.get_mut(id) {
            if source
                .state
                .pending_alert
                .as_ref()
                .is_some_and(|pending| pending.notification_id == notification_id)
            {
                source.state.pending_alert = None;
                let _ = self.save_state();
            }
        }
    }

    pub fn reconcile_alert_history(
        &mut self,
        accepted: &BTreeMap<String, u64>,
    ) -> Result<(), String> {
        let mut changed = false;
        for (id, source) in &mut self.sources {
            let Some(pending) = source.state.pending_alert.take() else {
                continue;
            };
            let key = format!("datasource.{id}:{}", pending.notification_id);
            if accepted.contains_key(&key) {
                source.state.last_alert_ms = Some(
                    source
                        .state
                        .last_alert_ms
                        .map_or(pending.alerted_at_ms, |last| {
                            last.max(pending.alerted_at_ms)
                        }),
                );
            }
            changed = true;
        }
        if changed {
            self.save_state()?;
        }
        Ok(())
    }
}

impl Drop for DataSourceManager {
    fn drop(&mut self) {
        for source in self.sources.values() {
            if let Some(token) = &source.cancellation {
                token.cancel();
            }
        }
    }
}

fn new_config(
    definition: &Definition,
    origin: String,
    allow_private_network: bool,
) -> SourceConfig {
    SourceConfig {
        enabled: true,
        alerts: definition.alert.is_some(),
        refresh_interval_ms: DEFAULT_REFRESH_INTERVAL_MS,
        min_alert_interval_ms: DEFAULT_MIN_ALERT_INTERVAL_MS,
        allow_private_network,
        origin,
        inputs: definition
            .inputs
            .iter()
            .filter_map(|(key, input)| input.default.clone().map(|value| (key.clone(), value)))
            .collect(),
        allow_plaintext_secrets: false,
        secret_refs: BTreeMap::new(),
    }
}

fn compact_id(id: &str) -> Result<ExtensionId, String> {
    ExtensionId::new(format!("{DATA_SOURCE_COMPACT_PREFIX}{id}/value"))
        .map_err(|_| "数据源 id 无效".to_owned())
}

fn icon_hash(icon: Option<&IconSpec>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    icon.hash(&mut hasher);
    hasher.finish()
}

impl std::hash::Hash for IconSpec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.mime.hash(state);
        self.base64.hash(state);
    }
}

fn decode_icon(
    context: &egui::Context,
    key: &str,
    icon: &IconSpec,
) -> Result<egui::TextureHandle, String> {
    let bytes = STANDARD
        .decode(icon.base64.trim())
        .map_err(|_| "图标 Base64 无法解码".to_owned())?;
    if bytes.len() > MAX_ICON_BYTES {
        return Err("图标超过 1 MB".into());
    }
    let pixels = match icon.mime.as_str() {
        "image/png" | "image/webp" => {
            let image = image::load_from_memory(&bytes).map_err(|_| "图片无法解码".to_owned())?;
            let (width, height) = image.dimensions();
            if u64::from(width) * u64::from(height) > MAX_ICON_PIXELS
                || width > 2048
                || height > 2048
            {
                return Err("图标像素数量超过限制".into());
            }
            egui::ColorImage::from_rgba_unmultiplied(
                [width as usize, height as usize],
                image.to_rgba8().as_raw(),
            )
        }
        "image/svg+xml" => {
            let source = std::str::from_utf8(&bytes).map_err(|_| "SVG 不是 UTF-8".to_owned())?;
            let lower = source.to_ascii_lowercase();
            if lower.contains("<text") {
                return Err("SVG 图标不能使用 <text>，文字不会被渲染".into());
            }
            if source.len() > 64_000
                || [
                    "<script",
                    "<foreignobject",
                    "href=",
                    "xlink:",
                    "url(",
                    "@import",
                    "<!doctype",
                    "<!entity",
                ]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                return Err("SVG 含有不允许的主动内容或外部资源".into());
            }
            egui_extras::image::load_svg_bytes_with_size(
                &bytes,
                Some(egui::SizeHint::Size(128, 128)),
            )
            .map_err(|_| "SVG 无法安全渲染".to_owned())?
        }
        _ => return Err("图标 MIME 类型不受支持".into()),
    };
    if pixels.size[0] > 2048
        || pixels.size[1] > 2048
        || pixels.size[0] * pixels.size[1] > MAX_ICON_PIXELS as usize
    {
        return Err("图标渲染尺寸超过限制".into());
    }
    let texture = context.load_texture(key, pixels, egui::TextureOptions::LINEAR);
    crate::ui::renderer::register_image(context, key, texture.clone());
    Ok(texture)
}

fn read_json<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> (T, Option<String>) {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(value) => (value, None),
            Err(error) => (
                T::default(),
                Some(format!("{} 无法解析：{error}", path.display())),
            ),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (T::default(), None),
        Err(error) => (
            T::default(),
            Some(format!("{} 无法读取：{error}", path.display())),
        ),
    }
}

fn json_file<T: Serialize>(path: &Path, value: &T) -> Result<(PathBuf, Vec<u8>), String> {
    serde_json::to_vec_pretty(value)
        .map(|bytes| (path.to_owned(), bytes))
        .map_err(|error| error.to_string())
}

fn write_batch_atomic(files: Vec<(PathBuf, Vec<u8>)>) -> Result<(), String> {
    struct StagedFile {
        path: PathBuf,
        temp: tempfile::NamedTempFile,
        previous: Option<Vec<u8>>,
    }
    let mut staged = Vec::with_capacity(files.len());
    for (path, bytes) in files {
        let parent = path.parent().ok_or("存储路径无效")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let previous = match fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.to_string()),
        };
        let mut temp =
            tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
        use std::io::Write;
        temp.write_all(&bytes).map_err(|error| error.to_string())?;
        temp.as_file()
            .sync_all()
            .map_err(|error| error.to_string())?;
        staged.push(StagedFile {
            path,
            temp,
            previous,
        });
    }
    let mut committed: Vec<(PathBuf, Option<Vec<u8>>)> = Vec::new();
    for file in staged {
        let StagedFile {
            path,
            temp,
            previous,
        } = file;
        if let Err(error) = temp.persist(&path) {
            let mut rollback_errors = Vec::new();
            for (path, previous) in committed.into_iter().rev() {
                let result = match previous {
                    Some(bytes) => write_bytes_atomic(&path, &bytes),
                    None => fs::remove_file(&path).map_err(|error| error.to_string()),
                };
                if let Err(error) = result {
                    rollback_errors.push(format!("{}: {error}", path.display()));
                }
            }
            let rollback = if rollback_errors.is_empty() {
                String::new()
            } else {
                format!("；回滚失败：{}", rollback_errors.join("；"))
            };
            return Err(format!(
                "保存 {} 失败：{}{rollback}",
                path.display(),
                error.error
            ));
        }
        committed.push((path, previous));
    }
    Ok(())
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("存储路径无效")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    use std::io::Write;
    temp.write_all(bytes).map_err(|error| error.to_string())?;
    temp.as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temp.persist(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    write_bytes_atomic(path, &bytes)
}

fn validate_icon_data(icon: &IconSpec) -> Result<(), String> {
    let bytes = STANDARD
        .decode(icon.base64.trim())
        .map_err(|_| "图标 Base64 无法解码".to_owned())?;
    if bytes.is_empty() || bytes.len() > MAX_ICON_BYTES {
        return Err("图标数据为空或超过 1 MB".into());
    }
    match icon.mime.as_str() {
        "image/png" | "image/webp" => {
            let image = image::load_from_memory(&bytes).map_err(|_| "图片无法解码".to_owned())?;
            let (width, height) = image.dimensions();
            if u64::from(width) * u64::from(height) > MAX_ICON_PIXELS
                || width > 2048
                || height > 2048
            {
                return Err("图标像素数量超过限制".into());
            }
        }
        "image/svg+xml" => {
            let source = std::str::from_utf8(&bytes).map_err(|_| "SVG 不是 UTF-8".to_owned())?;
            let lower = source.to_ascii_lowercase();
            if lower.contains("<text") {
                return Err("SVG 图标不能使用 <text>，文字不会被渲染".into());
            }
            if source.len() > 64_000
                || !lower.contains("<svg")
                || [
                    "<script",
                    "<foreignobject",
                    "href=",
                    "xlink:",
                    "url(",
                    "@import",
                    "<!doctype",
                    "<!entity",
                ]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                return Err("SVG 无效，含主动内容或外部资源".into());
            }
        }
        _ => return Err("图标 MIME 类型不受支持".into()),
    }
    Ok(())
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn validate_runtime_input(input: &InputDefinition, value: &Value) -> Result<(), String> {
    let kind_ok = match input.kind {
        InputType::String => value.is_string(),
        InputType::Number => value.as_f64().is_some_and(f64::is_finite),
        InputType::Integer => value.as_i64().is_some(),
        InputType::Boolean => value.is_boolean(),
        InputType::Enum => value
            .as_str()
            .is_some_and(|value| input.options.iter().any(|option| option == value)),
        InputType::Duration => value
            .as_str()
            .is_some_and(|value| schema::parse_duration(value).is_ok()),
    };
    if !kind_ok {
        return Err("参数值类型不匹配".into());
    }
    if input.required && value.as_str().is_some_and(|value| value.trim().is_empty()) {
        return Err("必填参数不能为空".into());
    }
    if let Some(number) = input_numeric_value(value, input.kind) {
        let min = input
            .min
            .as_ref()
            .and_then(|v| input_numeric_value(v, input.kind));
        let max = input
            .max
            .as_ref()
            .and_then(|v| input_numeric_value(v, input.kind));
        if min.is_some_and(|min| number < min) || max.is_some_and(|max| number > max) {
            return Err("参数值超出范围".into());
        }
    }
    Ok(())
}

fn resolve_input_secret_refs(
    inputs: &mut BTreeMap<String, Value>,
    store: &SecretStore,
) -> Result<(), String> {
    replace_input_secret_refs(inputs, |name| {
        store.get(name).map_err(|_| "Secret 存储不可用".to_owned())
    })
}

fn replace_input_secret_refs(
    inputs: &mut BTreeMap<String, Value>,
    mut lookup: impl FnMut(&str) -> Result<Option<String>, String>,
) -> Result<(), String> {
    for value in inputs.values_mut() {
        let Some(name) = value
            .as_str()
            .and_then(|text| text.strip_prefix('$'))
            .and_then(|text| text.strip_suffix('$'))
            .filter(|name| !name.is_empty() && !name.contains('$'))
        else {
            continue;
        };
        let secret = lookup(name)?.ok_or_else(|| format!("找不到 Secret：{name}"))?;
        *value = Value::String(secret);
    }
    Ok(())
}

fn validate_required_inputs(
    definition: &Definition,
    inputs: &BTreeMap<String, Value>,
) -> Result<(), String> {
    for (key, input) in &definition.inputs {
        let value = inputs.get(key).or(input.default.as_ref());
        if input.required && value.is_none() {
            return Err(format!("请填写必填参数：{}", input.label));
        }
        if let Some(value) = value {
            validate_runtime_input(input, value)
                .map_err(|error| format!("{}：{error}", input.label))?;
        }
    }
    Ok(())
}

struct RequestTask {
    definition: Definition,
    inputs: BTreeMap<String, Value>,
    origin: String,
    allow_private_network: bool,
    cancellation: crate::sdk::tasks::CancellationToken,
}

fn execute_request(task: RequestTask) -> Result<FetchResult, FetchError> {
    if task.cancellation.is_cancelled() {
        return Err(fetch_error("请求已取消"));
    }
    let request = &task.definition.request;
    let url_template = expand_string(&request.url, &task.inputs, true)?;
    let mut url = Url::parse(&url_template).map_err(|_| fetch_error("请求 URL 无效"))?;
    if url.origin().ascii_serialization() != task.origin {
        return Err(fetch_error("请求目标与授权 Origin 不匹配"));
    }
    if url.scheme() != "https" && !task.allow_private_network {
        return Err(fetch_error("HTTP 请求需要单独授权"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| fetch_error("请求目标无效"))?
        .to_owned();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| fetch_error("请求端口无效"))?;
    let addresses: Vec<SocketAddr> = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|_| fetch_error("无法解析请求目标"))?
        .collect();
    if addresses.is_empty() {
        return Err(fetch_error("请求目标没有可用地址"));
    }
    if !task.allow_private_network && addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(fetch_error("请求目标解析到了私有或本机网络"));
    }
    for (key, value) in &request.query {
        let value = expand_string(value, &task.inputs, false)?;
        url.query_pairs_mut().append_pair(key, &value);
    }
    let timeout = Duration::from_millis(
        request
            .timeout_ms
            .min(schema::MAX_TIMEOUT.as_millis() as u64),
    );
    let builder = reqwest::blocking::Client::builder()
        .connect_timeout(timeout.min(Duration::from_secs(10)))
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(&host, &addresses);
    // reqwest's default builder honors the system proxy configuration used by SDK requests.
    let client = builder
        .build()
        .map_err(|_| fetch_error("无法创建 HTTP 客户端"))?;
    let method = match request.method {
        HttpMethod::Get => reqwest::Method::GET,
        HttpMethod::Post => reqwest::Method::POST,
        HttpMethod::Put => reqwest::Method::PUT,
        HttpMethod::Patch => reqwest::Method::PATCH,
    };
    let mut request_builder = client.request(method, url);
    for (name, value) in &request.headers {
        let value = expand_string(value, &task.inputs, false)?;
        request_builder = request_builder.header(name, value);
    }
    if let Some(body) = &request.body {
        let body = expand_json(body, &task.inputs)?;
        request_builder = request_builder.json(&body);
    }
    if let Some(body) = &request.body_text {
        let body = expand_string(body, &task.inputs, false)?;
        request_builder = request_builder.body(body);
    }
    let mut response = request_builder.send().map_err(|error| {
        fetch_error(if error.is_timeout() {
            "HTTP 请求超时"
        } else {
            "HTTP 请求失败"
        })
    })?;
    if task.cancellation.is_cancelled() {
        return Err(fetch_error("请求已取消"));
    }
    let status = response.status();
    let retry_after = if status.as_u16() == 429 {
        response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs)
    } else {
        None
    };
    if !status.is_success() {
        return Err(FetchError {
            message: format!("HTTP 状态码 {}", status.as_u16()),
            status: Some(status.as_u16()),
            retry_after,
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(fetch_error("HTTP 响应超过 1 MB"));
    }
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| fetch_error("无法读取 HTTP 响应"))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(fetch_error("HTTP 响应超过 1 MB"));
    }
    let json: Value =
        serde_json::from_slice(&bytes).map_err(|_| fetch_error("响应不是合法 JSON"))?;
    let mut selector = jsonpath_lib::Selector::new();
    selector
        .str_path(&task.definition.value.path)
        .map_err(|_| fetch_error("JSONPath 无效"))?;
    selector.value(&json);
    let values = selector
        .select()
        .map_err(|_| fetch_error("JSONPath 解析失败"))?;
    if values.len() != 1 {
        return Err(fetch_error("JSONPath 必须匹配唯一数值"));
    }
    let raw = values[0]
        .as_f64()
        .or_else(|| {
            values[0]
                .as_str()
                .and_then(|value| value.trim().parse::<f64>().ok())
        })
        .filter(|value| value.is_finite())
        .ok_or_else(|| fetch_error("提取结果不是有限数值"))?;
    let spec = &task.definition.value;
    let value = raw * spec.multiply / spec.divide + spec.offset;
    if !value.is_finite() {
        return Err(fetch_error("数值转换结果无效"));
    }
    Ok(FetchResult {
        value,
        formatted: format_value(value, spec),
        status: status.as_u16(),
    })
}

fn fetch_error(message: &str) -> FetchError {
    FetchError {
        message: message.into(),
        status: None,
        retry_after: None,
    }
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            let shared_address = octets[0] == 100 && (64..=127).contains(&octets[1]);
            let special_protocol = octets[0] == 192 && octets[1] == 0 && octets[2] == 0;
            let documentation = (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113);
            let benchmarking = octets[0] == 198 && (octets[1] == 18 || octets[1] == 19);
            let relay_anycast = octets[0] == 192 && octets[1] == 88 && octets[2] == 99;
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.octets()[0] == 0
                || ip.octets()[0] >= 224
                || shared_address
                || special_protocol
                || documentation
                || benchmarking
                || relay_anycast)
        }
        IpAddr::V6(ip) => {
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (ip.segments()[0] & 0xfe00) == 0xfc00
                || (ip.segments()[0] & 0xffc0) == 0xfe80
                || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8)
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|v4| !is_public_ip(IpAddr::V4(v4))))
        }
    }
}

fn expand_string(
    text: &str,
    inputs: &BTreeMap<String, Value>,
    url_encode: bool,
) -> Result<String, FetchError> {
    let mut output = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}').ok_or_else(|| fetch_error("占位符无效"))?;
        let token = &after[..end];
        let value = resolve_token(token, inputs)?;
        let value = match value {
            Value::String(value) => value,
            value => value.to_string(),
        };
        if url_encode {
            output.extend(url::form_urlencoded::byte_serialize(value.as_bytes()));
        } else {
            output.push_str(&value);
        }
        rest = &after[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

fn expand_json(value: &Value, inputs: &BTreeMap<String, Value>) -> Result<Value, FetchError> {
    match value {
        Value::String(text)
            if text.starts_with("${") && text.ends_with('}') && text.matches("${").count() == 1 =>
        {
            resolve_token(&text[2..text.len() - 1], inputs)
        }
        Value::String(text) => expand_string(text, inputs, false).map(Value::String),
        Value::Array(values) => values
            .iter()
            .map(|value| expand_json(value, inputs))
            .collect(),
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), expand_json(value, inputs)?)))
            .collect::<Result<Map<_, _>, FetchError>>()
            .map(Value::Object),
        _ => Ok(value.clone()),
    }
}

fn resolve_token(token: &str, inputs: &BTreeMap<String, Value>) -> Result<Value, FetchError> {
    if let Some(name) = token.strip_prefix("input.") {
        return inputs
            .get(name)
            .cloned()
            .ok_or_else(|| fetch_error("必填参数未填写"));
    }
    Err(fetch_error("不支持的请求占位符"))
}

fn format_value(value: f64, spec: &ValueDefinition) -> String {
    let fixed = format!("{:.*}", usize::from(spec.decimals), value);
    let (integer, fraction) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let grouped = if spec.separator {
        let negative = integer.starts_with('-');
        let digits = integer.trim_start_matches('-');
        let mut parts = Vec::new();
        for end in (0..digits.len()).step_by(3) {
            let start = digits.len().saturating_sub(end + 3);
            let stop = digits.len() - end;
            parts.push(&digits[start..stop]);
        }
        let joined = parts.into_iter().rev().collect::<Vec<_>>().join(",");
        if negative {
            format!("-{joined}")
        } else {
            joined
        }
    } else {
        integer.to_owned()
    };
    if fraction.is_empty() {
        format!("{}{grouped}{}", spec.prefix, spec.suffix)
    } else {
        format!("{}{grouped}.{fraction}{}", spec.prefix, spec.suffix)
    }
}

fn configured_interval(source: &ManagedSource) -> Duration {
    Duration::from_millis(source.config.refresh_interval_ms)
}

fn minimum_request_wait_ms(last_request_ms: u64, now_ms: u64) -> u64 {
    (schema::MIN_REFRESH.as_millis() as u64).saturating_sub(now_ms.saturating_sub(last_request_ms))
}

fn restored_next_refresh(now: Instant, config: &SourceConfig, state: &SavedState) -> Instant {
    let now_ms = timestamp_ms();
    let regular_wait = state.updated_ms.map_or(1_000, |last_success| {
        config
            .refresh_interval_ms
            .saturating_sub(now_ms.saturating_sub(last_success))
    });
    let rate_limit_wait = state
        .last_request_ms
        .map_or(0, |last| minimum_request_wait_ms(last, now_ms));
    let retry_wait = state
        .retry_at_ms
        .map_or(0, |retry_at| retry_at.saturating_sub(now_ms));
    now + Duration::from_millis(regular_wait.max(rate_limit_wait).max(retry_wait))
}

fn retry_delay(retry_count: u8, retry_after: Option<Duration>) -> Duration {
    let exponent = u32::from(retry_count.saturating_sub(1).min(10));
    let backoff = Duration::from_secs(5_u64.saturating_mul(2_u64.pow(exponent)));
    retry_after
        .unwrap_or(backoff)
        .max(Duration::from_secs(1))
        .min(Duration::from_secs(3600))
}

fn evaluate_alert(source: &ManagedSource, now_ms: u64) -> Option<PendingAlert> {
    let alert = source.definition.alert.as_ref()?;
    let value = source.state.value?;
    let threshold = alert_threshold(source, alert)?;
    if !should_notify(
        value,
        threshold,
        alert.operator,
        source.state.last_alert_ms,
        source.config.min_alert_interval_ms,
        now_ms,
    ) {
        return None;
    }
    let formatted = source.state.formatted.as_deref().unwrap_or("—");
    let title = expand_alert_template(&alert.title, formatted, &source.config.inputs);
    let body = expand_alert_template(&alert.message, formatted, &source.config.inputs);
    let source_id = ExtensionId::new(format!("datasource.{}", source.definition.id)).ok()?;
    Some(PendingAlert {
        source_id: source.definition.id.clone(),
        alerted_at_ms: now_ms,
        notification: Notification {
            source_id,
            id: format!("alert-{now_ms}"),
            title,
            body,
            icon: None,
            priority: NotificationPriority::Normal,
            actions: vec![],
            created_at: UNIX_EPOCH + Duration::from_millis(now_ms),
        },
    })
}

fn alert_threshold(source: &ManagedSource, alert: &schema::AlertDefinition) -> Option<f64> {
    if let Some(number) = alert.threshold.as_f64() {
        return Some(number);
    }
    let token = alert
        .threshold
        .as_str()?
        .strip_prefix("${input.")?
        .strip_suffix('}')?;
    source
        .config
        .inputs
        .get(token)
        .or(source.definition.inputs.get(token)?.default.as_ref())?
        .as_f64()
}

fn compare_alert(value: f64, threshold: f64, operator: AlertOperator) -> bool {
    match operator {
        AlertOperator::Gt => value > threshold,
        AlertOperator::Gte => value >= threshold,
        AlertOperator::Lt => value < threshold,
        AlertOperator::Lte => value <= threshold,
        AlertOperator::Eq => value == threshold,
        AlertOperator::Ne => value != threshold,
    }
}

fn should_notify(
    value: f64,
    threshold: f64,
    operator: AlertOperator,
    last_alert_ms: Option<u64>,
    min_interval_ms: u64,
    now_ms: u64,
) -> bool {
    compare_alert(value, threshold, operator)
        && last_alert_ms.is_none_or(|last| now_ms.saturating_sub(last) >= min_interval_ms)
}

fn alert_diagnostic(source: &ManagedSource, now_ms: u64) -> String {
    let Some(alert) = &source.definition.alert else {
        return "未配置提醒".into();
    };
    if !source.config.alerts {
        return "提醒已关闭".into();
    }
    let Some(value) = source.state.value else {
        return "数据失效，暂不能判断".into();
    };
    if source.state.last_error.is_some() {
        return "最近请求失败，数据已失效，暂不能判断".into();
    }
    let Some(threshold) = alert_threshold(source, alert) else {
        return "阈值参数无效".into();
    };
    if !compare_alert(value, threshold, alert.operator) {
        return "当前数值未达到阈值".into();
    }
    if let Some(last) = source.state.last_alert_ms {
        let elapsed = now_ms.saturating_sub(last);
        if elapsed < source.config.min_alert_interval_ms {
            let remaining = source.config.min_alert_interval_ms - elapsed;
            return format!("已达到阈值，冷却剩余 {}", duration_text(remaining));
        }
    }
    "已达到阈值，可以再次提醒".into()
}

fn duration_text(millis: u64) -> String {
    let seconds = millis.div_ceil(1000);
    if seconds >= 86_400 {
        format!("{} 天 {} 小时", seconds / 86_400, seconds % 86_400 / 3600)
    } else if seconds >= 3600 {
        format!("{} 小时 {} 分", seconds / 3600, seconds % 3600 / 60)
    } else if seconds >= 60 {
        format!("{} 分 {} 秒", seconds / 60, seconds % 60)
    } else {
        format!("{seconds} 秒")
    }
}

fn value_to_string(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), ToOwned::to_owned)
}

fn expand_alert_template(template: &str, value: &str, inputs: &BTreeMap<String, Value>) -> String {
    let mut expanded = template.replace("${value}", value);
    for (key, input) in inputs {
        expanded = expanded.replace(&format!("${{input.{key}}}"), &value_to_string(input));
    }
    expanded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_input_secret_reference_is_resolved_only_for_the_request() {
        let mut inputs = BTreeMap::from([
            ("token".to_owned(), Value::String("$API_KEY$".into())),
            ("label".to_owned(), Value::String("price $5".into())),
        ]);
        replace_input_secret_refs(&mut inputs, |name| {
            assert_eq!(name, "API_KEY");
            Ok(Some("private-value".into()))
        })
        .unwrap();
        assert_eq!(inputs["token"], "private-value");
        assert_eq!(inputs["label"], "price $5");
    }

    fn definition() -> Definition {
        Definition::parse(include_str!(
            "../../../examples/datasources/shanghai-temperature.pds.json"
        ))
        .unwrap()
    }

    fn managed_source() -> ManagedSource {
        let definition = definition();
        let mut config = new_config(&definition, definition.origin().unwrap(), false);
        config
            .inputs
            .insert("alert_threshold".into(), Value::from(10));
        ManagedSource {
            definition,
            config,
            state: SavedState {
                value: Some(11.0),
                formatted: Some("11.0 °C".into()),
                ..SavedState::default()
            },
            compact: true,
            next_refresh: Instant::now(),
            last_request: None,
            in_flight: false,
            generation: 0,
            retry_count: 0,
            force_refresh: false,
            cancellation: None,
            icon_warning: None,
        }
    }

    #[test]
    fn text_based_svg_icons_are_rejected() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text x="1" y="1">D</text></svg>"#;
        let icon = IconSpec {
            mime: "image/svg+xml".into(),
            base64: STANDARD.encode(svg),
        };
        let error = validate_icon_data(&icon).unwrap_err();
        assert!(error.contains("<text>"), "{error}");
    }

    #[test]
    fn editing_an_input_requests_an_immediate_refresh() {
        let root = tempfile::tempdir().unwrap();
        let mut manager = DataSourceManager::open(root.path());
        manager.import(definition(), false).unwrap();
        {
            let source = manager
                .sources
                .get_mut("open-meteo-shanghai-temperature")
                .unwrap();
            source.state.retry_at_ms = Some(u64::MAX);
            source.retry_count = 3;
            source.next_refresh = Instant::now() + Duration::from_secs(300);
        }
        manager
            .set_input(
                "open-meteo-shanghai-temperature",
                "alert_threshold",
                Value::from(12),
            )
            .unwrap();
        let source = manager
            .sources
            .get("open-meteo-shanghai-temperature")
            .unwrap();
        assert!(source.force_refresh);
        assert!(source.state.retry_at_ms.is_none());
        assert_eq!(source.retry_count, 0);
        assert!(source.next_refresh <= Instant::now());

        manager
            .set_enabled("open-meteo-shanghai-temperature", false)
            .unwrap();
        manager
            .set_input(
                "open-meteo-shanghai-temperature",
                "alert_threshold",
                Value::from(13),
            )
            .unwrap();
        assert!(
            !manager
                .sources
                .get("open-meteo-shanghai-temperature")
                .unwrap()
                .force_refresh
        );
    }

    #[test]
    fn alert_cooldown_uses_threshold_and_last_timestamp_only() {
        let operator = AlertOperator::Gte;
        assert!(should_notify(10.0, 10.0, operator, None, 100, 0));
        assert!(!should_notify(9.9, 10.0, operator, None, 100, 1000));
        assert!(!should_notify(11.0, 10.0, operator, Some(1000), 100, 1099));
        assert!(should_notify(11.0, 10.0, operator, Some(1000), 100, 1100));
        assert!(!should_notify(11.0, 10.0, operator, Some(2000), 100, 1500));
    }

    #[test]
    fn retry_backoff_is_bounded_and_retry_after_takes_priority() {
        assert_eq!(retry_delay(1, None), Duration::from_secs(5));
        assert_eq!(retry_delay(3, None), Duration::from_secs(20));
        assert_eq!(
            retry_delay(1, Some(Duration::from_secs(120))),
            Duration::from_secs(120)
        );
        assert_eq!(
            retry_delay(1, Some(Duration::from_secs(7200))),
            Duration::from_secs(3600)
        );
    }

    #[test]
    fn restored_schedule_respects_refresh_rate_limit_and_retry_after() {
        let mut config = new_config(&definition(), "https://example.com".into(), false);
        config.refresh_interval_ms = 300_000;
        let now_ms = timestamp_ms();
        let state = SavedState {
            updated_ms: Some(now_ms.saturating_sub(60_000)),
            last_request_ms: Some(now_ms.saturating_sub(10_000)),
            retry_at_ms: Some(now_ms + 120_000),
            ..SavedState::default()
        };
        let remaining = restored_next_refresh(Instant::now(), &config, &state)
            .saturating_duration_since(Instant::now());
        assert!((Duration::from_secs(239)..=Duration::from_secs(241)).contains(&remaining));
    }

    #[test]
    fn cancelled_request_does_not_start_network_work() {
        let cancellation = crate::sdk::tasks::CancellationToken::default();
        cancellation.cancel();
        let error = execute_request(RequestTask {
            definition: definition(),
            inputs: BTreeMap::new(),
            origin: String::new(),
            allow_private_network: false,
            cancellation,
        })
        .unwrap_err();
        assert_eq!(error.message, "请求已取消");
    }

    #[test]
    fn creating_pending_alert_does_not_consume_cooldown() {
        let source = managed_source();
        let alert = evaluate_alert(&source, 5_000).unwrap();
        assert_eq!(alert.alerted_at_ms, 5_000);
        assert_eq!(alert.notification.id, "alert-5000");
        assert_eq!(source.state.last_alert_ms, None);
    }

    #[test]
    fn instance_settings_and_alert_time_survive_changes_and_restart() {
        let root = tempfile::tempdir().unwrap();
        {
            let mut manager = DataSourceManager::open(root.path());
            manager.import(definition(), false).unwrap();
            manager
                .prepare_alert("open-meteo-shanghai-temperature", "alert-test", 1_000_000)
                .unwrap();
            manager
                .alert_accepted("open-meteo-shanghai-temperature", "alert-test", 1_000_000)
                .unwrap();
            manager
                .set_refresh_interval("open-meteo-shanghai-temperature", Duration::from_secs(120))
                .unwrap();
            assert!(manager
                .set_refresh_interval("open-meteo-shanghai-temperature", Duration::from_secs(20))
                .is_err());
            manager
                .set_min_alert_interval(
                    "open-meteo-shanghai-temperature",
                    Duration::from_secs(1800),
                )
                .unwrap();
            manager
                .set_input(
                    "open-meteo-shanghai-temperature",
                    "alert_threshold",
                    Value::from(8),
                )
                .unwrap();
            manager
                .set_alerts("open-meteo-shanghai-temperature", false)
                .unwrap();
            manager
                .set_alerts("open-meteo-shanghai-temperature", true)
                .unwrap();
            manager
                .set_enabled("open-meteo-shanghai-temperature", false)
                .unwrap();
            manager
                .set_enabled("open-meteo-shanghai-temperature", true)
                .unwrap();
            let mut updated = definition();
            updated.description = "重新导入后的描述".into();
            manager.import(updated, false).unwrap();
        }
        let manager = DataSourceManager::open(root.path());
        let source = manager.snapshots().remove(0);
        assert_eq!(source.refresh_interval_ms, 120_000);
        assert_eq!(source.min_alert_interval_ms, 1_800_000);
        assert_eq!(source.last_alert_ms, Some(1_000_000));
        assert!(source.alerts);
        assert!(source.enabled);
    }

    #[test]
    fn interrupted_alert_commit_recovers_only_from_its_matching_history_entry() {
        let root = tempfile::tempdir().unwrap();
        let source_id = "open-meteo-shanghai-temperature";
        let notification_id = "alert-900000";
        {
            let mut manager = DataSourceManager::open(root.path());
            manager.import(definition(), false).unwrap();
            manager
                .prepare_alert(source_id, notification_id, 900_000)
                .unwrap();
        }
        let mut manager = DataSourceManager::open(root.path());
        let history =
            BTreeMap::from([(format!("datasource.{source_id}:{notification_id}"), 900_000)]);
        manager.reconcile_alert_history(&history).unwrap();
        assert_eq!(manager.snapshots()[0].last_alert_ms, Some(900_000));

        manager.delete(source_id).unwrap();
        manager.import(definition(), false).unwrap();
        manager.reconcile_alert_history(&history).unwrap();
        assert_eq!(manager.snapshots()[0].last_alert_ms, None);
    }

    #[test]
    fn malformed_storage_is_reported_and_never_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let storage = root.path().join("data-sources/instances.json");
        fs::create_dir_all(storage.parent().unwrap()).unwrap();
        fs::write(&storage, b"not json").unwrap();
        let mut manager = DataSourceManager::open(root.path());
        assert!(manager.import(definition(), false).is_err());
        assert_eq!(fs::read(storage).unwrap(), b"not json");
    }

    #[test]
    fn ignored_old_definition_is_preserved_without_blocking_a_new_import() {
        let root = tempfile::tempdir().unwrap();
        let definitions = root.path().join("data-sources/definitions");
        fs::create_dir_all(&definitions).unwrap();
        let old_file = definitions.join("old-source.pds.json");
        fs::write(
            &old_file,
            br#"{"schema":"peninsula.datasource/v1","request":{"interval":"5m"}}"#,
        )
        .unwrap();
        let old_bytes = fs::read(&old_file).unwrap();
        let mut manager = DataSourceManager::open(root.path());
        assert!(manager.storage_error().is_some());
        manager.import(definition(), false).unwrap();
        manager
            .set_refresh_interval("open-meteo-shanghai-temperature", Duration::from_secs(120))
            .unwrap();
        assert_eq!(fs::read(old_file).unwrap(), old_bytes);
        assert_eq!(manager.snapshots().len(), 1);
    }
}
