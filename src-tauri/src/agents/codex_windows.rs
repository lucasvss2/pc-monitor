use super::{Session, Source, Thread};
use crate::ports::{Files, LiveRollouts};
use std::{
    collections::{HashMap, HashSet},
    io::ErrorKind,
    path::PathBuf,
};

const PAGE: usize = 12;
const TRACKED_LIMIT: usize = 40;

#[derive(Clone)]
struct Tracked {
    thread: Thread,
    session: Session,
    missed: bool,
}

pub(in crate::agents) struct Adapter<F, L> {
    root: PathBuf,
    files: F,
    rollouts: L,
    offset: usize,
    confirmed: HashMap<String, Tracked>,
}

impl<F: Files, L: LiveRollouts> Adapter<F, L> {
    pub fn new(root: PathBuf, files: F, rollouts: L) -> Self {
        Self {
            root,
            files,
            rollouts,
            offset: PAGE,
            confirmed: HashMap::new(),
        }
    }

    fn page(&self, offset: usize) -> Result<Vec<Thread>, ()> {
        // Limit both projections to CLI records. A desktop/IDE record is not evidence of a CLI session.
        let query = format!("SELECT id, rollout_path, cwd, title, created_at, updated_at, git_branch FROM threads WHERE source IN ('cli', 'exec') AND archived = 0 ORDER BY updated_at DESC, id DESC LIMIT {PAGE} OFFSET {offset}");
        self.files
            .read_sqlite(&self.root.join("state_5.sqlite").to_string_lossy(), &query)
            .map_err(|_| ())?
            .into_iter()
            .map(|row| serde_json::from_value(row).map_err(|_| ()))
            .collect()
    }
}

impl<F: Files, L: LiveRollouts> Source for Adapter<F, L> {
    fn sessions(&mut self, tracked: &[String]) -> Result<Vec<(Session, bool)>, ()> {
        let database_exists = match self.files.list(&self.root.to_string_lossy()) {
            Ok(names) => names.iter().any(|name| name == "state_5.sqlite"),
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(_) => return Err(()),
        };
        if !database_exists {
            return if self.confirmed.is_empty() {
                Ok(vec![])
            } else {
                Err(())
            };
        }

        let mut candidates = self.page(0)?;
        let mut offset = PAGE;
        if candidates.len() == PAGE {
            let older = self.page(self.offset)?;
            if older.len() == PAGE {
                offset = self.offset.saturating_add(PAGE);
            }
            candidates.extend(older);
        }
        // Previously observed sessions are checked on every pass, even when older than both pages.
        let mut next = self.confirmed.clone();
        next.retain(|id, _| tracked.contains(id));
        for known in next.values() {
            if !candidates
                .iter()
                .any(|candidate| candidate.id == known.thread.id)
            {
                candidates.push(known.thread.clone());
            }
        }
        let mut ids = HashSet::new();
        candidates.retain(|thread| ids.insert(thread.id.clone()));
        let paths: Vec<_> = candidates
            .iter()
            .map(|thread| PathBuf::from(&thread.rollout_path))
            .collect();
        let live = self.rollouts.live(&self.root.join("sessions"), &paths)?;
        let mut observed = Vec::new();
        for thread in candidates {
            let id = thread.session_id();
            if let Some(path) = live.get(&PathBuf::from(&thread.rollout_path)) {
                if next.len() >= TRACKED_LIMIT && !next.contains_key(&id) {
                    continue;
                }
                let canonical = path.to_str().ok_or(())?;
                let session = thread.session(&self.files, canonical)?;
                observed.push((session.clone(), true));
                next.insert(
                    id,
                    Tracked {
                        thread,
                        session,
                        missed: false,
                    },
                );
            } else if let Some(known) = next.get_mut(&id) {
                if known.missed {
                    observed.push((known.session.clone(), false));
                    next.remove(&id);
                } else {
                    // A single miss can occur while the CLI is switching or reopening a rollout.
                    known.missed = true;
                }
            }
        }
        self.confirmed = next;
        self.offset = offset;
        Ok(observed)
    }

    fn partial(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::RootFilesystem;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Owners(Arc<Mutex<Option<PathBuf>>>);

    impl LiveRollouts for Owners {
        fn live(
            &self,
            _: &std::path::Path,
            paths: &[PathBuf],
        ) -> Result<HashMap<PathBuf, PathBuf>, ()> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|path| paths.contains(path))
                .map(|path| (path.clone(), path.clone()))
                .collect())
        }
    }

    #[test]
    fn opens_only_live_cli_records_and_confirms_their_departure() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let db = rusqlite::Connection::open(root.join("state_5.sqlite")).unwrap();
        db.execute_batch("CREATE TABLE threads (id TEXT, rollout_path TEXT, cwd TEXT, title TEXT, created_at INTEGER, updated_at INTEGER, git_branch TEXT, source TEXT, archived INTEGER);").unwrap();
        let current = root.join("rollout-live.jsonl");
        std::fs::write(
            &current,
            include_str!("../../fixtures/agents/codex/task_complete.jsonl"),
        )
        .unwrap();
        for (id, source, path) in [
            ("live", "cli", current.clone()),
            ("history", "exec", root.join("rollout-closed.jsonl")),
            ("desktop", "vscode", current.clone()),
        ] {
            db.execute("INSERT INTO threads VALUES (?1, ?2, 'fixture-project', 'Fixture session', 1, 2, NULL, ?3, 0)", rusqlite::params![id, path.to_str().unwrap(), source]).unwrap();
        }
        drop(db);
        let owners = Owners::default();
        let mut source = Adapter::new(root.to_owned(), RootFilesystem::system(), owners.clone());
        assert!(source.sessions(&[]).unwrap().is_empty());
        *owners.0.lock().unwrap() = Some(current);
        let live = source.sessions(&[]).unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].0.id, "codex:live");
        assert!(live[0].1);
        let tracked = ["codex:live".to_owned()];
        *owners.0.lock().unwrap() = None;
        assert!(source.sessions(&tracked).unwrap().is_empty());
        let closed = source.sessions(&tracked).unwrap();
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].0.id, "codex:live");
        assert!(!closed[0].1);
        assert!(source.sessions(&[]).unwrap().is_empty());
    }
}
