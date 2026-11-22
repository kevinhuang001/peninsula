# Extension platform V2

## Registration

`builtins::runtimes()` supplies the extension implementations. Each extension provides one `ExtensionDescriptor` with its business settings and static contributions. A `ContributionDescriptor` owns its settings schema, default visibility, execution policy and dependencies. The registry reserves the notification center for Core and validates schemas and ownership before a contribution becomes visible.

A running extension may return a complete snapshot from `RuntimeExtension::dynamic_contributions`. Returning `None` keeps the previous snapshot. Returning an empty vector removes all of that owner's dynamic entries. Dynamic IDs use the owner's ID followed by a slash and a stable local key. They can represent Compact, Widget and Notification entries in the same snapshot. A collision, invalid schema or invalid resource dependency rejects the whole update.

Core saves accepted snapshots to `contribution-catalog.json`. It reloads this catalog before starting extensions so a stopped extension's entries remain configurable. The registry exposes both static and dynamic entries to the same settings and rendering paths. Removing a dynamic entry invalidates its views, stops its scope and removes its settings and preferences. Notification history remains available.

## Settings and migration

Compact, Widget and Notification settings are grouped by extension. Each entry has its own desired enable state. Compact and Widget also have an order. The extensions page shows the owner switch, business configuration and Doctor status. The fixed navigation includes a separate Notifications page. Notification settings use the same schema controls as business settings.

The preference file stores desired enable states and order. `extension-settings.json` stores business settings, and `contribution-settings.json` stores per-entry values. A contribution runs when the owner and the contribution are enabled and any declared setup is complete. Stopping an owner preserves its children's desired states. Failed startups preserve the user's desired state as well as the failure for Doctor.

`migrate_settings` upgrades a saved schema version. The host preserves the original record when the runtime has no migration or the migrated values fail validation. `contribution_preference_migration` imports old notification switches before the host upgrades their owner settings. Market and data-source registrations use legacy values to seed the corresponding per-entry settings when a user first upgrades.

## Lifecycle and scheduling

An owner controls its own initialization and shutdown. Core starts each effective contribution with an independent context and generation. A background service referenced by several contributions stays active as long as a running consumer needs it. When the last consumer stops, Core releases the service. Service dependencies refer to services declared by the same owner.

Use `on_contribution_start` to acquire runtime resources and `on_contribution_stop` to release them. Spawn work through `context.tasks()` and check the supplied cancellation token. Use `context.schedule_at` for timers and handle owned events through `on_contribution_event`. Timer keys include the scope and generation. The host ignores events from old generations and cancels registered deadlines on stop. `EventDriven`, `Interval`, `WhenVisible` and `OnEnable` determine when Core calls `on_contribution_update`.

For HTTP, run the full asynchronous transfer with `sdk::http::run_cancellable`, including body reads. For child processes, pass the same scope token to `ProcessExecutor::run_with`. Core cancels a stopping scope and reaps completed workers. A scope that exceeds the shutdown deadline appears in Doctor, and Core retains the handle for reaping. In-process extensions are trusted Rust code. Capability flags describe the host API and do not create a security sandbox.

Market separates its overview, per-asset Compact values and per-asset notification rules. The quote service fetches the full watchlist for the overview and individual assets for other consumers. Price alerts read their threshold, comparison and cooldown from their own schemas.

## Declarative data sources

Installed `peninsula.datasource/v1` definitions generate Compact and optional Notification entries under the Data Sources group. The source's own switch controls whether it can make requests. Core supplies the separate Compact and Notification switches and the notification cooldown. Automatic refreshing requires at least one effective consumer.

Import still authorizes the definition's request origin and any private network access. The instance stores its request inputs and refresh cadence. Definitions, settings and sampled values remain separate. Disabling or replacing a source cancels its request and rejects its old results.

When a notification rule matches, the request path checks the source's active notification state. Core checks that state again at acceptance. The cooldown timestamp advances after notification history accepts the item. On startup the manager reconciles an interrupted state write with the history file.

## Mascot

`core::mascot` owns the independent leading slot and its renderer. Its visibility and Full/Reduced/Off motion settings live under Capsule. The passive slot is 38 logical pixels wide and uses the same character scale as vault unlock.

Independent blinks recur every 3–7 seconds. Full expressions start after 3–5 seconds of visibility, then wait 8–18 seconds after the last expression ends. Extensions declare owner-local IDs, typed motions, playback duration, bounded weights and either a state conjunction or an event trigger with its own expiry. When state expressions are active, scheduling selects their category 75% of the time. Events enter a bounded FIFO and can interrupt ordinary actions. Stopping an owner removes its pending and active actions. A hidden slot pauses playback while queued events retain their wall-clock expiry.

Compact and vault unlock share a procedural egui renderer. The unlock view supplies interaction states and has precedence over extension actions. Reduced scales movement and softens changes in expression. Off draws immediate static expressions without animation repaint requests.

## Search and Doctor

Search fields declare a provider, placeholder and minimum query length. Core runs the provider on a managed scope and cancels the previous query when a new one supersedes it. The selected result persists as one validated setting value. The in-progress query and unselected results stay in memory.

Doctor shows registered checks with a stable ID, status, detail and optional remediation. Read the check snapshots on the UI thread. Run process, browser and network checks on owned tasks. The host adds registration, settings and lifecycle failures to the report.

## Prompt contributions

Prompt is a one-shot exclusive capsule presentation with its own layout contract, separate from Compact items. It contains a declarative fallback view and a display duration. Optional `layers` position any number of `ViewNode`s in fractional rectangles across the entire capsule (left, top, width, height, each from 0 to 1); each layer can use the full Motion renderer independently. The optional `accent` colors the host's animated ambient light and outline. With no layers, the fallback view stays centered. Prompt content is painted directly on the capsule shell without an inner chip or a prescribed icon/text arrangement. Reduced Motion and Motion Off freeze the host lighting and declarative motion.

The host accepts a Prompt only from its registered running owner, then appends it to one FIFO queue; there is no priority field or persistent `while` trigger. While a Prompt is visible it replaces all ordinary Compact items, including the Mascot slot. Its duration pauses while the capsule is hidden by an expanded Widget, settings or a notification alert. When the queue drains, normal Compact composition resumes.

Every Prompt receives the host entrance fade and moving ambient highlight, including extensions that only supply the fallback view. The battery and USB built-ins add their own independently animated layers. USB samples the platform's device inventory off the UI thread (Linux sysfs, macOS System Profiler, Windows PnP), establishes a silent initial baseline, and queues a Prompt only for later insertions and removals.

Prompt presentation is separate from notification policy. Do Not Disturb suppresses notification popups only; it does not pause, suppress or reorder Prompts. Notifications and Prompts keep independent queues and lifetimes.

Notification history is stored atomically in a separate `notifications.json`, including acknowledgements and deduplication identity. Closing an alert leaves its center record pending. DND suppresses popups, not storage. Restored records appear in the center without replaying every popup. Source shutdown does not delete records. Corrupt files are preserved for recovery.

Pomodoro preserves paused state on stop/restart, has schema-driven focus/break durations and optional automatic next phase. Battery owns power sampling and charging visuals independently of System Monitor. Clock owns time/date formatting and its analog SVG; Core no longer reads wall-clock time to paint the clock icon.

## Validation

Unit and headless egui tests cover atomic mixed-kind snapshots, stopped catalog persistence, shared-service lifetime, stale timer rejection, cancellation during HTTP, notification history on disable, data-source refresh and cooldown, settings migrations, and existing platform adapters.

CI runs native tests and release builds on Linux, macOS and Windows. Formatting runs on every platform and strict all-target Clippy runs on Linux. Native OS integration and backend credentials remain dependent on their respective runners/machines.

Settings stay expanded while open without requiring the pin control. Ordinary Widgets use screen-dependent width limits with square-root growth capped at 350–700 points on large displays (280–560 at 1440 logical points); Music uses the same host minimum: short content keeps that width, and only content exceeding it expands the Widget, up to the shared maximum. Settings retain their independent navigation width.

Windows limits the native window region to the capsule outline, including DPI scaling, so the transparent canvas cannot intercept interaction with other applications. Hover focus is requested on pointer entry only; Windows reads the system cursor position instead of relying on stale egui coordinates. Automatic expansion or an open settings panel does not itself request focus.

The clock icon has hour/minute hands only. Performance application rankings include cached application icons extracted on the sampler worker (Windows Shell icons, Linux desktop/theme icons, macOS bundle icons), with a generic application glyph when no icon is accessible. Network rates explicitly label upstream/downstream. Global accent selection is limited to six presets; only those preset values are accepted. Bottom navigation text scales gently from 9 to 12 logical points with monitor resolution.

Performance rankings aggregate all processes before selecting the top ten applications. Executable paths identify applications; the outer macOS app bundle includes nested helper apps. CPU and RSS totals are summed and rows show each application's process count. Distinct paths are kept separate even when display names match. GPU/downstream/upstream use equal-width horizontal tiles.

Music uses the declarative normalized Slider control for playback progress. Pointer dragging previews the target and dispatches one action on release; arrow keys adjust by one percent. The runtime clamps the position to track duration and gates it on the player's seek capability. Windows uses GSMTC playback-position commands; Linux uses MPRIS SetPosition with the current track object path. Unknown duration or unsupported seeking disables the control.

SVG textures are cached by target physical dimensions as well as styling. Small icons render at four times their target size and downsample in premultiplied alpha to reduce jagged circles and hands at fractional DPI.
