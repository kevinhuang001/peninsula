# Extension platform V2

Based on PR #7 (`codex/performance-process-monitor`, `9d106ab`). This PR is stacked on that branch; its existing media backends, browser download readers, process monitor, HTTP proxy handling and window behavior remain the implementation base.

## Boundaries and registration

`builtins::runtimes()` is the composition root. It supplies Clock, Pomodoro, System Monitor, Battery, Downloads, Weather, Music (on its existing Linux/Windows platforms), Kuaidi and Bilibili to `core::extension_manager::ExtensionManager`.

Each runtime returns one `ExtensionDescriptor` containing its business ID, metadata, settings schema, execution policy and multiple `ContributionDescriptor`s. Existing surface IDs remain stable qualified contribution keys, so saved Compact selections and notification source identities survive the transition. The registry rejects duplicate owner/contribution IDs, unsupported schemas, reserved Core page IDs and plugin-owned enable/running settings. Background service, event handler, Mascot and Prompt contributions are registered with the same descriptor.

App coordinates windows and input. Capsule consumes declarative snapshots. Neither dispatches specific business modules. Core's only fixed user-facing capabilities are Mascot and Notification Center. The center is prepended by the host, is available with zero running extensions, and is absent from the extension registry.

## Lifecycle and scheduling

A runtime moves through Registered, Starting, Running, Stopping and Stopped; failures enter Failed and are shown in its settings page. Enabling/disabling is idempotent and persisted in the host lifecycle store. Hooks initialize/upgrade configuration, start services and stop them as one business module.

Only running owners receive updates, subscribed events, actions, Compact snapshots, Widget pages, Mascot registration and Prompt publication. Stop invalidates cached surfaces, drops that owner's queued Prompt entries and cancels all tracked scheduler keys, including tasks scheduled through the owner-scoped `ExtensionContext`. Already queued deadline events cannot reach a stopped/restarted owner. Task IDs are qualified by their owning extension.

`Worker` has cancellation and join semantics. Process/GPU/power samples use a single-slot latest-value mailbox; Downloads retains its event stream. Media, lyrics, artwork, weather, Kuaidi and Bilibili workers are joined during stop. A stop can wait for an in-flight OS query or network operation to return; these retain their existing backend timeout behavior.

Execution supports EventDriven, Interval, WhenVisible and OnEnable. Clock uses a view refresh interval; Pomodoro uses monotonic deadlines. App wakes for running services, visible view refreshes, pending notifications, DND schedules, hover intent and shell/renderer animations. Removing the last source leaves no fixed 100ms repaint loop.

## Mascot

`core::mascot` owns the independent leading slot and its renderer. Its enabled state, size and Full/Reduced/Off motion settings live under Capsule, separately from ordinary Compact order.

Expressions declare owner-local IDs, While conjunctions or OnEvent triggers, priority and duration. Selection collects active candidates, chooses randomly among the highest priority, and avoids repeating when alternatives exist. It reselects on candidate changes or expiry, never on every animation frame. Stopped owners and hidden slots lose candidates and event deadlines. Music, Pomodoro, Battery and Downloads supply expressions.

The expression declaration can use the bundled sprite catalog or an optional declarative `ViewNode` (SVG, PNG/WebP raster assets, multi-frame sprites and Motion). Assets and animations use the common Core renderer. Hidden slots do not render; per-Mascot Reduced/Off freezes nested animation declarations independently of global motion policy.

## Settings and upgrade

The fixed navigation is General, Capsule, Do Not Disturb, Extensions and About. Capsule keeps a second column for motion, Mascot, Compact order and Widget order, with the selected configuration on the right. Extensions keeps a persistent second column of plugin names; the selected plugin’s configuration occupies the bordered third panel on the right. All menus retain rounded, bordered navigation/content panels with neutral selected rows. Redundant page headings/descriptions are omitted. Settings grow to fit their content and show scrollbars only when content exceeds the screen-height limit. Schema forms place labels on the left with controls aligned on the right. Core renders system enable/status first, then schema controls, then ordinary Compact contribution visibility. Empty schemas still have a detail page; stopped extensions remain configurable. All Boolean, Integer, Float, String, Enum, Duration and Color values use the same renderer and validation/store path.

Business settings are stored once per owner in `extension-settings.json`. The host reads the current `preferences.conf` and extension settings formats only. A settings schema version change initializes that extension's fields from current defaults; previous values are not imported or transformed. Unknown preference keys are ignored.

Search is a declared setting type. Its schema names a provider, placeholder and minimum query length. The runtime creates a detached search task; Core executes it off the UI thread and returns structured results with stable IDs, labels, optional details and provider data. The selected result is persisted as one validated setting value. Search queries and unselected result lists are transient UI state. Weather uses this contract for city selection; latitude and longitude are provider data and are not separate user-editable fields.

Every extension page includes a Doctor section. A check has a stable ID, title, status (`Passed`, `Warning`, `Failed` or `Unknown`), detail and optional remediation. Doctor callbacks are nonblocking snapshots; extensions perform process, browser, account and network probes in their task workers. The host adds the extension lifecycle state to the report. OpenCLI integrations report at least whether the command exists, the required plugin is available, a browser bridge is connected and the target site is logged in.

## Prompt contributions

Prompt is a one-shot exclusive Compact contribution. It contains a declarative view and a display duration. The host accepts it only from its registered running owner, then appends it to one FIFO queue; there is no priority field or persistent `while` trigger. While a Prompt is visible it replaces all ordinary Compact items, including the Mascot slot. Its duration pauses while Compact is hidden by an expanded Widget, settings or a notification alert. When the queue drains, normal Compact composition resumes. Prompt animations use the same `ViewNode` and Motion renderer as other surfaces.

Prompt presentation is separate from notification policy. Do Not Disturb suppresses notification popups only; it does not pause, suppress or reorder Prompts. Notifications and Prompts keep independent queues and lifetimes.

Notification history is stored atomically in a separate `notifications.json`, including acknowledgements and deduplication identity. Closing an alert leaves its center record pending. DND suppresses popups, not storage. Restored records appear in the center without replaying every popup. Source shutdown does not delete records. Corrupt files are preserved for recovery.

Pomodoro preserves paused state on stop/restart, has schema-driven focus/break durations and optional automatic next phase. Battery owns power sampling and charging visuals independently of System Monitor. Clock owns time/date formatting and its analog SVG; Core no longer reads wall-clock time to paint the clock icon.

## Validation

Unit and headless egui tests cover registry/schema validation, five-section navigation, generated settings controls while stopped, multiple same-kind contributions, idempotent lifecycle, rollback, scoped cancellation, disabled settings/restart behavior, Mascot priority/random rotation/expiry, notification persistence and DND, timer restoration, layout and existing browser/platform adapters.

CI runs native tests and release builds on Linux, macOS and Windows. Formatting runs on every platform and strict all-target Clippy runs on Linux. Native OS integration and backend credentials remain dependent on their respective runners/machines.

Settings stay expanded while open without requiring the pin control. Ordinary Widgets use screen-dependent width limits with square-root growth capped at 350–700 points on large displays (280–560 at 1440 logical points); Music uses the same host minimum: short content keeps that width, and only content exceeding it expands the Widget, up to the shared maximum. Settings retain their independent navigation width.

Windows limits the native window region to the capsule outline, including DPI scaling, so the transparent canvas cannot intercept interaction with other applications. Hover focus is requested on pointer entry only; Windows reads the system cursor position instead of relying on stale egui coordinates. Automatic expansion or an open settings panel does not itself request focus.

The clock icon has hour/minute hands only. Performance application rankings include cached application icons extracted on the sampler worker (Windows Shell icons, Linux desktop/theme icons, macOS bundle icons), with a generic application glyph when no icon is accessible. Network rates explicitly label upstream/downstream. Global accent selection is limited to six presets; only those preset values are accepted. Bottom navigation text scales gently from 9 to 12 logical points with monitor resolution.

Performance rankings aggregate all processes before selecting the top ten applications. Executable paths identify applications; the outer macOS app bundle includes nested helper apps. CPU and RSS totals are summed and rows show each application's process count. Distinct paths are kept separate even when display names match. GPU/downstream/upstream use equal-width horizontal tiles.

Music uses the declarative normalized Slider control for playback progress. Pointer dragging previews the target and dispatches one action on release; arrow keys adjust by one percent. The runtime clamps the position to track duration and gates it on the player's seek capability. Windows uses GSMTC playback-position commands; Linux uses MPRIS SetPosition with the current track object path. Unknown duration or unsupported seeking disables the control.

SVG textures are cached by target physical dimensions as well as styling. Small icons render at four times their target size and downsample in premultiplied alpha to reduce jagged circles and hands at fractional DPI.
