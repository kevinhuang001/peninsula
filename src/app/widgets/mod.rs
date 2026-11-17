use crate::extension::ExtensionId;

pub fn default_widget_id() -> ExtensionId {
    ExtensionId::builtin(crate::extension::NOTIFICATION_CENTER_ID)
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
    use super::{default_widget_id, next_widget};
    use crate::{builtins, extension::ExtensionId};

    fn widget_ids() -> Vec<ExtensionId> {
        builtins::registry()
            .widget_list()
            .into_iter()
            .map(|descriptor| descriptor.id.clone())
            .collect()
    }

    #[test]
    fn notification_center_is_always_first_and_is_the_default_widget() {
        let ids = widget_ids();
        assert_eq!(ids.len(), 4);
        assert_eq!(ids[0], default_widget_id());
        assert_eq!(ids[0].as_str(), crate::extension::NOTIFICATION_CENTER_ID);
        assert_eq!(ids[1].as_str(), "builtin.system-monitor/overview");
        assert_eq!(ids[2].as_str(), "builtin.pomodoro/timer");
        assert_eq!(ids[3].as_str(), "builtin.weather/forecast");
    }

    #[test]
    fn widget_cycling_uses_registry_order() {
        let ids = widget_ids();
        assert_eq!(next_widget(&ids[1], &ids, 1), ids[2]);
        assert_eq!(next_widget(&ids[1], &ids, -1), ids[0]);
    }
}
