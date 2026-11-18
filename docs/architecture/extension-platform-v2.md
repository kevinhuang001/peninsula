# Extension platform V2

Based on PR #7 (`codex/performance-process-monitor`, `9d106ab`). This PR is stacked on that branch; its existing media backends, browser download readers, process monitor, HTTP proxy handling and window behavior remain the implementation base.

## Boundaries and registration

`builtins::runtimes()` is the composition root. It supplies Clock, Pomodoro, System Monitor, Battery, Downloads, Weather, Music (on its existing Linux/Windows platforms), Kuaidi and Bilibili to `core::extension_manager::ExtensionManager`.

Each runtime returns one `ExtensionDescriptor` containing its business ID, metadata, settings schema, execution policy and multiple `ContributionDescriptor`s. Existing surface IDs remain stable qualified contribution keys, so saved Compact selections and notification source identities survive the transition. The registry rejects duplicate owner/contribution IDs, unsupported schemas, reserved Core page IDs and plugin-owned enable/running settings. Background service, event handler and Mascot contributions are registered with the same descriptor.

App coordinates windows and input. Capsule consumes declarative snapshots. Neither dispatches specific business modules. Core's only fixed user-facing capabilities are Mascot and Notification Center. The center is prepended by the host, is available with zero running extensions, and is absent from the extension registry.

## Lifecycle and scheduling

A runtime moves through Registered, Starting, Running, Stopping and Stopped; failures enter Failed and are shown in its settings page. Enabling/disabling is idempotent and persisted in the host lifecycle store. Hooks initialize/upgrade configuration, start services and stop them as one business module.

Only running owners receive updates, subscribed events, actions, Compact snapshots, Widget pages and Mascot registration. Stop invalidates cached surfaces and cancels all tracked scheduler keys, including tasks scheduled through the owner-scoped `ExtensionContext`. Already queued deadline events cannot reach a stopped/restarted owner. Task IDs are qualified by their owning extension.

`Worker` has cancellation and join semantics. Process/GPU/power samples use a single-slot latest-value mailbox; Downloads retains its event stream. Media, lyrics, artwork, weather, Kuaidi and Bilibili workers are joined during stop. A stop can wait for an in-flight OS query or network operation to return; these retain their existing backend timeout behavior.

Execution supports EventDriven, Interval, WhenVisible and OnEnable. Clock uses a view refresh interval; Pomodoro uses monotonic deadlines. App wakes for running services, visible view refreshes, pending notifications, DND schedules, hover intent and shell/renderer animations. Removing the last source leaves no fixed 100ms repaint loop.

## Mascot

`core::mascot` owns the independent leading slot and its renderer. Its enabled state, size and Full/Reduced/Off motion settings live under Capsule, separately from ordinary Compact order.

Expressions declare owner-local IDs, While conjunctions or OnEvent triggers, priority and duration. Selection collects active candidates, chooses randomly among the highest priority, and avoids repeating when alternatives exist. It reselects on candidate changes or expiry, never on every animation frame. Stopped owners and hidden slots lose candidates and event deadlines. Music, Pomodoro, Battery and Downloads supply expressions.

The expression declaration can use the bundled sprite catalog or an optional declarative `ViewNode` (SVG, PNG/WebP raster assets, multi-frame sprites and Motion). Assets and animations use the common Core renderer. Hidden slots do not render; per-Mascot Reduced/Off freezes nested animation declarations independently of global motion policy.

## Settings and upgrade

The fixed navigation is General, Capsule, Do Not Disturb, Extensions and About. Extensions keeps a persistent second column of plugin names; the selected plugin’s configuration occupies the bordered third panel on the right. All menus retain rounded, bordered navigation/content panels with neutral selected rows. Redundant page headings/descriptions are omitted. Settings grow to fit their content and show scrollbars only when content exceeds the screen-height limit. Schema forms place labels on the left with controls aligned on the right. Core renders system enable/status first, then schema controls, then ordinary Compact contribution visibility. Empty schemas still have a detail page; stopped extensions remain configurable. All Boolean, Integer, Float, String, Enum, Duration and Color values use the same renderer and validation/store path.

Business settings are stored once per owner in `extension-settings.json`. Old per-surface records are merged into the owning business schema in an atomic write, then removed. `settings/migration.rs` reads old flat Pomodoro, Weather, Kuaidi and Bilibili input. Legacy Mascot visibility moves to Capsule and the old System Monitor battery key moves to Battery. Existing Compact visibility/order, DND, theme and widget order remain supported. Legacy input is retained if schema persistence is unsuccessful; normal saves remove it after successful migration.

Notification history is stored atomically in a separate `notifications.json`, including acknowledgements and deduplication identity. Closing an alert leaves its center record pending. DND suppresses popups, not storage. Restored records appear in the center without replaying every popup. Source shutdown does not delete records. Corrupt files are preserved for recovery.

Pomodoro preserves paused state on stop/restart, has schema-driven focus/break durations and optional automatic next phase. Battery owns power sampling and charging visuals independently of System Monitor. Clock owns time/date formatting and its analog SVG; Core no longer reads wall-clock time to paint the clock icon.

## Validation

Unit and headless egui tests cover registry/schema validation, five-section navigation, generated settings controls while stopped, multiple same-kind contributions, idempotent lifecycle, rollback, scoped cancellation, disabled settings/restart behavior, Mascot priority/random rotation/expiry, notification persistence and DND, timer restoration, layout and existing browser/platform adapters.

CI runs native tests and release builds on Linux, macOS and Windows. Formatting runs on every platform and strict all-target Clippy runs on Linux. Native OS integration and backend credentials remain dependent on their respective runners/machines.

Settings stay expanded while open without requiring the pin control. Ordinary Widgets use screen-dependent width limits with square-root growth capped at 350–700 points on large displays (280–560 at 1440 logical points); Music declares a smaller minimum through layout metadata, while long content stays inside the same host maximum. Settings retain their independent navigation width.
