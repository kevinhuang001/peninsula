mod descriptor;
mod diagnostics;
mod lifecycle;
mod notification;
mod registry;
mod schema;
mod view;

pub use descriptor::{
    Capability, ContributionDescriptor, ContributionKind, ExecutionPolicy, ExtensionDescriptor,
    ExtensionId,
};
pub use diagnostics::{DiagnosticCheck, DiagnosticStatus};
pub use lifecycle::{ExtensionContext, ExtensionLifecycleRecord, LifecycleManager};
pub use notification::{
    Notification, NotificationAction, NotificationAlertView, NotificationCenterItemView,
    NotificationKey, NotificationPresentation, NotificationPriority, NotificationStatus,
};
pub use registry::{ExtensionRegistry, RegistrationError, NOTIFICATION_CENTER_ID};
pub use view::{
    Action, ActionInvocation, AlertInteraction, CompactView, CompactVisibility, Event,
    ExtensionOutput, HostScheduler, NotificationPresentationProvider, Prompt, PromptLayer,
    RuntimeExtension, ScheduledTask, SettingSearchTask, WidgetVisibility,
};

pub use crate::ui::declarative::{ViewNode, WidgetView};

pub use schema::{
    validate_schema, Constraints, ReloadPolicy, SearchOption, SearchSchema, SearchSelection,
    SettingSchema, SettingValue, Settings, ValueType,
};
