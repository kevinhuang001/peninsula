mod performance;
pub mod pomodoro;

use crate::extension::{builtins, ExtensionId};
pub(in crate::app) use performance::{expanded_width, paint_metrics};

pub const PERFORMANCE_ID: &str = "builtin.system-monitor/overview";
pub const POMODORO_ID: &str = "builtin.pomodoro/timer";

pub fn widget_ids(notification_unread: bool) -> Vec<ExtensionId> {
    builtins()
        .widget_list(notification_unread)
        .into_iter()
        .map(|descriptor| descriptor.id.clone())
        .collect()
}

pub fn default_widget_id() -> ExtensionId {
    ExtensionId::builtin(PERFORMANCE_ID)
}

pub fn widget_height(id: &ExtensionId) -> f32 {
    builtins()
        .get(id)
        .filter(|descriptor| descriptor.size_hint[1] > 0.0)
        .map_or(76.0, |descriptor| descriptor.size_hint[1])
}

pub fn widget_title(id: &ExtensionId) -> String {
    builtins()
        .get(id)
        .map_or_else(|| "扩展".to_owned(), |descriptor| descriptor.title.clone())
}

pub fn next_widget(current: &ExtensionId, ids: &[ExtensionId], offset: isize) -> ExtensionId {
    if ids.is_empty() {
        return default_widget_id();
    }
    let index = ids.iter().position(|id| id == current).unwrap_or(0);
    let target = (index as isize + offset).rem_euclid(ids.len() as isize) as usize;
    ids[target].clone()
}

#[cfg(test)]
mod tests {
    use super::{default_widget_id, next_widget, widget_height, widget_ids};
    use crate::extension::ExtensionId;

    #[test]
    fn built_in_pages_are_registered_and_have_size_hints() {
        let ids = widget_ids(false);
        assert_eq!(ids.len(), 2);
        assert_eq!(
            widget_height(&ExtensionId::builtin(super::POMODORO_ID)),
            126.0
        );
        assert!(widget_height(&ExtensionId::builtin(super::POMODORO_ID)) > widget_height(&ids[0]));
    }

    #[test]
    fn unread_notifications_add_a_first_page_without_changing_page_ids() {
        let normal = widget_ids(false);
        let with_notifications = widget_ids(true);
        assert_eq!(
            with_notifications[0].as_str(),
            crate::extension::NOTIFICATION_CENTER_ID
        );
        assert!(with_notifications.contains(&normal[0]));
        assert_eq!(
            next_widget(&normal[0], &with_notifications, -1).as_str(),
            crate::extension::NOTIFICATION_CENTER_ID
        );
        assert_eq!(next_widget(&default_widget_id(), &normal, 1), normal[1]);
    }
}
