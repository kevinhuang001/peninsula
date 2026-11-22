# Core-owned extension settings (V3)

The extension API declares *data*, never egui controls or settings-page callbacks. Core owns settings navigation, grouping, editors, validation, persistence and contribution lifecycle.

## Contract

- `ExtensionDescriptor.settings` defines owner-wide business settings. Shared owner fields that should appear in Notifications declare `.in_notifications()`. Both surfaces edit the exact same owner key and stored value.
- `ContributionDescriptor.settings` defines per-contribution fields. `ContributionDescriptor.group_path` declares optional business subgroups; Core chooses the visual hierarchy and layout.
- Every Notification receives a host-owned enable switch. An extension must not add a second enable setting.
- `SettingSchema.presentation` contains generic, optional editor hints: `NumberText`, `DurationDisplay::Minutes`, placeholder and unit. Neither the schema nor extension code can provide UI callbacks.
- `SettingSchema.activation` is distinct from persistence validation. A zero price can be saved while an enabled notification remains pending configuration. For contributions with activation fields, Core determines readiness from schema; older custom setup hooks remain a fallback for contributions without declarative activation rules.
- Runtime contributions and background services are started only after Core checks owner enablement, contribution enablement, setup readiness and resource dependencies. Their desired switch values are retained while stopped.
- Dynamic contributions still use stable owner-qualified IDs and atomic catalog snapshots. Removing an entry invalidates its view, cancels the scoped task and releases unused dependencies; historical notifications persist.

## Example: Market

Market declares `alert_interval_minutes` once on the owner, with `in_notifications()` and `with_unit("分钟")`. Each stock/crypto asset declares a Notification with `condition` and `threshold`; the latter uses `number_text("例如 150.50")` and `activation_positive()`. Core groups them by `group_path`, renders the controls, and keeps the switch separate from the actual running state. No Market-specific conditional is required in the settings UI.

## Rules for reviewers

1. New business contributions must not require editing `src/app/settings/view.rs` or the notification-settings renderer.
2. Host settings code must not match extension IDs, asset ID prefixes or business-specific field names to decide which controls to draw.
3. Configuration writes are validated by `SettingsStore`; incomplete numeric input is an ephemeral UI draft, not a persisted value.
4. Multiple surfaces editing a setting must share its single owner key, not synchronize two independent copies.
5. Integration tests must cover dynamic contributions, pending setup, grouped rendering, shared owner settings and cancellation after disable.
