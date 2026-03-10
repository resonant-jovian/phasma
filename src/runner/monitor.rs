//! Filesystem watcher for monitoring running batch jobs (--monitor / --tail).

use std::path::{Path, PathBuf};

use tokio::sync::mpsc;

use crate::sim::SimState;

pub struct MonitorHandle {
    pub state_rx: mpsc::UnboundedReceiver<SimState>,
    pub task: tokio::task::JoinHandle<()>,
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl MonitorHandle {
    /// Watch a batch output directory for new snapshot files.
    /// Loads existing snapshots for catch-up, then watches for new files.
    pub fn spawn(dir: PathBuf) -> Self {
        let (state_tx, state_rx) = mpsc::unbounded_channel();

        let task = tokio::task::spawn_blocking(move || {
            watch_directory(&dir, &state_tx);
        });

        Self { state_rx, task }
    }
}

fn watch_directory(dir: &Path, state_tx: &mpsc::UnboundedSender<SimState>) {
    let snap_dir = dir.join("snapshots");

    // Phase 1: catch-up — load existing snapshots
    if snap_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&snap_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                e.path().extension().is_some_and(|ext| ext == "json")
                    && e.path()
                        .file_name()
                        .is_some_and(|n| n != "state_final.json")
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            if let Ok(json) = std::fs::read_to_string(entry.path())
                && let Ok(state) = serde_json::from_str::<SimState>(&json)
                && state_tx.send(state).is_err()
            {
                return;
            }
        }
    }

    // Phase 2: poll for new files
    let mut seen_count = count_snapshots(&snap_dir);
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Check if metadata.json exists with an end_time (run complete)
        let meta_path = dir.join("metadata.json");
        let run_complete = if let Ok(json) = std::fs::read_to_string(&meta_path) {
            json.contains("\"end_time\"") && !json.contains("\"end_time\": null")
        } else {
            false
        };

        let current_count = count_snapshots(&snap_dir);
        if current_count > seen_count {
            // Load new snapshots
            if let Ok(mut entries) = read_snapshot_entries(&snap_dir) {
                entries.sort_by_key(|e| e.file_name());
                for entry in entries.into_iter().skip(seen_count) {
                    if let Ok(json) = std::fs::read_to_string(entry.path())
                        && let Ok(state) = serde_json::from_str::<SimState>(&json)
                        && state_tx.send(state).is_err()
                    {
                        return;
                    }
                }
            }
            seen_count = current_count;
        }

        if run_complete {
            // Load final state
            let final_path = snap_dir.join("state_final.json");
            if final_path.exists()
                && let Ok(json) = std::fs::read_to_string(&final_path)
                && let Ok(state) = serde_json::from_str::<SimState>(&json)
            {
                let _ = state_tx.send(state);
            }
            break;
        }
    }
}

fn count_snapshots(snap_dir: &Path) -> usize {
    read_snapshot_entries(snap_dir)
        .map(|v| v.len())
        .unwrap_or(0)
}

fn read_snapshot_entries(snap_dir: &Path) -> std::io::Result<Vec<std::fs::DirEntry>> {
    let entries: Vec<_> = std::fs::read_dir(snap_dir)?
        .flatten()
        .filter(|e| {
            let p = e.path();
            p.extension().is_some_and(|ext| ext == "json")
                && p.file_name().is_some_and(|n| n != "state_final.json")
        })
        .collect();
    Ok(entries)
}
