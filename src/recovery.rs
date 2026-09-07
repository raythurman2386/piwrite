use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use fs4::fs_std::FileExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoverySnapshot {
    #[serde(rename = "fileUrl")]
    pub file_url: Option<String>,
    pub text: String,
}

pub struct RecoverySlot {
    pub json_path: PathBuf,
    _lock: File,
}

impl RecoverySlot {
    /// Claim an orphaned snapshot first, then an empty slot.
    pub fn claim(state_directory: &Path) -> io::Result<Self> {
        fs::create_dir_all(state_directory)?;
        for pass in 0..2 {
            for slot in 0..100 {
                let base = state_directory.join(format!("recovery-{slot}"));
                let json_path = base.with_extension("json");
                let lock_path = base.with_extension("lock");
                let snapshot_exists = json_path.exists();
                if (pass == 0) != snapshot_exists {
                    continue;
                }
                let lock = File::options()
                    .create(true)
                    .truncate(false)
                    .read(true)
                    .write(true)
                    .open(&lock_path)?;
                if lock.try_lock_exclusive().is_ok() {
                    return Ok(Self {
                        json_path,
                        _lock: lock,
                    });
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "no recovery slot available",
        ))
    }

    pub fn read(&self) -> Option<RecoverySnapshot> {
        let raw = fs::read(&self.json_path).ok()?;
        let snapshot: RecoverySnapshot = serde_json::from_slice(&raw).ok()?;
        if snapshot.text.is_empty() && snapshot.file_url.is_none() {
            return None;
        }
        Some(snapshot)
    }

    pub fn write(&self, snapshot: &RecoverySnapshot) -> io::Result<()> {
        if let Some(parent) = self.json_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec(snapshot)?;
        let tmp = self.json_path.with_extension("json.tmp");
        {
            let mut file = File::create(&tmp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        fs::rename(tmp, &self.json_path)?;
        Ok(())
    }

    pub fn clear(&self) {
        let _ = fs::remove_file(&self.json_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_snapshot() {
        let dir = std::env::temp_dir().join(format!("piwrite-recovery-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let slot = RecoverySlot::claim(&dir).unwrap();
        let snapshot = RecoverySnapshot {
            file_url: Some("file:///tmp/a.md".into()),
            text: "hello".into(),
        };
        slot.write(&snapshot).unwrap();
        assert_eq!(slot.read(), Some(snapshot));
        slot.clear();
        assert_eq!(slot.read(), None);
        let _ = fs::remove_dir_all(&dir);
    }
}
