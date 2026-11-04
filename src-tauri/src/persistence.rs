//! Sanitized snapshot cache and bounded metadata logs, through an injectable disk port.
use crate::{module::Module, snapshot::Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io, path::PathBuf};

pub trait Storage: Send {
    fn read(&self, path: &str) -> io::Result<String>;
    fn write(&self, path: &str, data: &str) -> io::Result<()>;
    fn rename(&self, from: &str, to: &str) -> io::Result<()>;
    fn remove(&self, path: &str) -> io::Result<()>;
}

pub struct DiskStorage(pub PathBuf);
impl Storage for DiskStorage {
    fn read(&self, path: &str) -> io::Result<String> {
        std::fs::read_to_string(self.0.join(path))
    }
    fn write(&self, path: &str, data: &str) -> io::Result<()> {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, data)
    }
    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        std::fs::rename(self.0.join(from), self.0.join(to))
    }
    fn remove(&self, path: &str) -> io::Result<()> {
        match std::fs::remove_file(self.0.join(path)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CachedGithub {
    calendar: Option<Calendar>,
    featured: Option<Featured>,
    /// Only a day count; never contribution content.
    #[serde(default)]
    streak: Option<u64>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Calendar {
    restricted_contributions_count: u64,
    contribution_calendar: ContributionCalendar,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContributionCalendar {
    total_contributions: u64,
    weeks: Vec<Week>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Week {
    contribution_days: Vec<Day>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Day {
    date: String,
    contribution_count: u64,
    color: String,
}
#[derive(Deserialize, Serialize)]
struct Featured {
    name: String,
    commits: u64,
    total: u64,
    when: u64,
}

/// Board structure without card titles, which are user content.
#[derive(Deserialize, Serialize)]
struct CachedTrello {
    board: TrelloBoard,
    lists: Vec<TrelloList>,
}
#[derive(Deserialize, Serialize)]
struct TrelloBoard {
    id: String,
    name: String,
}
#[derive(Deserialize, Serialize)]
struct TrelloList {
    id: String,
    name: String,
    cards: Vec<TrelloCard>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrelloCard {
    id: String,
    url: String,
    labels: Vec<String>,
    due: Option<u64>,
    due_complete: bool,
}

pub struct Cache<S>(pub S);
impl<S: Storage> Cache<S> {
    pub fn save(&self, snapshot: &Snapshot) -> io::Result<()> {
        let data = sanitize(snapshot.module, &snapshot.data)?;
        let body =
            json!({"module": snapshot.module, "updatedAt": snapshot.updated_at, "data": data});
        let path = format!("{}.json", snapshot.module.id());
        self.0.write(&format!("{path}.tmp"), &body.to_string())?;
        self.0.rename(&format!("{path}.tmp"), &path)
    }
    pub fn load(&self, module: Module) -> Option<Snapshot> {
        let text = self
            .0
            .read(&format!("{}.json", module.id()))
            .or_else(|error| {
                if module == Module::Tasks && error.kind() == io::ErrorKind::NotFound {
                    self.0.read("trello.json")
                } else {
                    Err(error)
                }
            })
            .ok()?;
        let body: Value = serde_json::from_str(&text).ok()?;
        let mut data = sanitize(module, &body["data"]).ok()?;
        if module == Module::Github {
            data["state"] = json!("stale");
            data["refreshing"] = json!(false);
            data["lastSuccessAt"] = body["updatedAt"].clone();
            if data["featured"].is_object() {
                data["featured"]["title"] = json!("");
            }
        }
        if module == Module::Tasks {
            data["state"] = json!("stale");
            data["refreshing"] = json!(false);
            data["account"] = Value::Null;
            data["boards"] = json!([]);
            for list in data["lists"].as_array_mut().into_iter().flatten() {
                for card in list["cards"].as_array_mut().into_iter().flatten() {
                    card["name"] = json!("");
                }
            }
        }
        Some(Snapshot {
            module,
            updated_at: body["updatedAt"].as_u64(),
            data,
        })
    }
}
fn sanitize(module: Module, data: &Value) -> io::Result<Value> {
    match module {
        Module::Github => {
            let clean: CachedGithub =
                serde_json::from_value(data.clone()).map_err(io::Error::other)?;
            serde_json::to_value(clean).map_err(io::Error::other)
        }
        Module::Tasks => {
            if data["provider"] == "jira" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Jira readings are memory-only",
                ));
            }
            let clean: CachedTrello =
                serde_json::from_value(data.clone()).map_err(io::Error::other)?;
            serde_json::to_value(clean).map_err(io::Error::other)
        }
        Module::Ai => {
            #[derive(Serialize, Deserialize)]
            struct CachedAi {
                #[serde(default, skip_serializing_if = "Option::is_none")]
                codex: Option<CachedProvider>,
                #[serde(default, skip_serializing_if = "Option::is_none")]
                claude: Option<CachedProvider>,
            }
            /// Plan and usage windows; never account e-mail, ids or tokens.
            #[derive(Serialize, Deserialize)]
            struct CachedProvider {
                plan: Option<String>,
                reading: Option<crate::ai::Reading>,
            }
            let clean: CachedAi = serde_json::from_value(data.clone()).map_err(io::Error::other)?;
            serde_json::to_value(clean).map_err(io::Error::other)
        }
        Module::Network => {
            #[derive(Serialize, Deserialize)]
            struct CachedNetwork {
                speed: Option<crate::network::SpeedResult>,
            }
            let clean: CachedNetwork =
                serde_json::from_value(data.clone()).map_err(io::Error::other)?;
            if clean.speed.as_ref().is_some_and(|s| !s.valid()) {
                return Err(io::Error::other("invalid measurement"));
            }
            serde_json::to_value(clean).map_err(io::Error::other)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "module has no disk cache",
        )),
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Event {
    Updated,
    Failed,
    InvalidCredential,
    MissingCredential,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    pub source: Module,
    pub event: Event,
    pub http_status: Option<u16>,
    pub duration_ms: u64,
}
/// Info events only. The type cannot carry free text or response content.
pub struct Logs<S>(pub S);
impl<S: Storage> Logs<S> {
    pub fn record(&self, record: LogRecord) -> io::Result<()> {
        const LIMIT: usize = 1_000_000;
        let line = serde_json::to_string(&record).map_err(io::Error::other)? + "\n";
        let mut current = match self.0.read("info.log") {
            Ok(data) => data,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error),
        };
        if current.len() + line.len() > LIMIT {
            match self.0.rename("info.1.log", "info.2.log") {
                Ok(()) => (),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(error),
            }
            self.0.rename("info.log", "info.1.log")?;
            current.clear();
        }
        current.push_str(&line);
        self.0.write("info.log", &current)
    }
}

pub trait Persistence: Send {
    fn load(&self, module: Module) -> Option<Snapshot>;
    fn save(&self, snapshot: &Snapshot);
    /// Forget the cached snapshot, e.g. after disconnecting an account.
    fn clear(&self, module: Module) -> io::Result<()>;
    fn log(&self, record: LogRecord);
}
pub struct LocalPersistence<C, L> {
    pub cache: Cache<C>,
    pub logs: Logs<L>,
}
impl<C: Storage, L: Storage> Persistence for LocalPersistence<C, L> {
    fn load(&self, module: Module) -> Option<Snapshot> {
        self.cache.load(module)
    }
    fn save(&self, snapshot: &Snapshot) {
        let _ = self.cache.save(snapshot);
    }
    fn clear(&self, module: Module) -> io::Result<()> {
        self.cache.0.remove(&format!("{}.json", module.id()))?;
        self.cache.0.remove(&format!("{}.json.tmp", module.id()))?;
        if module == Module::Tasks {
            self.cache.0.remove("trello.json")?;
            self.cache.0.remove("trello.json.tmp")?;
        }
        Ok(())
    }
    fn log(&self, record: LogRecord) {
        let _ = self.logs.record(record);
    }
}
pub struct Volatile;
impl Persistence for Volatile {
    fn load(&self, _: Module) -> Option<Snapshot> {
        None
    }
    fn save(&self, _: &Snapshot) {}
    fn clear(&self, _: Module) -> io::Result<()> {
        Ok(())
    }
    fn log(&self, _: LogRecord) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jira_readings_are_rejected_before_any_cache_file_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache(DiskStorage(dir.path().into()));
        // Even a shape compatible with the legacy cache cannot bypass the provider guard.
        let mut data: Value =
            serde_json::from_str(include_str!("../../src/lib/trello/fixtures/full.json")).unwrap();
        data["provider"] = json!("jira");
        data["issues"] = json!([{"title":"PRIVATE_ISSUE_CONTENT"}]);
        assert!(cache
            .save(&Snapshot {
                module: Module::Tasks,
                updated_at: Some(1000),
                data
            })
            .is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
    #[test]
    fn disk_cache_restores_counts_but_never_credentials_or_commit_content() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache(DiskStorage(dir.path().into()));
        let mut data: Value =
            serde_json::from_str(include_str!("../../src/lib/github/fixtures/full.json")).unwrap();
        data["token"] = json!("secret-token");
        data["prompt"] = json!("private-prompt");
        data["featured"]["title"] = json!("private-commit");
        cache
            .save(&Snapshot {
                module: Module::Github,
                updated_at: Some(1000),
                data,
            })
            .unwrap();
        let bytes = std::fs::read_to_string(dir.path().join("github.json")).unwrap();
        for forbidden in [
            "secret-token",
            "private-prompt",
            "private-commit",
            "token",
            "title",
        ] {
            assert!(!bytes.contains(forbidden));
        }
        let snapshot = cache.load(Module::Github).unwrap();
        assert_eq!(snapshot.updated_at, Some(1000));
        assert_eq!(snapshot.data["featured"]["commits"], 27);
        assert_eq!(snapshot.data["state"], "stale");
        assert_eq!(snapshot.data["featured"]["title"], "");
        assert!(cache.load(Module::System).is_none());
    }
    #[test]
    fn ai_cache_keeps_plan_and_windows_but_never_account_data() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache(DiskStorage(dir.path().into()));
        let mut data: Value =
            serde_json::from_str(include_str!("../../src/lib/ai/fixtures/full.json")).unwrap();
        data["codex"]["email"] = json!("exemplo@example.com");
        data["codex"]["accountId"] = json!("conta-exemplo");
        data["codex"]["reading"]["token"] = json!("secret-token");
        cache
            .save(&Snapshot {
                module: Module::Ai,
                updated_at: Some(1000),
                data,
            })
            .unwrap();
        let bytes = std::fs::read_to_string(dir.path().join("ai.json")).unwrap();
        for forbidden in [
            "exemplo@example.com",
            "conta-exemplo",
            "secret-token",
            "email",
            "token",
        ] {
            assert!(!bytes.contains(forbidden), "{forbidden}");
        }
        let snapshot = cache.load(Module::Ai).unwrap();
        assert_eq!(snapshot.data["codex"]["plan"], "plus");
        assert_eq!(
            snapshot.data["codex"]["reading"]["buckets"][0]["secondary"]["usedPercent"],
            48.0
        );
    }
    #[test]
    fn info_logs_have_only_metadata_and_rotate_with_three_one_megabyte_files() {
        let dir = tempfile::tempdir().unwrap();
        let disk = DiskStorage(dir.path().into());
        // Existing full files exercise rotation without a slow million-event test.
        disk.write("info.log", &" ".repeat(999_999)).unwrap();
        disk.write("info.1.log", "older").unwrap();
        disk.write("info.2.log", "oldest").unwrap();
        Logs(disk)
            .record(LogRecord {
                source: Module::Github,
                event: Event::Failed,
                http_status: Some(503),
                duration_ms: 123,
            })
            .unwrap();
        let line = std::fs::read_to_string(dir.path().join("info.log")).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&line).unwrap(),
            json!({"source":"github","event":"failed","httpStatus":503,"durationMs":123})
        );
        assert_eq!(
            std::fs::metadata(dir.path().join("info.1.log"))
                .unwrap()
                .len(),
            999_999
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("info.2.log")).unwrap(),
            "older"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 3);
    }
}
