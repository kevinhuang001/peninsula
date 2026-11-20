use crate::ui::declarative::{RefreshHint, WidgetBounds};
use crate::{
    core::{settings::SettingsStore, views::ViewCache},
    extension::{
        Action, ContributionKind, DiagnosticCheck, ExtensionContext, ExtensionId,
        ExtensionLifecycleRecord, ExtensionOutput, ExtensionRegistry, LifecycleManager,
        Notification, NotificationAction, NotificationPresentation, Prompt, ReloadPolicy,
        RuntimeExtension, SearchOption, Settings, WidgetView, WidgetVisibility,
        NOTIFICATION_CENTER_ID,
    },
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

struct SettingSearchJob {
    query: String,
    receiver: mpsc::Receiver<Result<Vec<SearchOption>, String>>,
}

struct ActivePrompt {
    prompt: Prompt,
    remaining: Duration,
    deadline: Option<Instant>,
}

pub struct SettingSearchResult {
    pub extension_id: ExtensionId,
    pub setting_key: String,
    pub query: String,
    pub result: Result<Vec<SearchOption>, String>,
}
/// All feature implementations are registered through this host collection.
/// It routes by extension IDs and keeps feature logic out of App and Capsule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleState {
    Registered,
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}
pub struct ExtensionManager {
    registry: ExtensionRegistry,
    states: Vec<LifecycleState>,
    generations: Vec<u64>,
    last_updates: Vec<Option<Instant>>,
    owned_tasks: Vec<std::collections::HashSet<String>>,
    visible: bool,
    motion_policy: crate::core::mascot::MotionPolicy,
    extensions: Vec<Box<dyn RuntimeExtension>>,
    lifecycle: LifecycleManager,
    context: ExtensionContext,
    owners: HashMap<ExtensionId, usize>,
    settings: SettingsStore,
    effective_settings: HashMap<ExtensionId, Settings>,
    startup_output: Vec<ExtensionOutput>,
    widget_cache: ViewCache<ExtensionId, Option<Arc<WidgetView>>>,
    compact_cache: ViewCache<usize, Vec<Arc<crate::extension::CompactView>>>,
    setting_searches: HashMap<(ExtensionId, String), SettingSearchJob>,
    prompt_queue: VecDeque<Prompt>,
    active_prompt: Option<ActivePrompt>,
    prompt_visible: bool,
}

impl ExtensionManager {
    pub fn new(extensions: Vec<Box<dyn RuntimeExtension>>) -> Self {
        let context = ExtensionContext::new(user_data_dir(), user_config_dir());
        Self::with_context(extensions, context, Some(lifecycle_path()))
    }
    fn with_context(
        extensions: Vec<Box<dyn RuntimeExtension>>,
        context: ExtensionContext,
        lifecycle_path: Option<PathBuf>,
    ) -> Self {
        let mut host = Self {
            registry: {
                let mut registry = ExtensionRegistry::default();
                for runtime in &extensions {
                    registry
                        .register(runtime.registration_descriptor())
                        .expect("valid extension descriptor");
                }
                registry
            },
            states: vec![LifecycleState::Registered; extensions.len()],
            generations: vec![0; extensions.len()],
            last_updates: vec![None; extensions.len()],
            owned_tasks: vec![Default::default(); extensions.len()],
            visible: true,
            motion_policy: Default::default(),
            extensions,
            lifecycle: LifecycleManager::load(lifecycle_path),
            settings: SettingsStore::open(context.config_dir().join("extension-settings.json")),
            context,
            owners: HashMap::new(),
            effective_settings: HashMap::new(),
            startup_output: vec![],
            widget_cache: ViewCache::default(),
            compact_cache: ViewCache::default(),
            setting_searches: HashMap::new(),
            prompt_queue: VecDeque::new(),
            active_prompt: None,
            prompt_visible: false,
        };
        for (index, extension) in host.extensions.iter_mut().enumerate() {
            for descriptor in [extension.registration_descriptor()] {
                for c in &descriptor.contributions {
                    host.owners.insert(c.id.clone(), index);
                }
                host.owners.insert(descriptor.id.clone(), index);
                if descriptor.settings.is_empty() {
                    continue;
                }
                let settings = host.settings.load(&descriptor).unwrap_or_else(|error| {
                    host.context
                        .log(&descriptor.id, &format!("Unable to load settings: {error}"));
                    SettingsStore::defaults(&descriptor)
                });
                host.effective_settings
                    .insert(descriptor.id.clone(), settings);
            }
        }
        host.start_enabled_extensions();
        host
    }

    pub fn configured_settings(&self, id: &ExtensionId) -> Option<&Settings> {
        self.settings
            .values(id)
            .or_else(|| self.effective_settings.get(id))
    }

    pub fn doctor(&self, id: &ExtensionId) -> Vec<DiagnosticCheck> {
        let Some(&owner) = self.owners.get(id) else {
            return vec![DiagnosticCheck::unknown(
                "extension-registration",
                "扩展注册",
                "扩展未注册。",
            )];
        };
        let mut checks = vec![match self.states[owner] {
            LifecycleState::Running => {
                DiagnosticCheck::passed("extension-runtime", "扩展运行状态", "扩展正在运行。")
            }
            LifecycleState::Failed => DiagnosticCheck::new(
                "extension-runtime",
                "扩展运行状态",
                crate::extension::DiagnosticStatus::Failed,
                self.status(id),
                Some("检查扩展报错信息，并修复所列依赖或配置。"),
            ),
            _ => DiagnosticCheck::unknown(
                "extension-runtime",
                "扩展运行状态",
                "扩展当前未运行，依赖项尚未完成检查。",
            ),
        }];
        checks.extend(self.extensions[owner].doctor());
        checks
    }

    pub fn search_setting(
        &mut self,
        id: &ExtensionId,
        setting_key: &str,
        query: &str,
    ) -> Result<(), String> {
        let owner = *self
            .owners
            .get(id)
            .ok_or_else(|| format!("settings owner {id} is not registered"))?;
        let descriptor = self
            .registry
            .extension(id)
            .ok_or_else(|| format!("settings owner {id} is not registered"))?;
        let field = descriptor
            .settings
            .iter()
            .find(|field| field.key == setting_key)
            .ok_or_else(|| format!("unknown search setting {setting_key}"))?;
        let search = field
            .search
            .as_ref()
            .ok_or_else(|| format!("setting {setting_key} does not declare search"))?;
        let query = query.trim();
        if query.chars().count() < search.min_query_chars {
            self.setting_searches
                .remove(&(id.clone(), setting_key.to_owned()));
            return Ok(());
        }
        let task = self.extensions[owner]
            .setting_search(&search.provider, query)
            .ok_or_else(|| format!("search provider {} is unavailable", search.provider))?;
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name(format!("settings-search-{}", id.as_str()))
            .spawn(move || {
                let _ = sender.send(task());
            })
            .map_err(|error| format!("unable to start setting search: {error}"))?;
        self.setting_searches.insert(
            (id.clone(), setting_key.to_owned()),
            SettingSearchJob {
                query: query.to_owned(),
                receiver,
            },
        );
        Ok(())
    }

    pub fn take_setting_search_results(&mut self) -> Vec<SettingSearchResult> {
        let keys: Vec<_> = self.setting_searches.keys().cloned().collect();
        let mut finished = Vec::new();
        let mut remove = Vec::new();
        for key in keys {
            let Some(job) = self.setting_searches.get(&key) else {
                continue;
            };
            match job.receiver.try_recv() {
                Ok(result) => {
                    finished.push(SettingSearchResult {
                        extension_id: key.0.clone(),
                        setting_key: key.1.clone(),
                        query: job.query.clone(),
                        result,
                    });
                    remove.push(key);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    finished.push(SettingSearchResult {
                        extension_id: key.0.clone(),
                        setting_key: key.1.clone(),
                        query: job.query.clone(),
                        result: Err("搜索任务意外结束".into()),
                    });
                    remove.push(key);
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        for key in remove {
            self.setting_searches.remove(&key);
        }
        finished
    }

    pub fn has_setting_searches(&self) -> bool {
        !self.setting_searches.is_empty()
    }

    pub fn active_prompt(&mut self, now: Instant, visible: bool) -> Option<Prompt> {
        self.prompt_visible = visible;
        let expired = self
            .active_prompt
            .as_ref()
            .and_then(|active| active.deadline)
            .is_some_and(|deadline| now >= deadline);
        if expired {
            self.active_prompt = None;
        } else if let Some(active) = &mut self.active_prompt {
            if let Some(deadline) = active.deadline.filter(|_| !visible) {
                active.remaining = deadline.saturating_duration_since(now);
                active.deadline = None;
            } else if active.deadline.is_none() && visible {
                active.deadline = Some(now + active.remaining);
            }
        }
        while self.active_prompt.is_none() && visible {
            let prompt = self.prompt_queue.pop_front()?;
            let valid = self
                .owners
                .get(&prompt.source_extension_id)
                .is_some_and(|owner| self.states[*owner] == LifecycleState::Running)
                && self
                    .registry
                    .get(&prompt.contribution_id)
                    .is_some_and(|contribution| contribution.kind == ContributionKind::Prompt)
                && self
                    .registry
                    .owner(&prompt.contribution_id)
                    .is_some_and(|owner| owner.id == prompt.source_extension_id);
            if valid {
                let duration = prompt.duration.max(Duration::from_millis(250));
                self.active_prompt = Some(ActivePrompt {
                    prompt,
                    remaining: duration,
                    deadline: Some(now + duration),
                });
            }
        }
        self.active_prompt
            .as_ref()
            .map(|active| active.prompt.clone())
    }
    pub fn update_settings(
        &mut self,
        id: &ExtensionId,
        values: Settings,
    ) -> Result<ExtensionOutput, String> {
        let (Some(&owner), Some(descriptor)) = (self.owners.get(id), self.registry.extension(id))
        else {
            return Err(format!("settings owner {id} is not registered"));
        };
        if descriptor.settings.is_empty() && values.is_empty() {
            return Ok(ExtensionOutput::default());
        }
        let changed = self.settings.update(descriptor, values)?;
        let effective = self.effective_settings.entry(id.clone()).or_default();
        let saved = self.settings.values(id).expect("settings just saved");
        let mut policy = None;
        for (key, reload) in changed {
            if reload != ReloadPolicy::Restart {
                effective.insert(key.clone(), saved[&key].clone());
                if policy.is_none() || reload == ReloadPolicy::Reload {
                    policy = Some(reload);
                }
            }
        }
        let Some(policy) = policy else {
            return Ok(ExtensionOutput::default());
        };
        let mut output = if self.states[owner] == LifecycleState::Running {
            self.extensions[owner].on_settings_changed(
                &self.context.scoped_at(id, self.generations[owner]),
                id,
                effective,
                policy,
            )
        } else {
            ExtensionOutput::default()
        };
        output.invalidated.push(id.clone());
        Ok(self.track_output(owner, output))
    }

    pub fn invalidate_views(&mut self, id: &ExtensionId) {
        self.widget_cache.invalidate(id);
        if let Some(owner) = self.owners.get(id) {
            self.compact_cache.invalidate(owner);
        }
    }

    pub fn mascot_sources(&self) -> Vec<crate::core::mascot::MascotSource> {
        self.extensions
            .iter()
            .enumerate()
            .filter(|(i, _)| self.states[*i] == LifecycleState::Running)
            .map(|(owner, e)| {
                (
                    self.registry.extensions()[owner].id.clone(),
                    e.mascot_expressions(),
                    e.mascot_state(),
                )
            })
            .collect()
    }
    pub fn data_dir(&self) -> &std::path::Path {
        self.context.data_dir()
    }
    pub fn next_wake(
        &self,
        now: Instant,
        compact_visible: bool,
        active_widget: Option<&ExtensionId>,
    ) -> Option<Instant> {
        let mut wake = active_widget.and_then(|id| self.widget_cache.refresh_at(id));
        wake = wake
            .into_iter()
            .chain(
                self.active_prompt
                    .as_ref()
                    .and_then(|active| active.deadline),
            )
            .chain(
                (self.prompt_visible
                    && self.active_prompt.is_none()
                    && !self.prompt_queue.is_empty())
                .then_some(now),
            )
            .chain((!self.setting_searches.is_empty()).then_some(now + Duration::from_millis(100)))
            .min();
        for owner in 0..self.extensions.len() {
            if self.states[owner] != LifecycleState::Running {
                continue;
            }
            let due = match self.registry.extensions()[owner].execution_policy {
                crate::extension::ExecutionPolicy::Interval(interval) => {
                    Some(self.last_updates[owner].unwrap_or(now) + interval)
                }
                crate::extension::ExecutionPolicy::WhenVisible if self.visible => {
                    Some(now + Duration::from_secs(1))
                }
                _ => None,
            };
            wake = wake.into_iter().chain(due).min();
            if compact_visible {
                wake = wake
                    .into_iter()
                    .chain(self.compact_cache.refresh_at(&owner))
                    .min();
            }
        }
        wake
    }
    pub fn state(&self, id: &ExtensionId) -> Option<LifecycleState> {
        self.owners.get(id).map(|&owner| self.states[owner])
    }
    pub fn running(&self, id: &ExtensionId) -> bool {
        self.owners
            .get(id)
            .is_some_and(|&owner| self.states[owner] == LifecycleState::Running)
    }
    pub fn status(&self, id: &ExtensionId) -> String {
        if let Some(failure) = self
            .lifecycle
            .record(id)
            .and_then(|r| r.last_failure.as_ref())
        {
            return failure.clone();
        }
        if self.running(id) {
            if let Some(message) = self
                .owners
                .get(id)
                .and_then(|&owner| self.extensions[owner].status_message())
            {
                return message;
            }
        }
        match self.state(id) {
            Some(LifecycleState::Running) => "运行中",
            Some(LifecycleState::Starting) => "启动中",
            Some(LifecycleState::Stopping) => "停止中",
            Some(LifecycleState::Registered) => "已注册",
            _ => "已停止",
        }
        .into()
    }
    pub fn set_motion_policy(&mut self, policy: crate::core::mascot::MotionPolicy) {
        if self.motion_policy != policy {
            self.motion_policy = policy;
            for (owner, runtime) in self.extensions.iter_mut().enumerate() {
                runtime.set_motion_policy(policy);
                self.compact_cache.invalidate(&owner);
            }
            for c in self.registry.by_kind(ContributionKind::Widget) {
                self.widget_cache.invalidate(&c.id);
            }
        }
    }
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
    fn start_enabled_extensions(&mut self) {
        for descriptor in self.registry.extensions().to_vec() {
            let enabled = self
                .lifecycle
                .record(&descriptor.id)
                .map_or(descriptor.default_enabled, |r| r.enabled);
            if enabled {
                if let Err(error) = self.set_enabled(&descriptor.id, true) {
                    self.context.log(&descriptor.id, &error);
                }
            }
        }
    }
    pub fn set_enabled(&mut self, id: &ExtensionId, enabled: bool) -> Result<(), String> {
        let owner = *self.owners.get(id).ok_or("unknown extension")?;
        let descriptor = self.registry.extensions()[owner].clone();
        let id = descriptor.id.clone();
        if self.running(&id) == enabled {
            return Ok(());
        }
        if enabled {
            self.generations[owner] += 1;
        }
        let context = self.context.scoped_at(&id, self.generations[owner]);
        let previous = self.lifecycle.record(&id).cloned();
        self.states[owner] = if enabled {
            LifecycleState::Starting
        } else {
            LifecycleState::Stopping
        };
        let extension = &mut self.extensions[owner];
        let mut configured = None;
        let result = if enabled {
            if let Some(settings) = self.effective_settings.get(&id) {
                let output =
                    extension.on_settings_changed(&context, &id, settings, ReloadPolicy::Live);
                configured = Some(output);
            }
            let prepare = if previous.as_ref().is_none_or(|r| !r.initialized) {
                extension.on_install(&context)
            } else if previous
                .as_ref()
                .is_some_and(|r| r.version != descriptor.version)
            {
                extension.on_upgrade(&context, &previous.as_ref().unwrap().version)
            } else {
                Ok(())
            };
            prepare
                .and_then(|()| extension.on_enable(&context))
                .and_then(|()| extension.on_start(&context))
        } else {
            extension
                .on_stop(&context)
                .and_then(|()| extension.on_disable(&context))
        };
        if result.is_err() && enabled {
            let _ = extension.on_stop(&context);
            let _ = extension.on_disable(&context);
        }
        if !enabled || result.is_err() {
            context.shutdown_tasks();
        }
        self.states[owner] = if result.is_err() {
            LifecycleState::Failed
        } else if enabled {
            LifecycleState::Running
        } else {
            LifecycleState::Stopped
        };
        if self.states[owner] != LifecycleState::Running {
            let owner_id = id.clone();
            self.prompt_queue
                .retain(|prompt| prompt.source_extension_id != owner_id);
            if self
                .active_prompt
                .as_ref()
                .is_some_and(|active| active.prompt.source_extension_id == owner_id)
            {
                self.active_prompt = None;
            }
            self.setting_searches
                .retain(|(search_owner, _), _| search_owner != &owner_id);
        }
        if self.states[owner] == LifecycleState::Running {
            if let Some(output) = configured {
                let output = self.track_output(owner, output);
                self.startup_output.push(output);
            }
        }
        self.last_updates[owner] = None;
        if self.states[owner] != LifecycleState::Running {
            self.startup_output.push(ExtensionOutput {
                cancel_tasks: self.owned_tasks[owner].drain().collect(),
                ..Default::default()
            });
        }
        for c in &descriptor.contributions {
            self.widget_cache.invalidate(&c.id);
        }
        self.compact_cache.invalidate(&owner);
        let record = ExtensionLifecycleRecord {
            version: descriptor.version,
            enabled: self.states[owner] == LifecycleState::Running,
            initialized: result.is_ok() || previous.as_ref().is_some_and(|r| r.initialized),
            last_result: if enabled { "on_start" } else { "on_stop" }.into(),
            last_failure: result.as_ref().err().cloned(),
        };
        self.lifecycle.set(id, record).map_err(|e| e.to_string())?;
        result
    }

    pub fn widget_ids(&self) -> Vec<ExtensionId> {
        std::iter::once(ExtensionId::builtin(NOTIFICATION_CENTER_ID))
            .chain(
                self.registry
                    .by_kind(ContributionKind::Widget)
                    .into_iter()
                    .filter(|d| {
                        self.owners.get(&d.id).is_some_and(|&owner| {
                            self.states[owner] == LifecycleState::Running
                                && self.extensions[owner].widget_visibility(&d.id)
                                    == WidgetVisibility::Visible
                        })
                    })
                    .map(|d| d.id.clone()),
            )
            .collect()
    }

    pub fn widget_size(
        &self,
        id: &ExtensionId,
        view: Option<&WidgetView>,
        host_limit: [f32; 2],
        content_size: [Option<f32>; 2],
    ) -> [f32; 2] {
        let center = crate::core::notifications::descriptor();
        let descriptor = self
            .registry
            .get(id)
            .or_else(|| (id.as_str() == NOTIFICATION_CENTER_ID).then_some(&center));
        let preferred = descriptor.map_or([300.0, 76.0], |d| d.size_hint);
        let limits = descriptor.map_or_else(WidgetBounds::default, |d| d.widget_bounds);
        let requested = view.map_or(limits, |view| view.bounds);
        let resizable = limits.resizable || requested.resizable;
        std::array::from_fn(|i| {
            let upper = [limits.max[i], requested.max[i]]
                .into_iter()
                .flatten()
                .filter(|v| v.is_finite() && *v >= 0.0)
                .fold(host_limit[i].max(0.0), f32::min);
            let lower = [limits.min[i], requested.min[i]]
                .into_iter()
                .filter(|v| v.is_finite())
                .fold(0.0, f32::max)
                .min(upper);
            let default = preferred[i].is_finite() && preferred[i] > 0.0;
            let runtime_hint = view
                .map(|v| v.preferred_size[i])
                .filter(|v| v.is_finite() && *v > 0.0);
            let size = if let Some(content) = content_size[i] {
                content
            } else if default && !resizable {
                preferred[i]
            } else {
                runtime_hint.unwrap_or(if default {
                    preferred[i]
                } else {
                    [300.0, 76.0][i]
                })
            };
            size.clamp(lower, upper)
        })
    }

    pub fn widget_title(&self, id: &ExtensionId) -> String {
        if id.as_str() == NOTIFICATION_CENTER_ID {
            "通知中心".into()
        } else {
            self.registry
                .get(id)
                .map_or_else(|| "扩展".into(), |d| d.title.clone())
        }
    }

    pub fn widget_view(&mut self, id: &ExtensionId, now: Instant) -> Option<Arc<WidgetView>> {
        let descriptor = self
            .registry
            .get(id)
            .filter(|d| d.kind == ContributionKind::Widget)?;
        let owner = *self.owners.get(id)?;
        if self.states[owner] != LifecycleState::Running {
            return None;
        }
        let extension = self.extensions.get(owner)?;
        self.widget_cache
            .get_or_build(id.clone(), now, || {
                let view = extension.widget_view(id).map(|mut view| {
                    view.bounds.fit_width |= descriptor.widget_bounds.fit_width;
                    view.bounds.fit_height |= descriptor.widget_bounds.fit_height;
                    if view.bounds.overflow == crate::ui::declarative::Overflow::Clip {
                        view.bounds.overflow = descriptor.widget_bounds.overflow;
                    }
                    Arc::new(view)
                });
                let hint = view.as_ref().map_or(descriptor.refresh_hint, |view| {
                    if view.refresh_hint == RefreshHint::OnInvalidation {
                        descriptor.refresh_hint
                    } else {
                        view.refresh_hint
                    }
                });
                (
                    view,
                    (hint == RefreshHint::EverySecond).then_some(Duration::from_secs(1)),
                )
            })
            .clone()
    }

    pub fn dynamic_compact_items(&self) -> Vec<(ExtensionId, Vec<(ExtensionId, String)>)> {
        self.extensions
            .iter()
            .enumerate()
            .filter(|(index, _)| self.states[*index] == LifecycleState::Running)
            .filter_map(|(index, extension)| {
                extension
                    .dynamic_compact_items()
                    .map(|items| (self.registry.extensions()[index].id.clone(), items))
            })
            .collect()
    }

    pub fn compact_views(&mut self, now: Instant) -> Vec<Arc<crate::extension::CompactView>> {
        // Build only stale sources, then borrow the resulting snapshots for this frame.
        // The cache borrows cannot overlap insertion, so do this in two passes.
        let registered_dynamic_ids: HashSet<_> = self
            .dynamic_compact_items()
            .into_iter()
            .flat_map(|(_, items)| items.into_iter().map(|(id, _)| id))
            .collect();
        for (index, extension) in self
            .extensions
            .iter()
            .enumerate()
            .filter(|(i, _)| self.states[*i] == LifecycleState::Running)
        {
            let dynamic_ids: HashSet<_> = extension
                .dynamic_compact_items()
                .unwrap_or_default()
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            self.compact_cache.get_or_build(index, now, || {
                (
                    extension
                        .compact_views()
                        .into_iter()
                        .filter(|view| {
                            self.owners.get(&view.id) == Some(&index)
                                || dynamic_ids.contains(&view.id)
                        })
                        .map(Arc::new)
                        .collect(),
                    extension.compact_refresh_interval(),
                )
            });
        }
        self.extensions
            .iter()
            .enumerate()
            .filter(|(i, _)| self.states[*i] == LifecycleState::Running)
            .flat_map(|(index, _)| self.compact_cache.get(&index).into_iter().flatten())
            .filter(|view| view.visibility == crate::extension::CompactVisibility::Visible)
            .filter(|view| {
                self.registry
                    .get(&view.id)
                    .is_some_and(|d| d.kind == ContributionKind::Compact)
                    || registered_dynamic_ids.contains(&view.id)
            })
            .cloned()
            .collect()
    }

    fn track_output(&mut self, owner: usize, mut output: ExtensionOutput) -> ExtensionOutput {
        let id = self.registry.extensions()[owner].id.clone();
        for task in &mut output.schedule {
            task.task_id = format!("{id}::{}::{}", self.generations[owner], task.task_id);
            self.owned_tasks[owner].insert(task.task_id.clone());
        }
        for task in &mut output.cancel_tasks {
            *task = format!("{id}::{}::{task}", self.generations[owner]);
            self.owned_tasks[owner].remove(task);
        }
        output.notifications.retain(|n| {
            self.registry
                .get(&n.source_id)
                .is_some_and(|c| c.kind == ContributionKind::Notification)
                && self
                    .registry
                    .owner(&n.source_id)
                    .is_some_and(|d| d.id == id)
                && self.extensions[owner].handles_notification(&n.source_id)
        });
        output.resolved_notifications.retain(|key| {
            self.registry
                .owner(&key.source_extension_id)
                .is_some_and(|descriptor| descriptor.id == id)
        });
        output.prompts.retain(|prompt| {
            prompt.source_extension_id == id
                && self.registry.get(&prompt.contribution_id).is_some_and(|c| {
                    c.kind == ContributionKind::Prompt
                        && self
                            .registry
                            .owner(&c.id)
                            .is_some_and(|owner| owner.id == id)
                })
                && prompt.duration > Duration::ZERO
                && prompt.min_width.is_finite()
                && prompt.preferred_width.is_finite()
                && prompt.min_width > 0.0
                && prompt.preferred_width >= prompt.min_width
                && serde_json::to_vec(&prompt.content)
                    .is_ok_and(|encoded| encoded.len() <= 64 * 1024)
        });
        let prompts = std::mem::take(&mut output.prompts);
        for prompt in prompts {
            output.invalidated.push(prompt.contribution_id.clone());
            self.prompt_queue.push_back(prompt);
        }
        output
    }
    pub fn update(&mut self, now: Instant) -> Vec<ExtensionOutput> {
        let mut outputs = std::mem::take(&mut self.startup_output);
        for owner in 0..self.extensions.len() {
            if self.states[owner] != LifecycleState::Running {
                continue;
            }
            let due = match self.registry.extensions()[owner].execution_policy {
                crate::extension::ExecutionPolicy::EventDriven => false,
                crate::extension::ExecutionPolicy::WhenVisible => self.visible,
                crate::extension::ExecutionPolicy::OnEnable => self.last_updates[owner].is_none(),
                crate::extension::ExecutionPolicy::Interval(d) => self.last_updates[owner]
                    .is_none_or(|last| now.saturating_duration_since(last) >= d),
            };
            if due {
                let output = self.extensions[owner].update(now);
                self.last_updates[owner] = Some(now);
                outputs.push(self.track_output(owner, output));
            }
        }
        outputs
    }
    pub fn on_event(
        &mut self,
        event: &crate::extension::Event,
        now: Instant,
    ) -> Vec<ExtensionOutput> {
        let mut outputs = Vec::new();
        for owner in 0..self.extensions.len() {
            if self.states[owner] != LifecycleState::Running {
                continue;
            }
            let is_task =
                matches!(event, crate::extension::Event::Custom(task) if task.contains("::"));
            if !is_task
                && !self.registry.extensions()[owner]
                    .contributions
                    .iter()
                    .any(|c| c.kind == ContributionKind::EventHandler)
            {
                continue;
            }
            let event = if let crate::extension::Event::Custom(task) = event {
                if let Some(local) = task.splitn(3, "::").nth(2) {
                    if !self.owned_tasks[owner].remove(task) {
                        continue;
                    }
                    crate::extension::Event::Custom(local.into())
                } else {
                    event.clone()
                }
            } else {
                event.clone()
            };
            let output = self.extensions[owner].on_event(&event, now);
            outputs.push(self.track_output(owner, output));
        }
        outputs
    }
    pub fn on_action(
        &mut self,
        source_id: &ExtensionId,
        action: Action,
        now: Instant,
    ) -> Result<ExtensionOutput, String> {
        let owner = *self.owners.get(source_id).ok_or("unknown action source")?;
        if self.states[owner] != LifecycleState::Running {
            return Err("extension stopped".into());
        }
        let output = self.extensions[owner].on_action(action, now);
        Ok(self.track_output(owner, output))
    }

    pub fn drain_context_tasks(&mut self) -> ExtensionOutput {
        let (scheduled, mut canceled) = self.context.drain_tasks();
        let schedule = scheduled
            .into_iter()
            .filter_map(|(task_id, deadline)| {
                let owner = task_id
                    .split_once("::")
                    .and_then(|(id, _)| ExtensionId::new(id).ok())
                    .and_then(|id| self.owners.get(&id).copied())?;
                if self.states[owner] == LifecycleState::Running {
                    self.owned_tasks[owner].insert(task_id.clone());
                    Some(crate::extension::ScheduledTask { task_id, deadline })
                } else {
                    canceled.push(task_id);
                    None
                }
            })
            .collect();
        ExtensionOutput {
            schedule,
            cancel_tasks: canceled,
            ..Default::default()
        }
    }

    pub fn on_notifications_committed(&mut self, notifications: &[Notification]) {
        for (i, extension) in self.extensions.iter_mut().enumerate() {
            if self.states[i] != LifecycleState::Running {
                continue;
            }
            extension.on_notifications_committed(notifications);
        }
    }

    pub fn notification_presentation(
        &self,
        notification: &Notification,
    ) -> NotificationPresentation {
        self.extensions
            .iter()
            .enumerate()
            .filter(|(i, _)| self.states[*i] == LifecycleState::Running)
            .map(|(_, e)| e)
            .find(|extension| extension.handles_notification(&notification.source_id))
            .map_or_else(
                || NotificationPresentation::from_notification(notification),
                |extension| NotificationPresentation {
                    alert: extension.alert_view(notification),
                    center_item: extension.center_item_view(notification),
                },
            )
    }

    pub fn on_notification_action(
        &mut self,
        notification: &Notification,
        action: &NotificationAction,
    ) -> Result<(), String> {
        let Some(owner) = self
            .extensions
            .iter()
            .enumerate()
            .filter(|(i, _)| self.states[*i] == LifecycleState::Running)
            .find_map(|(owner, extension)| {
                (self.registry.get(&notification.source_id).is_some()
                    && extension.handles_notification(&notification.source_id))
                .then_some(owner)
            })
        else {
            return Err(format!(
                "no notification extension registered for {}",
                notification.source_id
            ));
        };
        let result = self.extensions[owner].on_notification_action(notification, action);
        for contribution in &self.registry.extensions()[owner].contributions {
            self.widget_cache.invalidate(&contribution.id);
        }
        self.compact_cache.invalidate(&owner);
        result
    }
}

impl Drop for ExtensionManager {
    fn drop(&mut self) {
        for (i, extension) in self.extensions.iter_mut().enumerate() {
            if self.states[i] != LifecycleState::Running {
                continue;
            }
            let context = self
                .context
                .scoped_at(&extension.descriptor().id, self.generations[i]);
            if let Err(error) = extension.on_stop(&context) {
                eprintln!("Extension on_stop failed: {error}");
            }
            context.shutdown_tasks();
        }
    }
}

fn user_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("IslandLab");
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Library/Application Support/island-lab");
    }
    #[cfg(target_os = "linux")]
    {
        return std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join(".local/share")
            })
            .join("island-lab");
    }
    #[allow(unreachable_code)]
    std::env::temp_dir().join("island-lab")
}

fn user_config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("IslandLab");
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Library/Application Support/island-lab");
    }
    #[cfg(target_os = "linux")]
    {
        return std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join(".config")
            })
            .join("island-lab");
    }
    #[allow(unreachable_code)]
    std::env::temp_dir().join("island-lab")
}

fn lifecycle_path() -> PathBuf {
    user_data_dir().join("extension-lifecycle.tsv")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::{
        Capability, ContributionDescriptor as C, ExecutionPolicy, ExtensionDescriptor as D,
        ScheduledTask, SettingSchema, SettingValue,
    };
    use std::sync::{Arc, Mutex};
    #[derive(Default)]
    struct Observations {
        starts: usize,
        stops: usize,
        updates: usize,
        events: usize,
        configured: Settings,
    }
    struct Probe {
        observations: Arc<Mutex<Observations>>,
        fail: bool,
    }
    impl crate::extension::NotificationPresentationProvider for Probe {}
    impl RuntimeExtension for Probe {
        fn descriptor(&self) -> D {
            let mut d = D::builtin(
                "test.module",
                "Probe",
                "",
                vec![
                    C::builtin(
                        "test.module/widget",
                        ContributionKind::Widget,
                        "Page",
                        "",
                        true,
                        1,
                    ),
                    C::builtin(
                        "test.module/compact.a",
                        ContributionKind::Compact,
                        "A",
                        "",
                        true,
                        0,
                    ),
                    C::builtin(
                        "test.module/compact.b",
                        ContributionKind::Compact,
                        "B",
                        "",
                        true,
                        1,
                    ),
                ],
            );
            d.contributions[0].capabilities.push(Capability::Event);
            d.settings = vec![
                SettingSchema::new("minutes", SettingValue::Integer(25), "Minutes")
                    .range(1.0, 180.0),
            ];
            d.execution_policy = ExecutionPolicy::Interval(Duration::from_secs(1));
            d
        }
        fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
            self.observations.lock().unwrap().starts += 1;
            ctx.schedule_at("from-context", Instant::now() + Duration::from_secs(30));
            if self.fail {
                Err("startup failed".into())
            } else {
                Ok(())
            }
        }
        fn on_stop(&mut self, _: &ExtensionContext) -> Result<(), String> {
            self.observations.lock().unwrap().stops += 1;
            Ok(())
        }
        fn update(&mut self, _: Instant) -> ExtensionOutput {
            self.observations.lock().unwrap().updates += 1;
            Default::default()
        }
        fn on_event(&mut self, _: &crate::extension::Event, _: Instant) -> ExtensionOutput {
            self.observations.lock().unwrap().events += 1;
            Default::default()
        }
        fn compact_views(&self) -> Vec<crate::extension::CompactView> {
            ["test.module/compact.a", "test.module/compact.b"]
                .map(|id| crate::extension::CompactView {
                    id: ExtensionId::builtin(id),
                    source_extension_id: ExtensionId::builtin("test.module"),
                    content: crate::extension::ViewNode::text("", Default::default()),
                    visibility: Default::default(),
                    min_width: 10.0,
                    preferred_width: 10.0,
                    fit_width: false,
                    priority: 0,
                    tooltip: None,
                })
                .into()
        }
        fn on_action(&mut self, _: Action, now: Instant) -> ExtensionOutput {
            ExtensionOutput {
                schedule: vec![ScheduledTask {
                    task_id: "deadline".into(),
                    deadline: now + Duration::from_secs(1),
                }],
                ..Default::default()
            }
        }
        fn on_settings_changed(
            &mut self,
            _: &ExtensionContext,
            _: &ExtensionId,
            values: &Settings,
            _: ReloadPolicy,
        ) -> ExtensionOutput {
            self.observations.lock().unwrap().configured = values.clone();
            Default::default()
        }
    }
    fn host(root: &std::path::Path, probe: Probe) -> ExtensionManager {
        ExtensionManager::with_context(
            vec![Box::new(probe)],
            ExtensionContext::new(root.into(), root.join("config")),
            Some(root.join("lifecycle.tsv")),
        )
    }
    #[test]
    fn stop_is_idempotent_and_removes_all_contributions_subscriptions_and_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let log = Arc::new(Mutex::new(Observations::default()));
        let mut h = host(
            dir.path(),
            Probe {
                observations: log.clone(),
                fail: false,
            },
        );
        let id = ExtensionId::builtin("test.module");
        let now = Instant::now();
        assert_eq!(h.compact_views(now).len(), 2);
        assert_eq!(h.widget_ids().len(), 2);
        h.set_enabled(&id, true).unwrap();
        assert_eq!(log.lock().unwrap().starts, 1);
        assert_eq!(
            h.drain_context_tasks().schedule[0].task_id,
            "test.module::1::from-context"
        );
        let output = h.on_action(&id, Action::Toggle, now).unwrap();
        assert_eq!(output.schedule[0].task_id, "test.module::1::deadline");
        h.update(now);
        h.update(now + Duration::from_millis(10));
        assert_eq!(log.lock().unwrap().updates, 1);
        h.set_enabled(&id, false).unwrap();
        h.set_enabled(&id, false).unwrap();
        assert_eq!(log.lock().unwrap().stops, 1);
        let outputs = h.update(now + Duration::from_secs(2));
        let cancelled: Vec<_> = outputs.into_iter().flat_map(|o| o.cancel_tasks).collect();
        assert!(
            cancelled.contains(&"test.module::1::deadline".into())
                && cancelled.contains(&"test.module::1::from-context".into())
        );
        assert_eq!(
            h.widget_ids(),
            [ExtensionId::builtin(NOTIFICATION_CENTER_ID)]
        );
        assert!(h.compact_views(now).is_empty());
        assert!(h
            .on_event(
                &crate::extension::Event::Custom("test.module::1::deadline".into()),
                now
            )
            .is_empty());
        assert_eq!(log.lock().unwrap().events, 0);
        h.update_settings(&id, [("minutes".into(), SettingValue::Integer(42))].into())
            .unwrap();
        assert_ne!(
            log.lock().unwrap().configured["minutes"],
            SettingValue::Integer(42)
        );
        h.set_enabled(&id, true).unwrap();
        assert_eq!(
            log.lock().unwrap().configured["minutes"],
            SettingValue::Integer(42)
        );
        assert_eq!(h.compact_views(now).len(), 2);
        let fresh = h
            .on_action(&id, Action::Toggle, now)
            .unwrap()
            .schedule
            .remove(0)
            .task_id;
        assert!(h
            .on_event(
                &crate::extension::Event::Custom("test.module::1::deadline".into()),
                now
            )
            .is_empty());
        h.on_event(&crate::extension::Event::Custom(fresh), now);
        assert_eq!(log.lock().unwrap().events, 1);
    }
    #[test]
    fn failed_start_rolls_back_and_disabled_configuration_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let log = Arc::new(Mutex::new(Observations::default()));
        let id = ExtensionId::builtin("test.module");
        {
            let h = host(
                dir.path(),
                Probe {
                    observations: log.clone(),
                    fail: true,
                },
            );
            assert_eq!(h.state(&id), Some(LifecycleState::Failed));
            assert_eq!(h.widget_ids().len(), 1);
            assert_eq!(log.lock().unwrap().stops, 1);
        }
        let mut h = host(
            dir.path(),
            Probe {
                observations: log.clone(),
                fail: false,
            },
        );
        assert!(!h.running(&id));
        assert_eq!(log.lock().unwrap().starts, 1);
        assert!(h.drain_context_tasks().schedule.is_empty());
        h.set_enabled(&id, true).unwrap();
        assert!(h.running(&id));
    }
}
