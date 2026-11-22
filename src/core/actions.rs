use crate::extension::ActionInvocation;

/// Records every user action before forwarding it to the owning extension.
#[derive(Clone, Debug, Default)]
pub struct ActionDispatcher {
    history: Vec<ActionInvocation>,
}

impl ActionDispatcher {
    pub fn dispatch(
        &mut self,
        invocation: ActionInvocation,
        handler: impl FnOnce(&ActionInvocation) -> Result<(), String>,
    ) -> Result<(), String> {
        self.history.push(invocation.clone());
        handler(&invocation)
    }

    #[allow(dead_code)] // Retained for host diagnostics and focused dispatcher tests.
    pub fn history(&self) -> &[ActionInvocation] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::ActionDispatcher;
    use crate::extension::{Action, ActionInvocation, ExtensionId};

    #[test]
    fn action_is_recorded_before_dispatch_even_when_it_fails() {
        let invocation = ActionInvocation {
            source_extension_id: ExtensionId::builtin("test.timer"),
            action: Action::Toggle,
        };
        let mut dispatcher = ActionDispatcher::default();
        assert!(dispatcher
            .dispatch(invocation.clone(), |_| Err("failed".into()))
            .is_err());
        assert_eq!(dispatcher.history(), &[invocation]);
    }
}
