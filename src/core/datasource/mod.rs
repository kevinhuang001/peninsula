mod schema;

use crate::{
    extension::{CompactView, ExtensionId, Notification, NotificationPriority, ViewNode},
    sdk::secrets::SecretStore,
    ui::declarative::{ColorSpec, CompactLayout, FontSize, FontWeight, TextStyle},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use eframe::egui;
use image::GenericImageView;
use schema::{duration_value, input_numeric_value};
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceConfig {
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_true")]
    compact: bool,
    #[serde(default)]
    alerts: bool,
    #[serde(default)]
    allow_private_network: bool,
    #[serde(default)]
    allow_plaintext_secrets: bool,
    #[serde(default)]
    origin: String,
    #[serde(default)]
    inputs: BTreeMap<String, Value>,
    #[serde(default)]
    secret_refs: BTreeMap<String, String>,
}

fn default_true() -> bool {
    true
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
    last_error: Option<String>,
    #[serde(default)]
    http_status: Option<u16>,
    #[serde(default)]
    alert_match: Option<bool>,
    #[serde(default)]
    last_alert_ms: Option<u64>,
}

struct ManagedSource {
    definition: Definition,
    config: SourceConfig,
    state: SavedState,
    next_refresh: Instant,
    last_request: Option<Instant>,
    in_flight: bool,
    generation: u64,
    retry_count: u8,
    force_refresh: bool,
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
    pub inputs: BTreeMap<String, Value>,
    pub secret_refs: BTreeMap<String, String>,
    pub input_schema: BTreeMap<String, InputDefinition>,
    pub value: Option<f64>,
    pub formatted: Option<String>,
    pub updated_ms: Option<u64>,
    pub last_error: Option<String>,
    pub http_status: Option<u16>,
    pub in_flight: bool,
    pub icon_warning: Option<String>,
    pub alert_enabled: bool,
    pub allow_plaintext_secrets: bool,
}

struct FinishedRequest {
    id: String,
    generation: u64,
    result: Result<FetchResult, FetchError>,
}

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
}

impl DataSourceManager {
    pub fn open(data_dir: &Path) -> Self {
        let root = data_dir.join("data-sources");
        let _ = fs::create_dir_all(root.join("definitions"));
        let configs: BTreeMap<String, SourceConfig> = read_json(&root.join("instances.json"));
        let states: BTreeMap<String, SavedState> = read_json(&root.join("state.json"));
        let now = Instant::now();
        let mut sources = BTreeMap::new();
        if let Ok(entries) = fs::read_dir(root.join("definitions")) {
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
                    continue;
                };
                let Ok(definition) = Definition::parse(&text) else {
                    continue;
                };
                if entry.file_name().to_string_lossy() != format!("{}.pds.json", definition.id) {
                    continue;
                }
                let id = definition.id.clone();
                let mut config = configs.get(&id).cloned().unwrap_or_else(|| {
                    new_config(
                        &definition,
                        definition.origin().unwrap_or_default(),
                        false,
                        false,
                    )
                });
                config.inputs.retain(|key, _| {
                    definition
                        .inputs
                        .get(key)
                        .is_some_and(|input| input.kind != InputType::Secret)
                });
                config.secret_refs.retain(|key, _| {
                    definition
                        .inputs
                        .get(key)
                        .is_some_and(|input| input.kind == InputType::Secret)
                });
                for (key, input) in &definition.inputs {
                    if input.kind != InputType::Secret && !config.inputs.contains_key(key) {
                        if let Some(value) = &input.default {
                            config.inputs.insert(key.clone(), value.clone());
                        }
                    }
                }
                sources.insert(
                    id.clone(),
                    ManagedSource {
                        definition,
                        config,
                        state: states.get(&id).cloned().unwrap_or_default(),
                        next_refresh: now + Duration::from_secs(1),
                        last_request: None,
                        in_flight: false,
                        generation: 0,
                        retry_count: 0,
                        force_refresh: false,
                        icon_warning: None,
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
        }
    }

    pub fn snapshots(&self) -> Vec<SourceSnapshot> {
        self.sources
            .iter()
            .map(|(id, source)| SourceSnapshot {
                id: id.clone(),
                name: source.definition.name.clone(),
                description: source.definition.description.clone(),
                origin: source.config.origin.clone(),
                enabled: source.config.enabled,
                compact: source.config.compact,
                alerts: source.config.alerts,
                inputs: source.config.inputs.clone(),
                secret_refs: source.config.secret_refs.clone(),
                input_schema: source.definition.inputs.clone(),
                value: source.state.value,
                formatted: source.state.formatted.clone(),
                updated_ms: source.state.updated_ms,
                last_error: source.state.last_error.clone(),
                http_status: source.state.http_status,
                in_flight: source.in_flight,
                icon_warning: source.icon_warning.clone(),
                alert_enabled: source.definition.alert.is_some(),
                allow_plaintext_secrets: source.config.allow_plaintext_secrets,
            })
            .collect()
    }

    pub fn compact_ids(&self) -> Vec<ExtensionId> {
        self.sources
            .keys()
            .filter_map(|id| compact_id(id).ok())
            .collect()
    }

    pub fn compact_views(&self) -> Vec<CompactView> {
        self.sources
            .iter()
            .filter(|(_, source)| source.config.enabled && source.config.compact)
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
        allow_plaintext_secrets: bool,
        store: &SecretStore,
    ) -> Result<bool, String> {
        definition.validate()?;
        let id = definition.id.clone();
        let origin = definition.origin()?;
        let mut config = self.sources.get(&id).map_or_else(
            || {
                new_config(
                    &definition,
                    origin.clone(),
                    allow_private_network,
                    allow_plaintext_secrets,
                )
            },
            |old| {
                let mut config = old.config.clone();
                config.origin = origin.clone();
                config.allow_private_network = allow_private_network;
                config.allow_plaintext_secrets = allow_plaintext_secrets;
                config.inputs.retain(|key, value| {
                    definition.inputs.get(key).is_some_and(|input| {
                        input.kind != InputType::Secret
                            && validate_runtime_input(input, value).is_ok()
                    })
                });
                config.secret_refs.retain(|key, _| {
                    definition
                        .inputs
                        .get(key)
                        .is_some_and(|input| input.kind == InputType::Secret)
                });
                for (key, input) in &definition.inputs {
                    if input.kind != InputType::Secret && !config.inputs.contains_key(key) {
                        if let Some(default) = &input.default {
                            config.inputs.insert(key.clone(), default.clone());
                        }
                    }
                }
                config
            },
        );
        config.alerts &= definition.alert.is_some();
        let removed_secrets: Vec<String> = self
            .sources
            .get(&id)
            .map(|old| {
                old.definition
                    .secret_inputs()
                    .into_iter()
                    .filter(|key| {
                        definition
                            .inputs
                            .get(key)
                            .is_none_or(|input| input.kind != InputType::Secret)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let removed_secret_values = removed_secrets
            .iter()
            .map(|key| {
                store
                    .get(&secret_key(&id, key))
                    .map(|value| (key.clone(), value))
                    .map_err(|error| format!("无法读取待清理的 Secret：{error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let previous = self.sources.remove(&id);
        let replaced = previous.is_some();
        let in_flight = previous.as_ref().is_some_and(|source| source.in_flight);
        let generation = previous
            .as_ref()
            .map_or(0, |source| source.generation.wrapping_add(1));
        let mut state = previous
            .as_ref()
            .filter(|old| {
                old.definition.origin().ok() == Some(origin.clone())
                    && old.definition.value.path == definition.value.path
            })
            .map(|old| old.state.clone())
            .unwrap_or_default();
        state.alert_match = None;
        state.last_alert_ms = None;
        let now = Instant::now();
        self.sources.insert(
            id.clone(),
            ManagedSource {
                definition,
                config,
                state,
                next_refresh: now,
                last_request: None,
                in_flight,
                generation,
                retry_count: 0,
                force_refresh: false,
                icon_warning: None,
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
        for key in &removed_secrets {
            if let Err(error) = store.delete(&secret_key(&id, key)) {
                self.sources.remove(&id);
                if let Some(previous) = previous {
                    self.sources.insert(id.clone(), previous);
                }
                let mut rollback_errors = Vec::new();
                if let Err(rollback_error) = self.save_all() {
                    rollback_errors.push(format!("恢复数据源失败：{rollback_error}"));
                }
                for (key, value) in &removed_secret_values {
                    if let Some(value) = value {
                        if let Err(restore_error) = store.set(&secret_key(&id, key), value) {
                            rollback_errors.push(format!("恢复 Secret 失败：{restore_error}"));
                        }
                    }
                }
                let detail = if rollback_errors.is_empty() {
                    String::new()
                } else {
                    format!("；{}", rollback_errors.join("；"))
                };
                return Err(format!("无法清除已移除的 Secret：{error}{detail}"));
            }
        }
        Ok(replaced)
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        let source = self.source_mut(id)?;
        source.config.enabled = enabled;
        source.generation = source.generation.wrapping_add(1);
        source.state.alert_match = None;
        source.state.last_alert_ms = None;
        source.next_refresh = Instant::now();
        self.save_all()
    }

    pub fn set_compact(&mut self, id: &str, compact: bool) -> Result<(), String> {
        self.source_mut(id)?.config.compact = compact;
        self.save_all()
    }

    pub fn set_alerts(&mut self, id: &str, alerts: bool) -> Result<(), String> {
        let source = self.source_mut(id)?;
        source.config.alerts = alerts && source.definition.alert.is_some();
        source.state.alert_match = None;
        source.state.last_alert_ms = None;
        source.next_refresh = Instant::now();
        self.save_all()
    }

    pub fn set_plaintext_secret_fallback(&mut self, id: &str, allow: bool) -> Result<(), String> {
        self.source_mut(id)?.config.allow_plaintext_secrets = allow;
        self.save_all()
    }

    pub fn set_input(&mut self, id: &str, key: &str, value: Value) -> Result<(), String> {
        let source = self.source_mut(id)?;
        let input = source
            .definition
            .inputs
            .get(key)
            .filter(|input| input.kind != InputType::Secret)
            .ok_or_else(|| "参数不存在或为 Secret".to_owned())?;
        validate_runtime_input(input, &value)?;
        if input_duration_reference(&source.definition.request.interval) == Some(key)
            && schema::parse_duration(value.as_str().unwrap_or_default())? < schema::MIN_REFRESH
        {
            return Err("刷新间隔不能少于 30 秒".into());
        }
        if source
            .definition
            .alert
            .as_ref()
            .and_then(|alert| alert.repeat_interval.as_deref())
            .and_then(input_duration_reference)
            == Some(key)
            && schema::parse_duration(value.as_str().unwrap_or_default())?
                < Duration::from_secs(300)
        {
            return Err("重复提醒间隔不能少于 5 分钟".into());
        }
        source.config.inputs.insert(key.to_owned(), value);
        source.state.alert_match = None;
        source.state.last_alert_ms = None;
        source.next_refresh = Instant::now();
        self.save_all()
    }

    pub fn set_secret(
        &mut self,
        store: &SecretStore,
        id: &str,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        let source = self.sources.get(id).ok_or("数据源不存在")?;
        if source
            .definition
            .inputs
            .get(key)
            .is_none_or(|input| input.kind != InputType::Secret)
        {
            return Err("Secret 参数不存在".into());
        }
        if store
            .backend(&secret_key(id, key))
            .map_err(|error| error.to_string())?
            == Some(crate::sdk::secrets::SecretBackend::Unavailable)
            && !source.config.allow_plaintext_secrets
        {
            return Err(
                "系统 Keyring 不可用。重新导入此数据源并明确授权本地回退存储后，才能保存 Secret。"
                    .into(),
            );
        }
        store
            .set(&secret_key(id, key), value)
            .map_err(|error| error.to_string())?;
        let source = self.sources.get_mut(id).ok_or("数据源不存在")?;
        source.config.secret_refs.remove(key);
        self.save_all()
    }

    pub fn delete_secret(
        &mut self,
        store: &SecretStore,
        id: &str,
        key: &str,
    ) -> Result<(), String> {
        let source = self.sources.get(id).ok_or("数据源不存在")?;
        if source
            .definition
            .inputs
            .get(key)
            .is_none_or(|input| input.kind != InputType::Secret)
        {
            return Err("Secret 参数不存在".into());
        }
        if source.config.secret_refs.contains_key(key) {
            store
                .delete(&secret_key(id, key))
                .map_err(|error| error.to_string())?;
            let source = self.sources.get_mut(id).ok_or("数据源不存在")?;
            source.config.secret_refs.remove(key);
            return self.save_all();
        }
        store
            .delete(&secret_key(id, key))
            .map_err(|error| error.to_string())
    }

    pub fn secret_exists(&self, store: &SecretStore, id: &str, key: &str) -> Result<bool, String> {
        let source = self.sources.get(id).ok_or("数据源不存在")?;
        let storage_key = source
            .config
            .secret_refs
            .get(key)
            .map_or_else(|| secret_key(id, key), ToOwned::to_owned);
        store
            .get(&storage_key)
            .map(|value| value.is_some())
            .map_err(|error| error.to_string())
    }

    pub fn set_secret_reference(
        &mut self,
        store: &SecretStore,
        id: &str,
        input_key: &str,
        secret_name: Option<&str>,
    ) -> Result<(), String> {
        let source = self.sources.get(id).ok_or("数据源不存在")?;
        if source
            .definition
            .inputs
            .get(input_key)
            .is_none_or(|input| input.kind != InputType::Secret)
        {
            return Err("Secret 参数不存在".into());
        }
        if let Some(secret_name) = secret_name {
            if secret_name.trim().is_empty()
                || store
                    .get(secret_name)
                    .map_err(|error| error.to_string())?
                    .is_none()
            {
                return Err("找不到同名的已保存 Secret".into());
            }
        }
        let source = self.sources.get_mut(id).ok_or("数据源不存在")?;
        if let Some(secret_name) = secret_name {
            store
                .delete(&secret_key(id, input_key))
                .map_err(|error| error.to_string())?;
            source
                .config
                .secret_refs
                .insert(input_key.to_owned(), secret_name.to_owned());
        } else {
            source.config.secret_refs.remove(input_key);
        }
        self.save_all()
    }

    pub fn request_now(&mut self, id: &str) -> Result<(), String> {
        let source = self.source_mut(id)?;
        if source.in_flight {
            return Ok(());
        }
        if source
            .last_request
            .is_some_and(|last| last.elapsed() < schema::MIN_REFRESH)
        {
            return Err("此数据源刚刚请求过，请稍后再试".into());
        }
        source.force_refresh = true;
        source.next_refresh = Instant::now();
        Ok(())
    }

    pub fn delete(&mut self, id: &str) -> Result<Vec<String>, String> {
        let source = self.sources.remove(id).ok_or("数据源不存在")?;
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
        Ok(source.definition.secret_inputs())
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
                        || (source.config.enabled
                            && (source.config.compact || source.config.alerts)))
            })
            .map(|source| source.next_refresh)
            .min()
    }

    pub fn poll(
        &mut self,
        now: Instant,
        store: &SecretStore,
        context: &egui::Context,
    ) -> Vec<Notification> {
        let mut notifications = Vec::new();
        let mut changed = false;
        while let Ok(finished) = self.receiver.try_recv() {
            self.active_requests = self.active_requests.saturating_sub(1);
            let Some(source) = self.sources.get_mut(&finished.id) else {
                continue;
            };
            if source.generation != finished.generation {
                source.in_flight = false;
                continue;
            }
            source.in_flight = false;
            source.last_request = Some(now);
            source.force_refresh = false;
            match finished.result {
                Ok(result) => {
                    source.state.value = Some(result.value);
                    source.state.formatted = Some(result.formatted);
                    source.state.updated_ms = Some(timestamp_ms());
                    source.state.last_error = None;
                    source.state.http_status = Some(result.status);
                    source.retry_count = 0;
                    source.next_refresh = now + configured_interval(source);
                    if source.config.alerts {
                        if let Some(notification) = evaluate_alert(source) {
                            notifications.push(notification);
                        }
                    }
                    changed = true;
                }
                Err(error) => {
                    source.state.last_error = Some(error.message);
                    source.state.http_status = error.status;
                    source.retry_count = source.retry_count.saturating_add(1);
                    if source.retry_count <= 3 {
                        let backoff = Duration::from_secs(
                            5_u64.saturating_mul(2_u64.pow(u32::from(source.retry_count - 1))),
                        );
                        let delay = error
                            .retry_after
                            .unwrap_or(backoff)
                            .max(Duration::from_secs(1))
                            .min(Duration::from_secs(3600));
                        source.next_refresh = now + delay;
                    } else {
                        source.retry_count = 0;
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
                            && (source.config.compact || source.config.alerts)
                            && source.next_refresh <= now))
            })
            .map(|(id, _)| id.clone())
            .take(MAX_CONCURRENT_REQUESTS.saturating_sub(self.active_requests))
            .collect();
        for id in due {
            if let Err(error) = self.start_request(&id, store.clone(), context.clone(), now) {
                if let Some(source) = self.sources.get_mut(&id) {
                    source.state.last_error = Some(error);
                    source.force_refresh = false;
                    source.next_refresh = now + Duration::from_secs(30);
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
        let source = self.sources.get_mut(id).ok_or("数据源不存在")?;
        if source.config.origin != source.definition.origin()? {
            return Err("请求目标没有获得授权".into());
        }
        let inputs = source.config.inputs.clone();
        let secrets = load_secrets(&source.definition, &source.config, &store, id)?;
        validate_required_inputs(&source.definition, &inputs, &secrets)?;
        let task = RequestTask {
            definition: source.definition.clone(),
            inputs,
            secrets,
            origin: source.config.origin.clone(),
            allow_private_network: source.config.allow_private_network,
        };
        source.in_flight = true;
        source.last_request = Some(now);
        source.force_refresh = false;
        let sender = self.sender.clone();
        let request_id = id.to_owned();
        let generation = source.generation;
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

    fn save_all(&self) -> Result<(), String> {
        let mut configs = BTreeMap::new();
        let mut states = BTreeMap::new();
        for (id, source) in &self.sources {
            configs.insert(id.clone(), source.config.clone());
            states.insert(id.clone(), source.state.clone());
        }
        write_json_atomic(&self.root.join("instances.json"), &configs)?;
        write_json_atomic(&self.root.join("state.json"), &states)?;
        for source in self.sources.values() {
            write_json_atomic(
                &self.definition_path(&source.definition.id),
                &source.definition,
            )?;
        }
        Ok(())
    }

    fn save_state(&self) -> Result<(), String> {
        let states = self
            .sources
            .iter()
            .map(|(id, source)| (id.clone(), source.state.clone()))
            .collect::<BTreeMap<_, _>>();
        write_json_atomic(&self.root.join("state.json"), &states)
    }
}

fn new_config(
    definition: &Definition,
    origin: String,
    allow_private_network: bool,
    allow_plaintext_secrets: bool,
) -> SourceConfig {
    SourceConfig {
        enabled: true,
        compact: true,
        alerts: definition.alert.is_some(),
        allow_private_network,
        allow_plaintext_secrets,
        origin,
        inputs: definition
            .inputs
            .iter()
            .filter_map(|(key, input)| {
                (input.kind != InputType::Secret)
                    .then(|| input.default.clone().map(|value| (key.clone(), value)))
                    .flatten()
            })
            .collect(),
        secret_refs: BTreeMap::new(),
    }
}

fn compact_id(id: &str) -> Result<ExtensionId, String> {
    ExtensionId::new(format!("{DATA_SOURCE_COMPACT_PREFIX}{id}/value"))
        .map_err(|_| "数据源 id 无效".to_owned())
}

fn secret_key(id: &str, key: &str) -> String {
    format!("datasource::{id}::{key}")
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

fn read_json<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path.parent().ok_or("存储路径无效")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(&mut file, value).map_err(|error| error.to_string())?;
    file.as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    file.persist(path)
        .map_err(|error| error.error.to_string())?;
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
        InputType::Secret => false,
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

fn load_secrets(
    definition: &Definition,
    config: &SourceConfig,
    store: &SecretStore,
    id: &str,
) -> Result<BTreeMap<String, String>, String> {
    let mut secrets = BTreeMap::new();
    for key in definition.secret_inputs() {
        let storage_key = config
            .secret_refs
            .get(&key)
            .cloned()
            .unwrap_or_else(|| secret_key(id, &key));
        let value = store
            .get(&storage_key)
            .map_err(|_| "Secret 存储不可用".to_owned())?;
        match value {
            Some(value) => {
                secrets.insert(key, value);
            }
            None if definition.inputs[&key].required => {
                return Err(format!(
                    "缺少必填 Secret：{}",
                    definition.inputs[&key].label
                ));
            }
            None => {}
        }
    }
    Ok(secrets)
}

fn validate_required_inputs(
    definition: &Definition,
    inputs: &BTreeMap<String, Value>,
    secrets: &BTreeMap<String, String>,
) -> Result<(), String> {
    for (key, input) in &definition.inputs {
        if input.kind == InputType::Secret {
            if input.required && !secrets.contains_key(key) {
                return Err(format!("缺少必填 Secret：{}", input.label));
            }
            continue;
        }
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
    secrets: BTreeMap<String, String>,
    origin: String,
    allow_private_network: bool,
}

fn execute_request(task: RequestTask) -> Result<FetchResult, FetchError> {
    let request = &task.definition.request;
    let url_template = expand_string(&request.url, &task.inputs, &task.secrets, true)?;
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
        let value = expand_string(value, &task.inputs, &task.secrets, false)?;
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
        let value = expand_string(value, &task.inputs, &task.secrets, false)?;
        request_builder = request_builder.header(name, value);
    }
    if let Some(body) = &request.body {
        let body = expand_json(body, &task.inputs, &task.secrets)?;
        request_builder = request_builder.json(&body);
    }
    if let Some(body) = &request.body_text {
        let body = expand_string(body, &task.inputs, &task.secrets, false)?;
        request_builder = request_builder.body(body);
    }
    let mut response = request_builder
        .send()
        .map_err(|_| fetch_error("HTTP 请求失败"))?;
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
    secrets: &BTreeMap<String, String>,
    url_encode: bool,
) -> Result<String, FetchError> {
    let mut output = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}').ok_or_else(|| fetch_error("占位符无效"))?;
        let token = &after[..end];
        let value = resolve_token(token, inputs, secrets)?;
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

fn expand_json(
    value: &Value,
    inputs: &BTreeMap<String, Value>,
    secrets: &BTreeMap<String, String>,
) -> Result<Value, FetchError> {
    match value {
        Value::String(text)
            if text.starts_with("${") && text.ends_with('}') && text.matches("${").count() == 1 =>
        {
            resolve_token(&text[2..text.len() - 1], inputs, secrets)
        }
        Value::String(text) => expand_string(text, inputs, secrets, false).map(Value::String),
        Value::Array(values) => values
            .iter()
            .map(|value| expand_json(value, inputs, secrets))
            .collect(),
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), expand_json(value, inputs, secrets)?)))
            .collect::<Result<Map<_, _>, FetchError>>()
            .map(Value::Object),
        _ => Ok(value.clone()),
    }
}

fn resolve_token(
    token: &str,
    inputs: &BTreeMap<String, Value>,
    secrets: &BTreeMap<String, String>,
) -> Result<Value, FetchError> {
    if let Some(name) = token.strip_prefix("input.") {
        return inputs
            .get(name)
            .cloned()
            .ok_or_else(|| fetch_error("必填参数未填写"));
    }
    if let Some(name) = token.strip_prefix("secret.") {
        return secrets
            .get(name)
            .cloned()
            .map(Value::String)
            .ok_or_else(|| fetch_error("Secret 不可用"));
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
    let value = &source.definition.request.interval;
    if let Some(name) = value
        .strip_prefix("${input.")
        .and_then(|value| value.strip_suffix('}'))
    {
        if let Some(value) = source.config.inputs.get(name).and_then(Value::as_str) {
            return schema::parse_duration(value)
                .unwrap_or(Duration::from_secs(300))
                .max(schema::MIN_REFRESH);
        }
    }
    duration_value(value, &source.definition)
        .unwrap_or(Duration::from_secs(300))
        .max(schema::MIN_REFRESH)
}

fn input_duration_reference(template: &str) -> Option<&str> {
    template
        .strip_prefix("${input.")
        .and_then(|value| value.strip_suffix('}'))
}

fn evaluate_alert(source: &mut ManagedSource) -> Option<Notification> {
    let alert = source.definition.alert.as_ref()?;
    let value = source.state.value?;
    let threshold = if let Some(number) = alert.threshold.as_f64() {
        number
    } else {
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
            .as_f64()?
    };
    let matches = match alert.operator {
        AlertOperator::Gt => value > threshold,
        AlertOperator::Gte => value >= threshold,
        AlertOperator::Lt => value < threshold,
        AlertOperator::Lte => value <= threshold,
        AlertOperator::Eq => value == threshold,
        AlertOperator::Ne => value != threshold,
    };
    let previous = source.state.alert_match.replace(matches);
    if !matches {
        return None;
    }
    let now = timestamp_ms();
    let should_notify = match previous {
        None => alert.notify_on_initial,
        Some(false) => true,
        Some(true) => alert.repeat_interval.as_ref().is_some_and(|interval| {
            let interval = resolve_interval_input(interval, source);
            source
                .state
                .last_alert_ms
                .is_some_and(|last| now.saturating_sub(last) >= interval.as_millis() as u64)
        }),
    };
    if !should_notify {
        if previous.is_none() && matches {
            source.state.last_alert_ms = Some(now);
        }
        return None;
    }
    source.state.last_alert_ms = Some(now);
    let formatted = source.state.formatted.as_deref().unwrap_or("—");
    let title = expand_alert_template(&alert.title, formatted, &source.config.inputs);
    let body = expand_alert_template(&alert.message, formatted, &source.config.inputs);
    let source_id = ExtensionId::new(format!("datasource.{}", source.definition.id)).ok()?;
    Some(Notification {
        source_id,
        id: format!("alert-{now}"),
        title,
        body,
        icon: None,
        priority: NotificationPriority::Normal,
        actions: vec![],
        created_at: SystemTime::now(),
    })
}

fn resolve_interval_input(interval: &str, source: &ManagedSource) -> Duration {
    if let Some(name) = interval
        .strip_prefix("${input.")
        .and_then(|value| value.strip_suffix('}'))
    {
        if let Some(value) = source.config.inputs.get(name).and_then(Value::as_str) {
            return schema::parse_duration(value).unwrap_or(Duration::from_secs(3600));
        }
        if let Some(value) = source
            .definition
            .inputs
            .get(name)
            .and_then(|input| input.default.as_ref())
            .and_then(Value::as_str)
        {
            return schema::parse_duration(value).unwrap_or(Duration::from_secs(3600));
        }
    }
    schema::parse_duration(interval).unwrap_or(Duration::from_secs(3600))
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
