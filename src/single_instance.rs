//! A per-user OS file lock, held for the entire application lifetime.
//! Keep the file in place: deleting it would allow a second inode to be locked.
use std::{
    env,
    fs::{self, File, OpenOptions, TryLockError},
    io,
    path::{Path, PathBuf},
};

pub(crate) struct InstanceGuard {
    _file: File,
}

impl InstanceGuard {
    pub(crate) fn acquire() -> io::Result<Option<Self>> {
        Self::acquire_at(&instance_path()?)
    }

    fn acquire_at(path: &Path) -> io::Result<Option<Self>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
}

fn instance_path() -> io::Result<PathBuf> {
    let root = if cfg!(target_os = "windows") {
        env::var_os("LOCALAPPDATA")
            .or_else(|| env::var_os("APPDATA"))
            .map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
    }
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "user state directory is unavailable",
        )
    })?;
    Ok(root.join("island-lab").join("instance.lock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_is_exclusive_and_can_restart_after_guard_drop() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("app/instance.lock");
        let first = InstanceGuard::acquire_at(&path).unwrap().unwrap();
        assert!(InstanceGuard::acquire_at(&path).unwrap().is_none());
        drop(first);
        assert!(InstanceGuard::acquire_at(&path).unwrap().is_some());
    }

    #[test]
    fn lock_child_probe() {
        let Some(path) = env::var_os("PENINSULA_TEST_INSTANCE_LOCK") else {
            return;
        };
        let guard = InstanceGuard::acquire_at(Path::new(&path)).unwrap();
        // process::exit intentionally skips Rust destructors. The OS must still
        // release the lock, including after an unexpected application exit.
        std::process::exit(if guard.is_some() { 0 } else { 23 });
    }

    #[test]
    fn separate_processes_cannot_overlap_and_exit_does_not_leave_a_stale_lock() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("instance.lock");
        let first = InstanceGuard::acquire_at(&path).unwrap().unwrap();
        let probe = || {
            crate::core::external::process::command(env::current_exe().unwrap())
                .args(["--exact", "single_instance::tests::lock_child_probe"])
                .env("PENINSULA_TEST_INSTANCE_LOCK", &path)
                .output()
                .unwrap()
                .status
                .code()
        };
        assert_eq!(probe(), Some(23));
        drop(first);
        assert_eq!(probe(), Some(0));
        assert_eq!(probe(), Some(0));
    }

    #[test]
    fn lock_io_errors_are_not_mistaken_for_another_running_instance() {
        let root = tempfile::tempdir().unwrap();
        assert!(InstanceGuard::acquire_at(root.path()).is_err());
    }
}
