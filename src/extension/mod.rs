mod descriptor;
mod lifecycle;
mod notification;
mod registry;
mod schema;
mod view;

pub use descriptor::{Capability, ExtensionDescriptor, ExtensionId, ExtensionKind};
pub use lifecycle::{ExtensionContext, ExtensionLifecycleRecord, LifecycleManager};
pub use notification::{
    Notification, NotificationAction, NotificationAlertView, NotificationCenterItemView,
    NotificationKey, NotificationPresentation, NotificationPriority, NotificationStatus,
};
pub use registry::{ExtensionRegistry, RegistrationError, NOTIFICATION_CENTER_ID};
pub use view::{
    Action, ActionInvocation, AlertInteraction, CompactView, Event, ExtensionOutput, HostScheduler,
    NotificationPresentationProvider, RuntimeExtension, ScheduledTask,
};

pub use crate::ui::declarative::{ViewNode, WidgetView};

pub use schema::{
    validate_schema, Constraints, ReloadPolicy, SettingSchema, SettingValue, Settings, ValueType,
};
