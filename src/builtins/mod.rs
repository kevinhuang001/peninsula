pub mod bilibili;
pub mod download;
pub mod performance;
pub mod pomodoro;

use crate::{
    core::notifications,
    extension::{
        Action, ExtensionContext, ExtensionId, ExtensionKind, ExtensionLifecycleRecord,
        ExtensionOutput, ExtensionRegistry, LifecycleManager, Notification, NotificationAction,
        NotificationPresentation, RuntimeExtension, WidgetView,
    },
};
use std::path::PathBuf;
use std::time::Instant;

mod extras;

pub fn registry() -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::default();
    for descriptor in [notifications::descriptor()]
        .into_iter()
        .chain(performance::descriptors())
        .chain(download::descriptors())
        .chain(pomodoro::descriptors())
        .chain(
            std::iter::once(bilibili::BilibiliExtension::default())
                .flat_map(|extension| extension.descriptors()),
        )
        .chain(extras::descriptors())
    {
        registry
            .register(descriptor)
            .expect("built-in extension IDs must be unique");
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
}

impl BuiltinExtensions {
    pub fn new() -> Self {
        let mut host = Self {
            registry: registry(),
            extensions: vec![
                Box::new(performance::PerformanceExtension::new()),
                Box::new(download::DownloadExtension::new()),
                Box::new(pomodoro::PomodoroExtension::default()),
                Box::new(bilibili::BilibiliExtension::default()),
                Box::new(extras::CompactExtras),
            ],
            lifecycle: LifecycleManager::load(Some(lifecycle_path())),
            context: ExtensionContext::new(user_data_dir(), user_config_dir()),
        };
        host.start_enabled_extensions();
        host
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

    pub fn widget_ids(&self) -> Vec<ExtensionId> {
        self.registry
            .widget_list()
            .into_iter()
            .map(|descriptor| descriptor.id.clone())
            .collect()
    }

    pub fn widget_height(&self, id: &ExtensionId) -> f32 {
        self.registry
            .get(id)
            .map_or(76.0, |descriptor| descriptor.size_hint[1])
    }

    pub fn widget_width(&self, id: &ExtensionId) -> f32 {
        self.registry
            .get(id)
            .map_or(300.0, |descriptor| descriptor.size_hint[0])
    }

    pub fn widget_title(&self, id: &ExtensionId) -> String {
        self.registry
            .get(id)
            .map_or_else(|| "扩展".to_owned(), |descriptor| descriptor.title.clone())
    }

    pub fn widget_view(&self, id: &ExtensionId) -> Option<WidgetView> {
        if self
            .registry
            .get(id)
            .is_none_or(|descriptor| descriptor.kind != ExtensionKind::Widget)
        {
            return None;
        }
        self.extensions
            .iter()
            .find_map(|extension| extension.widget_view(id))
    }

    pub fn compact_views(&self) -> Vec<crate::extension::CompactView> {
        self.extensions
            .iter()
            .flat_map(|extension| extension.compact_views())
            .filter(|view| {
                self.registry
                    .get(&view.id)
                    .is_some_and(|descriptor| descriptor.kind == ExtensionKind::Compact)
            })
            .collect()
    }

    pub fn update(&mut self, now: Instant) -> Vec<ExtensionOutput> {
        self.extensions
            .iter_mut()
            .map(|extension| extension.update(now))
            .collect()
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
        let Some(extension) = self.extensions.iter_mut().find(|extension| {
            self.registry.get(source_id).is_some()
                && extension
                    .descriptors()
                    .iter()
                    .any(|descriptor| &descriptor.id == source_id)
        }) else {
            return Err(format!(
                "no extension registered for action source {source_id}"
            ));
        };
        Ok(extension.on_action(action, now))
    }

    pub fn set_pomodoro_durations(
        &mut self,
        focus_minutes: u16,
        break_minutes: u16,
    ) -> Vec<ExtensionOutput> {
        self.extensions
            .iter_mut()
            .map(|extension| extension.set_pomodoro_durations(focus_minutes, break_minutes))
            .filter(|output| {
                !output.invalidated.is_empty()
                    || !output.cancel_tasks.is_empty()
                    || !output.schedule.is_empty()
            })
            .collect()
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

    pub fn configure_bilibili(
        &mut self,
        opencli_path: &str,
        dm_enabled: bool,
        likes_enabled: bool,
        show_dm_summary: bool,
        poll_seconds: u16,
    ) -> Result<(), String> {
        let id = ExtensionId::builtin(bilibili::NOTIFICATION_ID);
        let Some(extension) = self
            .extensions
            .iter_mut()
            .find(|extension| extension.handles_notification(&id))
        else {
            return Err("Bilibili extension is not registered".into());
        };
        extension.configure_bilibili(
            &self.context,
            opencli_path,
            dm_enabled,
            likes_enabled,
            show_dm_summary,
            poll_seconds,
        )
    }

    pub fn bilibili_status(&self) -> String {
        let id = ExtensionId::builtin(bilibili::NOTIFICATION_ID);
        self.extensions
            .iter()
            .find(|extension| extension.handles_notification(&id))
            .and_then(|extension| extension.status_message())
            .unwrap_or_else(|| "尚未启用".into())
    }

    pub fn request_bilibili_check(&mut self) -> Result<(), String> {
        let id = ExtensionId::builtin(bilibili::NOTIFICATION_ID);
        let Some(extension) = self
            .extensions
            .iter_mut()
            .find(|extension| extension.handles_notification(&id))
        else {
            return Err("Bilibili extension is not registered".into());
        };
        extension.request_bilibili_check(&self.context)
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

impl Default for BuiltinExtensions {
    fn default() -> Self {
        Self::new()
    }
}
