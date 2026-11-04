//! Provider semantics behind the shared Tasks coordinator. I/O stays in the existing ports.
use crate::{lifecycle, ports::Processes, snapshot::Snapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    Trello,
    Jira,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Error {
    InvalidCredential,
    InvalidSite,
    Offline,
    KeyringUnavailable,
    CleanupFailed,
    SettingsUnavailable,
    StaleContext,
    ReadRequired,
    Unconfirmed,
    RequiredFields,
    Refused,
    UnsupportedBoard,
}

#[derive(Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum Connection {
    Trello {
        key: String,
        token: String,
    },
    Jira {
        site: String,
        email: String,
        token: String,
    },
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    Move { item: String, list: String },
    Transition { item: String, transition: String },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    pub id: String,
    pub name: String,
    pub status: String,
    pub requires_fields: bool,
}

pub trait Source: Send {
    fn snapshot(&self, refreshing: bool) -> Snapshot;
    fn poll(
        &mut self,
        visible: bool,
        manual: bool,
        progress: &mut dyn FnMut(Snapshot),
    ) -> Option<Snapshot>;
    fn connect(&mut self, connection: Connection) -> Result<String, Error>;
    fn select_board(&mut self, id: &str) -> Result<(), Error>;
    fn disconnect(&mut self) -> Result<(), Error>;
    fn reset(&mut self);
    fn restore(&mut self, snapshot: Snapshot);
    fn transitions(&self, item: &str) -> Result<Vec<Transition>, Error>;
    fn perform(&mut self, action: Action, progress: &mut dyn FnMut(Snapshot)) -> Result<(), Error>;
    fn authorize(&mut self, key: &str, processes: &dyn Processes) -> Result<(), Error>;
    fn lifecycle(&mut self, status: lifecycle::Status);
}

fn credential(error: crate::trello::CredentialError) -> Error {
    match error {
        crate::trello::CredentialError::InvalidCredential => Error::InvalidCredential,
        crate::trello::CredentialError::Offline => Error::Offline,
        crate::trello::CredentialError::KeyringUnavailable => Error::KeyringUnavailable,
    }
}

impl<A: crate::trello::TrelloApi, R: crate::ports::Clock> Source for crate::trello::Trello<A, R> {
    fn snapshot(&self, refreshing: bool) -> Snapshot {
        self.snapshot(refreshing)
    }
    fn poll(
        &mut self,
        visible: bool,
        manual: bool,
        progress: &mut dyn FnMut(Snapshot),
    ) -> Option<Snapshot> {
        if let Some(snapshot) = self.begin(visible, manual) {
            progress(snapshot);
        }
        self.step(visible, manual)
    }
    fn connect(&mut self, connection: Connection) -> Result<String, Error> {
        let Connection::Trello { key, token } = connection else {
            return Err(Error::InvalidCredential);
        };
        self.connect(&key, &token).map_err(credential)
    }
    fn select_board(&mut self, id: &str) -> Result<(), Error> {
        self.select_board(id).map_err(credential)
    }
    fn disconnect(&mut self) -> Result<(), Error> {
        self.disconnect().map_err(credential)
    }
    fn reset(&mut self) {
        self.reset_reading();
    }
    fn restore(&mut self, snapshot: Snapshot) {
        self.restore_reading(snapshot);
    }
    fn transitions(&self, _: &str) -> Result<Vec<Transition>, Error> {
        Err(Error::Refused)
    }
    fn perform(&mut self, action: Action, progress: &mut dyn FnMut(Snapshot)) -> Result<(), Error> {
        let Action::Move { item, list } = action else {
            return Err(Error::Refused);
        };
        self.move_card(&item, &list, progress).map_err(|_| {
            if self.snapshot(false).data["movement"]["state"] == "unconfirmed" {
                Error::Unconfirmed
            } else {
                Error::Refused
            }
        })
    }
    fn authorize(&mut self, key: &str, processes: &dyn Processes) -> Result<(), Error> {
        self.authorize(processes, key).map_err(credential)
    }
    fn lifecycle(&mut self, status: lifecycle::Status) {
        self.lifecycle(status);
    }
}
