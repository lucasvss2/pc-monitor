//! Core to UI: publish one snapshot per module on snapshot:<module> whenever it changes.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::module::Module;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub module: Module,
    /// Last update in epoch milliseconds; None until data exists.
    pub updated_at: Option<u64>,
    pub data: serde_json::Value,
}

/// Latest snapshot of each module, delivered when the UI loads.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Publication {
    #[serde(flatten)]
    pub snapshot: Snapshot,
    pub runtime_id: String,
    pub revision: u64,
    pub context_generation: u64,
}

pub struct Snapshots {
    runtime_id: String,
    latest: Mutex<HashMap<Module, Publication>>,
}
impl Default for Snapshots {
    fn default() -> Self {
        use crate::ports::Clock;
        Self {
            runtime_id: format!("{}-{}", crate::ports::SystemClock.now(), std::process::id()),
            latest: Mutex::default(),
        }
    }
}

impl Snapshots {
    fn store(&self, snapshot: Snapshot) -> Publication {
        let mut latest = self.latest.lock().unwrap();
        let revision = latest.get(&snapshot.module).map_or(1, |p| p.revision + 1);
        let publication = Publication {
            context_generation: snapshot.data["contextGeneration"].as_u64().unwrap_or(0),
            snapshot,
            runtime_id: self.runtime_id.clone(),
            revision,
        };
        latest.insert(publication.snapshot.module, publication.clone());
        publication
    }

    pub fn runtime_id(&self) -> &str {
        &self.runtime_id
    }

    pub fn all(&self) -> Vec<Publication> {
        self.latest.lock().unwrap().values().cloned().collect()
    }
}

/// Store the snapshot and push it to the UI.
pub fn publish(app: &AppHandle, snapshot: Snapshot) {
    let event = format!("snapshot:{}", snapshot.module.id());
    let publication = app.state::<Snapshots>().store(snapshot);
    let _ = app.emit(&event, publication);
}
