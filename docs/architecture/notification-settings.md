# Unified Notification Settings Contract (v1)

## Goals and ownership

Notification controls belong to the host Settings → Notifications page, grouped by owning extension. Extensions register **Notification contributions** dynamically or statically; they do not create separate notification-preferences pages or store duplicate host switch state.

Each notification contribution declares a stable ID, title, `ContributionKind::Notification`, `default_enabled`, optional dependencies, and its own typed `SettingSchema`. The host renders its switch and schema fields and persists them in `contribution-settings.json`. The switch is held in host contribution preferences. The extension lifecycle manager starts and stops contributions based on switches, dependencies and setup status.

Extension-wide options shared by many notifications belong in `ExtensionDescriptor.settings`, not in each contribution's settings. Any second settings surface must edit the **same key** and data record; there must be no second persisted copy.

## Market notification contract

For each user-owned market asset, the market extension registers:

- `builtin.market/assets/<kind>/<symbol>/price-alert` as a `Notification` contribution, and a separate optional `Compact` contribution.
- `condition: Enum("高于" | "低于")` and `threshold: Float` as notification-specific fields.
- `builtin.market/service.quotes` as an on-demand background dependency. No Widget or Compact is required for alerts.
- `alert_interval_minutes: Integer`, valid from **1–10,080 minutes**, once on the **market extension**. All per-asset price alerts share it.

The Notifications → Market UI displays this one shared interval before the asset list, and displays each asset's independent enable switch, condition and target price. The source of truth is the existing extension setting; the same control in Extensions → Market edits the same record.

A threshold must be finite and **greater than zero** before a price-alert contribution can start. If an alert is switched on but not configured, show “待配置” and let the user edit its settings; never report the notification as actively running. Removing an asset unregisters its notification contribution and stops any associated work. Dynamic contribution IDs remain stable while the asset is present.

On each new quote, compare the current price with the selected condition and threshold. Only emit an alert when the condition matches and the asset has not alerted during the shared cooldown period. Quiet Hours / Do Not Disturb affects notification delivery, not prompt contributions.

## Data sources and other extensions

Data-source notifications follow the same registration/switch/schema contract; their per-source cooldown is **stored once in the Notification contribution setting** (`cooldown: Duration` in milliseconds internally) and rendered to the user as **whole minutes** in both Notifications and the data-source settings window. Conversions use `minutes × 60,000` milliseconds, with schema bounds checked by the host.

Other notifications may use their own contribution-specific fields, but the host—not the extension—owns the unified enable switch and general settings layout.

## Resource lifecycle and configuration updates

An inactive contribution does not remain subscribed to events or schedule work as a separate consumer. Background resources needed by multiple enabled contributions are shared and are stopped once no running contribution depends on them. Enabling a notification with missing configuration does not start a background resource solely for that incomplete notification. Settings changes are validated by their declared schema, persisted atomically by the host, and reconciled with active contributions.

## Capsule secondary menus

The Capsule configuration keeps existing Motion and Mascot entries; the four independent presentation-management entries are **Compact**, **Widget**, **开关** and **顺序**.

- Compact: compact text presentation.
- Widget: widget/global theme presentation.
- 开关: Compact switches and Widget switches, grouped by owner extension; switches do not reorder.
- 顺序: separate Compact and Widget lists containing **enabled items only**, with notification center permanently first. Disabled items preserve their saved relative order.

A change to one contribution's visibility must not toggle other contributions owned by the same extension.

## Input focus rendering

Text controls retain their full-size interactive rectangles, but nested settings sub-UIs permit a small extra painting inset (`2 pt`) around the input rectangle so centered focus strokes are not clipped on the left and right. Do not change the keyboard focus semantics or use an application-wide style change to hide the problem. Test both nested row controls and dialog inputs.
