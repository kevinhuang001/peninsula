mod performance;
pub mod pomodoro;

use crate::extension::{builtins, ExtensionId};
pub(in crate::app) use performance::{expanded_width, paint_metrics};

pub const PERFORMANCE_ID: &str = "builtin.system-monitor/overview";
pub const POMODORO_ID: &str = "builtin.pomodoro/timer";

pub fn widget_ids() -> Vec<ExtensionId> {
    builtins()
        .widget_list()
        .into_iter()
        .map(|descriptor| descriptor.id.clone())
        .collect()
}

pub fn default_widget_id() -> ExtensionId {
    ExtensionId::builtin(crate::extension::NOTIFICATION_CENTER_ID)
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
    use crate::extension::{ExtensionId, NOTIFICATION_CENTER_ID};

    #[test]
    fn notification_center_is_always_first_even_when_empty() {
        let ids = widget_ids();
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0].as_str(), NOTIFICATION_CENTER_ID);
        assert_eq!(default_widget_id(), ids[0]);
        assert_eq!(ids[1].as_str(), super::PERFORMANCE_ID);
        assert_eq!(ids[2].as_str(), super::POMODORO_ID);
        assert_eq!(
            widget_height(&ExtensionId::builtin(NOTIFICATION_CENTER_ID)),
            230.0
        );
    }

    #[test]
    fn widget_navigation_uses_stable_ids() {
        let ids = widget_ids();
        assert_eq!(next_widget(&ids[0], &ids, 1), ids[1]);
        assert_eq!(next_widget(&ids[1], &ids, -1), ids[0]);
        assert_eq!(next_widget(&ids[2], &ids, 1), ids[0]);
    }
}
