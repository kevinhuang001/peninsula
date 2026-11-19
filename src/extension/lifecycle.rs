use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

use super::ExtensionId;

/// Host services available to in-process extension lifecycle hooks. This is an
/// application convenience API, not a permission boundary.
#[derive(Clone, Debug)]
pub struct ExtensionContext {
    owner: Option<ExtensionId>,
    generation: u64,
    data_dir: PathBuf,
    config_dir: PathBuf,
    scheduled: Arc<Mutex<Vec<(String, Instant)>>>,
    canceled: Arc<Mutex<Vec<String>>>,
    task_scopes: crate::sdk::tasks::TaskRegistry,
    task_scope: Option<crate::sdk::tasks::TaskScope>,
}

impl ExtensionContext {
    pub fn new(data_dir: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            owner: None,
            generation: 0,
            data_dir,
            config_dir,
            scheduled: Arc::new(Mutex::new(Vec::new())),
            canceled: Arc::new(Mutex::new(Vec::new())),
            task_scopes: crate::sdk::tasks::TaskRegistry::default(),
            task_scope: None,
        }
    }

    pub fn scoped(&self, owner: &ExtensionId) -> Self {
        self.scoped_at(owner, self.generation)
    }
    pub fn scoped_at(&self, owner: &ExtensionId, generation: u64) -> Self {
        let mut context = self.clone();
        context.owner = Some(owner.clone());
        context.generation = generation;
        context.task_scope = Some(self.task_scopes.scope(owner, generation));
        context
    }
    fn task_key(&self, task: String) -> String {
        self.owner.as_ref().map_or_else(
            || task.clone(),
            |owner| format!("{owner}::{}::{task}", self.generation),
        )
    }
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
    pub fn extension_id(&self) -> Option<&ExtensionId> {
        self.owner.as_ref()
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn sdk(&self) -> crate::sdk::SdkContext {
        crate::sdk::SdkContext::new(self.clone())
    }
    pub fn tasks(&self) -> crate::sdk::tasks::TaskScope {
        self.task_scope.clone().unwrap_or_default()
    }
    pub fn shutdown_tasks(&self) {
        if let Some(owner) = self.owner.as_ref() {
            self.task_scopes.cancel(owner, self.generation);
        }
    }
    pub(crate) fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn extension_data_dir(&self, id: &ExtensionId) -> PathBuf {
        self.data_dir
            .join("extensions")
            .join(id.as_str().replace('/', "%2F"))
    }

    pub fn log(&self, extension: &ExtensionId, message: &str) {
        eprintln!("[{extension}] {message}");
    }

    pub fn schedule_at(&self, task_id: impl Into<String>, deadline: Instant) {
        if let Ok(mut scheduled) = self.scheduled.lock() {
            scheduled.push((self.task_key(task_id.into()), deadline));
        }
    }

    pub fn cancel_task(&self, task_id: impl Into<String>) {
        if let Ok(mut canceled) = self.canceled.lock() {
            canceled.push(self.task_key(task_id.into()));
        }
    }

    pub fn drain_tasks(&self) -> (Vec<(String, Instant)>, Vec<String>) {
        let scheduled = self
            .scheduled
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default();
        let canceled = self
            .canceled
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default();
        (scheduled, canceled)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExtensionLifecycleRecord {
    pub version: String,
    pub enabled: bool,
    pub initialized: bool,
    pub last_result: String,
    pub last_failure: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct LifecycleManager {
    path: Option<PathBuf>,
    records: HashMap<ExtensionId, ExtensionLifecycleRecord>,
}

impl LifecycleManager {
    pub fn load(path: Option<PathBuf>) -> Self {
        let records = path
            .as_deref()
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|contents| parse_records(&contents))
            .unwrap_or_default();
        Self { path, records }
    }

    pub fn record(&self, id: &ExtensionId) -> Option<&ExtensionLifecycleRecord> {
        self.records.get(id)
    }

    pub fn set(&mut self, id: ExtensionId, record: ExtensionLifecycleRecord) -> io::Result<()> {
        self.records.insert(id, record);
        self.save()
    }

    pub fn remove(&mut self, id: &ExtensionId) -> io::Result<()> {
        self.records.remove(id);
        self.save()
    }

    fn save(&self) -> io::Result<()> {
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut rows: Vec<_> = self.records.iter().collect();
        rows.sort_by_key(|(id, _)| id.as_str());
        let contents = rows
            .into_iter()
            .map(|(id, record)| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    id,
                    record.version,
                    record.enabled,
                    record.initialized,
                    record.last_result.replace(['\t', '\n'], " "),
                    record
                        .last_failure
                        .as_deref()
                        .unwrap_or("")
                        .replace(['\t', '\n'], " "),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, contents)?;
        fs::rename(temporary, path)
    }
}

fn parse_records(contents: &str) -> HashMap<ExtensionId, ExtensionLifecycleRecord> {
    contents
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(6, '\t');
            let id = ExtensionId::new(fields.next()?).ok()?;
            let version = fields.next()?.to_owned();
            let enabled = fields.next()?.parse().ok()?;
            let initialized = fields.next()?.parse().ok()?;
            let last_result = fields.next()?.to_owned();
            let failure = fields.next().unwrap_or_default();
            Some((
                id,
                ExtensionLifecycleRecord {
                    version,
                    enabled,
                    initialized,
                    last_result,
                    last_failure: (!failure.is_empty()).then(|| failure.to_owned()),
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ExtensionContext, ExtensionId, ExtensionLifecycleRecord, LifecycleManager};
    use std::time::Instant;

    #[test]
    fn lifecycle_records_and_extension_configuration_persist() {
        let root = std::env::temp_dir().join(format!("peninsula-lifecycle-{}", std::process::id()));
        let id = ExtensionId::builtin("builtin.test/lifecycle");
        let mut manager = LifecycleManager::load(Some(root.join("state.tsv")));
        manager
            .set(
                id.clone(),
                ExtensionLifecycleRecord {
                    version: "1.2.3".into(),
                    enabled: true,
                    initialized: true,
                    last_result: "on_start:ok".into(),
                    last_failure: None,
                },
            )
            .unwrap();
        let loaded = LifecycleManager::load(Some(root.join("state.tsv")));
        assert_eq!(loaded.record(&id).unwrap().version, "1.2.3");
        assert!(loaded.record(&id).unwrap().initialized);

        let context = ExtensionContext::new(root.clone(), root.join("config"));
        context.schedule_at("test", Instant::now());
        context.cancel_task("old");
        let (scheduled, canceled) = context.drain_tasks();
        assert_eq!(scheduled[0].0, "test");
        assert_eq!(canceled, vec!["old".to_owned()]);
        let _ = std::fs::remove_dir_all(root);
    }
}
