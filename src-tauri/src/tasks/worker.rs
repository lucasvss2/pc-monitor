//! A single mailbox serializes reads and writes. Context changes wait for the
//! bounded HTTP operation; aborting a local wait never replays a remote write.
use super::*;
use std::sync::mpsc;
use tauri::Manager;

type Reply<T> = tokio::sync::oneshot::Sender<Result<T, Error>>;
pub enum Command {
    Refresh(u64),
    Switch(u64, Provider, Reply<()>),
    Connect(u64, Connection, Reply<String>),
    SelectBoard(u64, String, Reply<()>),
    Disconnect(u64, Reply<()>),
    RetryCleanup(u64, Reply<()>),
    Transitions(u64, String, Reply<Vec<Transition>>),
    Perform(u64, Action, Reply<()>),
    Open(u64, String),
    Authorize(u64, String, Reply<()>),
}
pub struct Collector(pub mpsc::Sender<Command>);

pub(super) fn dispatch(
    tasks: &mut Tasks,
    command: Command,
    progress: &mut dyn FnMut(Snapshot),
    processes: &dyn crate::ports::Processes,
) -> bool {
    macro_rules! answer {
        ($reply:expr, $result:expr) => {{
            let result = $result;
            progress(tasks.snapshot());
            let _ = $reply.send(result);
        }};
    }
    match command {
        Command::Refresh(expected) => return tasks.check(expected).is_ok(),
        Command::Switch(generation, provider, reply) => {
            answer!(reply, tasks.switch(generation, provider, progress))
        }
        Command::Connect(generation, connection, reply) => {
            answer!(reply, tasks.connect(generation, connection, progress))
        }
        Command::SelectBoard(generation, board, reply) => {
            answer!(reply, tasks.select_board(generation, &board, progress))
        }
        Command::Disconnect(generation, reply) => {
            answer!(reply, tasks.disconnect(generation, progress))
        }
        Command::RetryCleanup(generation, reply) => answer!(reply, tasks.retry_cleanup(generation)),
        Command::Transitions(generation, item, reply) => {
            answer!(reply, tasks.transitions(generation, &item))
        }
        Command::Perform(generation, action, reply) => {
            answer!(reply, tasks.perform(generation, action, progress))
        }
        Command::Authorize(generation, key, reply) => {
            answer!(reply, tasks.authorize(generation, &key, processes))
        }
        Command::Open(generation, url) => {
            let _ = tasks.open(generation, &url, processes);
        }
    }
    false
}

pub fn start(app: tauri::AppHandle) {
    let (tx, rx) = mpsc::channel();
    app.manage(Collector(tx));
    std::thread::spawn(move || {
        let Some(config) = dirs::config_dir() else {
            return;
        };
        let shared = app.state::<crate::lifecycle::Shared>().inner().clone();
        let trello =
            crate::trello::Trello::new(crate::trello::TrelloHttp, crate::ports::SystemClock)
                .with_keyring(crate::ports::NativeKeyring)
                .with_persistence(crate::persistence::Volatile)
                .with_lifecycle(shared.clone());
        let jira = jira::Jira::new(
            jira::JiraHttp,
            crate::ports::SystemClock,
            crate::ports::NativeKeyring,
        )
        .with_lifecycle(shared.clone());
        let cache = dirs::cache_dir()
            .map(|p| p.join("kvojps-monitor"))
            .unwrap_or_else(|| config.join("kvojps-monitor/cache"));
        let logs = dirs::state_dir()
            .map(|p| p.join("kvojps-monitor"))
            .unwrap_or_else(|| config.join("kvojps-monitor/logs"));
        let persistence = crate::persistence::LocalPersistence {
            cache: crate::persistence::Cache(crate::persistence::DiskStorage(cache)),
            logs: crate::persistence::Logs(crate::persistence::DiskStorage(logs)),
        };
        let mut tasks = Tasks::new(
            trello,
            jira,
            persistence,
            crate::persistence::DiskStorage(config.join("kvojps-monitor")),
        );
        let mut publish = |snapshot| crate::snapshot::publish(&app, snapshot);
        publish(tasks.snapshot());
        let mut previous_status = None;
        loop {
            let command = rx.recv_timeout(std::time::Duration::from_secs(1)).ok();
            let status = *shared.lock().unwrap();
            tasks.lifecycle(status);
            let manual = command.is_some_and(|command| {
                dispatch(
                    &mut tasks,
                    command,
                    &mut publish,
                    &crate::ports::SystemProcesses,
                )
            });
            let identity = (status.online, status.generation, status.ready_at);
            if previous_status != Some(identity) {
                previous_status = Some(identity);
                publish(tasks.snapshot());
            }
            let visible = crate::collection::is_visible(&app, Module::Tasks);
            tasks.poll(visible, manual, &mut publish);
        }
    });
}
