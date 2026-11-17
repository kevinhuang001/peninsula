use std::{collections::HashSet, fs, io, path::Path};

#[derive(Default)]
pub struct SyncState {
    pub account: String,
    pub seen: HashSet<String>,
    pub initialized: bool,
}

impl SyncState {
    pub fn load(path: &Path) -> Self {
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };
        let mut state = Self::default();
        for line in contents.lines() {
            if let Some(account) = line.strip_prefix("account=") {
                state.account = account.to_owned();
            } else if line == "initialized=true" {
                state.initialized = true;
            } else if let Some(id) = line.strip_prefix("event=") {
                state.seen.insert(id.to_owned());
            }
        }
        state
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut lines = vec![
            format!("account={}", self.account),
            format!("initialized={}", self.initialized),
        ];
        let mut events: Vec<_> = self.seen.iter().collect();
        events.sort();
        lines.extend(events.into_iter().map(|id| format!("event={id}")));
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, format!("{}\n", lines.join("\n")))?;
        fs::rename(temporary, path)
    }
}
