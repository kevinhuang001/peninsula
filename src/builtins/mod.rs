pub mod download;
pub mod performance;
pub mod pomodoro;

use crate::{
    core::notifications,
    extension::{
        Action, ExtensionId, ExtensionKind, ExtensionOutput, ExtensionRegistry, Notification,
        NotificationAction, NotificationPresentation, RuntimeExtension, WidgetView,
    },
};
use std::time::Instant;

mod extras;

pub fn registry() -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::default();
    for descriptor in [notifications::descriptor()]
        .into_iter()
        .chain(performance::descriptors())
        .chain(download::descriptors())
        .chain(pomodoro::descriptors())
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
}

impl BuiltinExtensions {
    pub fn new() -> Self {
        Self {
            registry: registry(),
            extensions: vec![
                Box::new(performance::PerformanceExtension::new()),
                Box::new(download::DownloadExtension::new()),
                Box::new(pomodoro::PomodoroExtension::default()),
                Box::new(extras::CompactExtras),
            ],
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

impl Default for BuiltinExtensions {
    fn default() -> Self {
        Self::new()
    }
}
