pub mod bilibili;
pub mod download;
pub mod kuaidi;
pub mod performance;
pub mod pomodoro;
pub mod weather;

use crate::ui::declarative::{RefreshHint, WidgetBounds};
use crate::{
    core::{notifications, settings::SettingsStore, views::ViewCache},
    extension::{
        Action, ExtensionContext, ExtensionId, ExtensionKind, ExtensionLifecycleRecord,
        ExtensionOutput, ExtensionRegistry, LifecycleManager, Notification, NotificationAction,
        NotificationPresentation, ReloadPolicy, RuntimeExtension, Settings, ViewNode, WidgetView,
    },
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

mod extras;

fn compact_text(text: impl Into<String>) -> ViewNode {
    use crate::ui::declarative::*;
    ViewNode::text(
        text,
        TextStyle {
            font_size: FontSize::Small,
            ..Default::default()
        },
    )
}

fn icon_text(
    icon: &str,
    text: impl Into<String>,
    color: crate::ui::declarative::ColorSpec,
    size: f32,
) -> ViewNode {
    use crate::ui::declarative::*;
    ViewNode::row(
        vec![ViewNode::icon(icon, size), compact_text(text)],
        Flow {
            gap: Some(4.0),
            ..Default::default()
        },
    )
    .color(color)
}

pub fn registry() -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::default();
    for descriptor in [notifications::descriptor()]
        .into_iter()
        .chain(performance::descriptors())
        .chain(download::descriptors())
        .chain(pomodoro::descriptors())
        .chain(kuaidi::descriptors())
        .chain(
            std::iter::once(bilibili::BilibiliExtension::default())
                .flat_map(|extension| extension.descriptors()),
        )
        .chain(weather::descriptors())
        .chain(extras::descriptors())
    {
        registry
            .register(descriptor)
            .expect("built-in extension descriptors must be valid");
    }
    registry
}

/// All feature implementations are registered through this host collection.
/// It routes by extension IDs and keeps feature logic out of App and Capsule.
pub struct BuiltinExtensions {
    registry: ExtensionRegistry,
    extensions: Vec<Box<dyn RuntimeExtension>>,
    lifecycle: LifecycleManager,
    context: ExtensionContext,
    owners: HashMap<ExtensionId, usize>,
    settings: SettingsStore,
    effective_settings: HashMap<ExtensionId, Settings>,
    startup_output: Vec<ExtensionOutput>,
    widget_cache: ViewCache<ExtensionId, Option<Arc<WidgetView>>>,
    compact_cache: ViewCache<usize, Vec<crate::extension::CompactView>>,
}

impl BuiltinExtensions {
    pub fn new(mut legacy: HashMap<ExtensionId, Settings>) -> Self {
        let context = ExtensionContext::new(user_data_dir(), user_config_dir());
        let mut host = Self {
            registry: registry(),
            extensions: vec![
                Box::new(performance::PerformanceExtension::new()),
                Box::new(download::DownloadExtension::new()),
                Box::new(pomodoro::PomodoroExtension::default()),
                Box::new(bilibili::BilibiliExtension::default()),
                Box::new(kuaidi::KuaidiExtension::default()),
                Box::new(weather::WeatherExtension::new(
                    weather::WeatherPreferences::default(),
                )),
                Box::new(extras::CompactExtras),
            ],
            lifecycle: LifecycleManager::load(Some(lifecycle_path())),
            settings: SettingsStore::open(context.config_dir().join("extension-settings.json")),
            context,
            owners: HashMap::new(),
            effective_settings: HashMap::new(),
            startup_output: vec![],
            widget_cache: ViewCache::default(),
            compact_cache: ViewCache::default(),
        };
        for (index, extension) in host.extensions.iter_mut().enumerate() {
            for descriptor in extension.descriptors() {
                host.owners.insert(descriptor.id.clone(), index);
                if descriptor.settings.is_empty() {
                    continue;
                }
                let seed = legacy.remove(&descriptor.id).unwrap_or_default();
                let settings = host
                    .settings
                    .load(&descriptor, seed.clone(), |version, values| {
                        extension.migrate_settings(&descriptor.id, version, values)
                    })
                    .unwrap_or_else(|error| {
                        host.context
                            .log(&descriptor.id, &format!("Unable to load settings: {error}"));
                        let mut values = SettingsStore::defaults(&descriptor);
                        values.extend(seed.into_iter().filter(|(key, value)| {
                            descriptor
                                .settings
                                .iter()
                                .any(|field| field.key == *key && field.validate(value).is_ok())
                        }));
                        values
                    });
                host.startup_output.push(extension.on_settings_changed(
                    &host.context,
                    &descriptor.id,
                    &settings,
                    ReloadPolicy::Live,
                ));
                host.effective_settings
                    .insert(descriptor.id.clone(), settings);
            }
        }
        host.start_enabled_extensions();
        host
    }

    pub fn settings(&self, id: &ExtensionId) -> Option<&Settings> {
        self.effective_settings.get(id)
    }

    pub fn update_settings(
        &mut self,
        id: &ExtensionId,
        values: Settings,
    ) -> Result<ExtensionOutput, String> {
        let (Some(&owner), Some(descriptor)) = (self.owners.get(id), self.registry.get(id)) else {
            return Err(format!("settings owner {id} is not registered"));
        };
        let changed = self.settings.update(descriptor, values)?;
        let effective = self.effective_settings.entry(id.clone()).or_default();
        let saved = self.settings.values(id).expect("settings just saved");
        let mut policy = None;
        for (key, reload) in changed {
            if reload != ReloadPolicy::Restart {
                effective.insert(key.clone(), saved[&key].clone());
                if policy.is_none() || reload == ReloadPolicy::Reload {
                    policy = Some(reload);
                }
            }
        }
        let Some(policy) = policy else {
            return Ok(ExtensionOutput::default());
        };
        let mut output =
            self.extensions[owner].on_settings_changed(&self.context, id, effective, policy);
        output.invalidated.push(id.clone());
        Ok(output)
    }

    pub fn invalidate_views(&mut self, id: &ExtensionId) {
        self.widget_cache.invalidate(id);
        if let Some(owner) = self.owners.get(id) {
            self.compact_cache.invalidate(owner);
        }
    }

    fn start_enabled_extensions(&mut self) {
        for extension in &mut self.extensions {
            let descriptors = extension.descriptors();
            if let Some(descriptor) = descriptors.into_iter().find(|item| item.default_enabled) {
                let previous = self.lifecycle.record(&descriptor.id).cloned();
                let result =
                    if let Some(record) = previous.as_ref().filter(|record| record.initialized) {
                        if record.version != descriptor.version {
                            extension.on_upgrade(&self.context, &record.version)
                        } else {
                            Ok(())
                        }
                    } else {
                        extension
                            .on_install(&self.context)
                            .and_then(|()| extension.on_enable(&self.context))
                    };
                match result {
                    Ok(()) => {
                        let record = ExtensionLifecycleRecord {
                            version: descriptor.version.clone(),
                            enabled: true,
                            initialized: true,
                            last_result: "on_start:ok".into(),
                            last_failure: None,
                        };
                        if let Err(error) = extension.on_start(&self.context) {
                            self.context
                                .log(&descriptor.id, &format!("on_start failed: {error}"));
                            let mut record = record;
                            record.last_result = "on_start:failed".into();
                            record.last_failure = Some(error);
                            if let Err(save_error) =
                                self.lifecycle.set(descriptor.id.clone(), record)
                            {
                                eprintln!("Unable to save extension lifecycle: {save_error}");
                            }
                            continue;
                        }
                        if let Err(error) = self.lifecycle.set(descriptor.id, record) {
                            eprintln!("Unable to save extension lifecycle: {error}");
                        }
                    }
                    Err(error) => {
                        self.context
                            .log(&descriptor.id, &format!("lifecycle hook failed: {error}"));
                        let record = ExtensionLifecycleRecord {
                            version: previous.as_ref().map_or_else(
                                || descriptor.version.clone(),
                                |record| record.version.clone(),
                            ),
                            enabled: false,
                            initialized: previous.as_ref().is_some_and(|record| record.initialized),
                            last_result: "startup:failed".into(),
                            last_failure: Some(error),
                        };
                        if let Err(save_error) = self.lifecycle.set(descriptor.id, record) {
                            eprintln!("Unable to save extension lifecycle: {save_error}");
                        }
                    }
                }
            }
        }
    }

    pub fn apply_weather_preferences(&mut self, preferences: &weather::WeatherPreferences) {
        for extension in &mut self.extensions {
            let Some(any) = extension.as_any_mut() else {
                continue;
            };
            if let Some(weather) = any.downcast_mut::<weather::WeatherExtension>() {
                if weather.set_preferences(preferences) {
                    self.invalidate_views(&ExtensionId::builtin(weather::WIDGET_ID));
                    self.invalidate_views(&ExtensionId::builtin(weather::COMPACT_ID));
                }
                return;
            }
        }
    }

    pub fn widget_ids(&self) -> Vec<ExtensionId> {
        self.registry
            .widget_list()
            .into_iter()
            .map(|descriptor| descriptor.id.clone())
            .collect()
    }

    pub fn widget_size(
        &self,
        id: &ExtensionId,
        view: Option<&WidgetView>,
        host_limit: [f32; 2],
        content_size: [Option<f32>; 2],
    ) -> [f32; 2] {
        let descriptor = self.registry.get(id);
        let preferred = descriptor.map_or([300.0, 76.0], |d| d.size_hint);
        let limits = descriptor.map_or_else(WidgetBounds::default, |d| d.widget_bounds);
        let requested = view.map_or(limits, |view| view.bounds);
        let resizable = limits.resizable || requested.resizable;
        std::array::from_fn(|i| {
            let upper = [limits.max[i], requested.max[i]]
                .into_iter()
                .flatten()
                .filter(|v| v.is_finite() && *v >= 0.0)
                .fold(host_limit[i].max(0.0), f32::min);
            let lower = [limits.min[i], requested.min[i]]
                .into_iter()
                .filter(|v| v.is_finite())
                .fold(0.0, f32::max)
                .min(upper);
            let default = preferred[i].is_finite() && preferred[i] > 0.0;
            let runtime_hint = view
                .map(|v| v.preferred_size[i])
                .filter(|v| v.is_finite() && *v > 0.0);
            let size = if let Some(content) = content_size[i] {
                content
            } else if default && !resizable {
                preferred[i]
            } else {
                runtime_hint.unwrap_or(if default {
                    preferred[i]
                } else {
                    [300.0, 76.0][i]
                })
            };
            size.clamp(lower, upper)
        })
    }

    pub fn widget_title(&self, id: &ExtensionId) -> String {
        self.registry
            .get(id)
            .map_or_else(|| "扩展".to_owned(), |descriptor| descriptor.title.clone())
    }

    pub fn widget_view(&mut self, id: &ExtensionId, now: Instant) -> Option<Arc<WidgetView>> {
        let descriptor = self
            .registry
            .get(id)
            .filter(|d| d.kind == ExtensionKind::Widget)?;
        let extension = self.extensions.get(*self.owners.get(id)?)?;
        self.widget_cache
            .get_or_build(id.clone(), now, || {
                let view = extension.widget_view(id).map(|mut view| {
                    view.bounds.fit_width |= descriptor.widget_bounds.fit_width;
                    view.bounds.fit_height |= descriptor.widget_bounds.fit_height;
                    if view.bounds.overflow == crate::ui::declarative::Overflow::Clip {
                        view.bounds.overflow = descriptor.widget_bounds.overflow;
                    }
                    Arc::new(view)
                });
                let hint = view.as_ref().map_or(descriptor.refresh_hint, |view| {
                    if view.refresh_hint == RefreshHint::OnInvalidation {
                        descriptor.refresh_hint
                    } else {
                        view.refresh_hint
                    }
                });
                (
                    view,
                    (hint == RefreshHint::EverySecond).then_some(Duration::from_secs(1)),
                )
            })
            .clone()
    }

    pub fn compact_views(&mut self, now: Instant) -> Vec<&crate::extension::CompactView> {
        // Build only stale sources, then borrow the resulting snapshots for this frame.
        // The cache borrows cannot overlap insertion, so do this in two passes.
        for (index, extension) in self.extensions.iter().enumerate() {
            self.compact_cache.get_or_build(index, now, || {
                (
                    extension.compact_views(),
                    extension.compact_refresh_interval(),
                )
            });
        }
        self.extensions
            .iter()
            .enumerate()
            .flat_map(|(index, _)| self.compact_cache.get(&index).into_iter().flatten())
            .filter(|view| {
                self.registry
                    .get(&view.id)
                    .is_some_and(|d| d.kind == ExtensionKind::Compact)
            })
            .collect()
    }

    pub fn update(&mut self, now: Instant) -> Vec<ExtensionOutput> {
        let mut output = std::mem::take(&mut self.startup_output);
        output.extend(
            self.extensions
                .iter_mut()
                .map(|extension| extension.update(now)),
        );
        output
    }

    pub fn on_event(
        &mut self,
        event: &crate::extension::Event,
        now: Instant,
    ) -> Vec<ExtensionOutput> {
        self.extensions
            .iter_mut()
            .map(|extension| extension.on_event(event, now))
            .collect()
    }

    pub fn on_action(
        &mut self,
        source_id: &ExtensionId,
        action: Action,
        now: Instant,
    ) -> Result<ExtensionOutput, String> {
        let Some(&owner) = self.owners.get(source_id) else {
            return Err(format!(
                "no extension registered for action source {source_id}"
            ));
        };
        Ok(self.extensions[owner].on_action(action, now))
    }

    pub fn set_bilibili_enabled(&mut self, enabled: bool) -> Result<(), String> {
        let id = ExtensionId::builtin(bilibili::NOTIFICATION_ID);
        let Some(extension) = self
            .extensions
            .iter_mut()
            .find(|extension| extension.handles_notification(&id))
        else {
            return Err("Bilibili extension is not registered".into());
        };
        let previous = self.lifecycle.record(&id).cloned();
        if enabled {
            let descriptor = extension
                .descriptors()
                .into_iter()
                .find(|descriptor| descriptor.id == id)
                .ok_or_else(|| "Bilibili extension descriptor is missing".to_owned())?;
            if previous.as_ref().is_some_and(|record| record.enabled) {
                if let Some(record) = previous
                    .as_ref()
                    .filter(|record| record.version != descriptor.version)
                {
                    extension.on_upgrade(&self.context, &record.version)?;
                }
                extension.on_start(&self.context)?;
                return self
                    .lifecycle
                    .set(
                        id,
                        ExtensionLifecycleRecord {
                            version: descriptor.version,
                            enabled: true,
                            initialized: true,
                            last_result: "on_start:ok".into(),
                            last_failure: None,
                        },
                    )
                    .map_err(|error| error.to_string());
            }
            let install_result = if previous.as_ref().is_none_or(|record| !record.initialized) {
                extension
                    .on_install(&self.context)
                    .and_then(|()| extension.on_enable(&self.context))
            } else if previous
                .as_ref()
                .is_some_and(|record| record.version != descriptor.version)
            {
                let old = &previous.as_ref().expect("checked record").version;
                extension
                    .on_upgrade(&self.context, old)
                    .and_then(|()| extension.on_enable(&self.context))
            } else {
                extension.on_enable(&self.context)
            };
            if let Err(error) = install_result {
                let record = ExtensionLifecycleRecord {
                    version: previous.as_ref().map_or_else(
                        || descriptor.version.clone(),
                        |record| record.version.clone(),
                    ),
                    enabled: false,
                    initialized: previous.as_ref().is_some_and(|record| record.initialized),
                    last_result: "on_enable:failed".into(),
                    last_failure: Some(error.clone()),
                };
                if let Err(save_error) = self.lifecycle.set(id, record) {
                    eprintln!("Unable to save extension lifecycle: {save_error}");
                }
                return Err(error);
            }
            if let Err(error) = extension.on_start(&self.context) {
                let record = ExtensionLifecycleRecord {
                    version: descriptor.version,
                    enabled: false,
                    initialized: true,
                    last_result: "on_start:failed".into(),
                    last_failure: Some(error.clone()),
                };
                if let Err(save_error) = self.lifecycle.set(id, record) {
                    eprintln!("Unable to save extension lifecycle: {save_error}");
                }
                return Err(error);
            }
            self.lifecycle
                .set(
                    id,
                    ExtensionLifecycleRecord {
                        version: descriptor.version,
                        enabled: true,
                        initialized: true,
                        last_result: "on_start:ok".into(),
                        last_failure: None,
                    },
                )
                .map_err(|error| error.to_string())
        } else {
            if previous.as_ref().is_none_or(|record| !record.enabled) {
                return Ok(());
            }
            extension.on_stop(&self.context)?;
            extension.on_disable(&self.context)?;
            let mut record = previous.unwrap_or_default();
            record.enabled = false;
            record.last_result = "on_disable:ok".into();
            record.last_failure = None;
            self.lifecycle
                .set(id, record)
                .map_err(|error| error.to_string())
        }
    }

    #[allow(dead_code)] // Exposed to the explicit extension-removal workflow.
    pub fn uninstall_bilibili(&mut self) -> Result<(), String> {
        let id = ExtensionId::builtin(bilibili::NOTIFICATION_ID);
        let Some(extension) = self
            .extensions
            .iter_mut()
            .find(|extension| extension.handles_notification(&id))
        else {
            return Err("Bilibili extension is not registered".into());
        };
        extension.on_stop(&self.context)?;
        extension.on_disable(&self.context)?;
        extension.on_uninstall(&self.context)?;
        self.lifecycle
            .remove(&id)
            .map_err(|error| error.to_string())
    }

    pub fn bilibili_status(&self) -> String {
        let id = ExtensionId::builtin(bilibili::NOTIFICATION_ID);
        self.extensions
            .iter()
            .find(|extension| extension.handles_notification(&id))
            .and_then(|extension| extension.status_message())
            .unwrap_or_else(|| "尚未启用".into())
    }

    pub fn drain_context_tasks(&self) -> ExtensionOutput {
        let (scheduled, canceled) = self.context.drain_tasks();
        ExtensionOutput {
            schedule: scheduled
                .into_iter()
                .map(|(task_id, deadline)| crate::extension::ScheduledTask { task_id, deadline })
                .collect(),
            cancel_tasks: canceled,
            ..ExtensionOutput::default()
        }
    }

    pub fn on_notifications_committed(&mut self, notifications: &[Notification]) {
        for extension in &mut self.extensions {
            extension.on_notifications_committed(notifications);
        }
    }

    pub fn notification_presentation(
        &self,
        notification: &Notification,
    ) -> NotificationPresentation {
        self.extensions
            .iter()
            .find(|extension| extension.handles_notification(&notification.source_id))
            .map_or_else(
                || NotificationPresentation::from_notification(notification),
                |extension| NotificationPresentation {
                    alert: extension.alert_view(notification),
                    center_item: extension.center_item_view(notification),
                },
            )
    }

    pub fn on_notification_action(
        &mut self,
        notification: &Notification,
        action: &NotificationAction,
    ) -> Result<(), String> {
        let Some(extension) = self.extensions.iter_mut().find(|extension| {
            self.registry.get(&notification.source_id).is_some()
                && extension.handles_notification(&notification.source_id)
        }) else {
            return Err(format!(
                "no notification extension registered for {}",
                notification.source_id
            ));
        };
        extension.on_notification_action(notification, action)
    }
}

impl Drop for BuiltinExtensions {
    fn drop(&mut self) {
        for extension in &mut self.extensions {
            if let Err(error) = extension.on_stop(&self.context) {
                eprintln!("Extension on_stop failed: {error}");
            }
        }
    }
}

fn user_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("IslandLab");
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Library/Application Support/island-lab");
    }
    #[cfg(target_os = "linux")]
    {
        return std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join(".local/share")
            })
            .join("island-lab");
    }
    #[allow(unreachable_code)]
    std::env::temp_dir().join("island-lab")
}

fn user_config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("IslandLab");
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Library/Application Support/island-lab");
    }
    #[cfg(target_os = "linux")]
    {
        return std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join(".config")
            })
            .join("island-lab");
    }
    #[allow(unreachable_code)]
    std::env::temp_dir().join("island-lab")
}

fn lifecycle_path() -> PathBuf {
    user_data_dir().join("extension-lifecycle.tsv")
}
