# Unified Notification Settings Contract (v1)

## Goals and ownership

Notification controls belong to the host Settings → Notifications page, grouped by owning extension. Extensions register **Notification contributions** dynamically or statically; they do not create separate notification-preferences pages or store duplicate host switch state.

Each notification contribution declares a stable ID, title, `ContributionKind::Notification`, `default_enabled`, optional dependencies, and its own typed `SettingSchema`. The host renders its switch and schema fields and persists them in `contribution-settings.json`. The switch is held in host contribution preferences. The extension lifecycle manager starts and stops contributions based on switches, dependencies and setup status.

Extension-wide options shared by many notifications belong in `ExtensionDescriptor.settings`, not in each contribution's settings. Any second settings surface must edit the **same key** and data record; there must be no second persisted copy.

## Market notification contract

For each user-owned market asset, the market extension registers:

- `builtin.market/assets/<kind>/<symbol>/price-alert` as a `Notification` contribution, and a separate optional `Capsule` contribution.
- `condition: Enum("高于" | "低于")` and `threshold: Float` as notification-specific fields.
- `builtin.market/service.quotes` as an on-demand background dependency. No Widget or Capsule is required for alerts.
- `alert_interval_minutes: Integer`, valid from **1–10,080 minutes**, once on the **market extension**. All per-asset price alerts share it.

The Notifications → Market UI displays this one shared interval before the asset list, and displays each asset's independent enable switch, condition and **editable target price text field**. The text editor accepts finite nonnegative decimal values, keeps incomplete typing as a UI draft, and commits valid prices into the contribution's existing Float setting; a blank value clears the threshold to zero. Nonpositive prices cannot activate the notification. The source of truth is the existing extension setting; the same cooldown control in Extensions → Market edits the same record.

A threshold must be finite and **greater than zero** before a price-alert contribution can start. A switch records the user's intended enabled state; Core starts a contribution only after its activation rules pass. For an incomplete activation field, show a brief warning beside that field (for example `目标价格：需填写大于 0 的数值`), not a card-wide banner. Clear it as soon as the value becomes valid; the switch need not be toggled again. Removing an asset unregisters its notification contribution and stops any associated work. Dynamic contribution IDs remain stable while the asset is present.

On each new quote, compare the current price with the selected condition and threshold. Only emit an alert when the condition matches and the asset has not alerted during the shared cooldown period. Quiet Hours / Do Not Disturb affects notification delivery, not prompt contributions.

## Data sources and other extensions

Data-source notifications follow the same registration/switch/schema contract; their per-source cooldown is **stored once in the Notification contribution setting** (`cooldown: Duration` in milliseconds internally) and rendered to the user as **whole minutes** in both Notifications and the data-source settings window. Conversions use `minutes × 60,000` milliseconds, with schema bounds checked by the host.

Other notifications may use their own contribution-specific fields, but the host—not the extension—owns the unified enable switch and general settings layout.

## Resource lifecycle and configuration updates

An inactive contribution does not remain subscribed to events or schedule work as a separate consumer. Background resources needed by multiple enabled contributions are shared and are stopped once no running contribution depends on them. Enabling a notification with missing configuration does not start a background resource solely for that incomplete notification. Settings changes are validated by their declared schema, persisted atomically by the host, and reconciled with active contributions.

## Capsule and Components navigation

**胶囊 (Capsule)** keeps Motion, Mascot and Capsule 样式, and owns two more secondary menus:

- **Capsule 开关**: switches grouped by owning extension; switching does not change order.
- **Capsule 顺序**: sorting contains **enabled Capsule** entries only.
- **Capsule 样式**: independent Capsule typography configuration.

**组件 (Components)** remains a first-level settings page, but owns **only Widget** secondary menus:

- **组件开关**: switches grouped by owning extension. A disabled, permanently-on **通知中心** row appears first; the user cannot toggle it.
- **组件顺序**: a fixed **通知中心** row appears first and has no move controls, followed by sortable **enabled Widget** entries only. No explanatory paragraph is needed.

Widget theme and accent configuration remain in General settings.

Disabled contributions retain their saved position, but do not appear in order lists.

A change to one contribution's visibility must not toggle other contributions owned by the same extension.

## Input focus rendering

Text controls retain their full-size interactive rectangles, but nested settings sub-UIs permit a small extra painting inset (`2 pt`) around the input rectangle so centered focus strokes are not clipped on the left and right. Do not change the keyboard focus semantics or use an application-wide style change to hide the problem. Test both nested row controls and dialog inputs.
