//! Agent notification policy; desktop transport is injected at the core seam.
use super::{Provider, Session, SessionState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub waiting: bool,
    pub done: bool,
    pub failed: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            waiting: true,
            done: true,
            failed: true,
        }
    }
}
pub trait Sink: Send {
    fn available(&self) -> bool {
        true
    }
    fn show(&mut self, replaces: u32, title: &str, body: &str, session: &str) -> u32;
    fn close(&mut self, id: u32);
}
struct Silent;
impl Sink for Silent {
    fn available(&self) -> bool {
        false
    }
    fn show(&mut self, _: u32, _: &str, _: &str, _: &str) -> u32 {
        0
    }
    fn close(&mut self, _: u32) {}
}
pub(super) struct Policy {
    sink: Box<dyn Sink>,
    live: HashMap<String, u32>,
    turns: HashMap<String, u64>,
    recent: HashMap<String, (u64, SessionState)>,
    group: Option<(u32, Vec<String>)>,
    pub preferences: Preferences,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            sink: Box::new(Silent),
            live: HashMap::new(),
            turns: HashMap::new(),
            recent: HashMap::new(),
            group: None,
            preferences: Preferences::default(),
        }
    }
}
impl Policy {
    pub fn available(&self) -> bool {
        self.sink.available()
    }
    pub fn new(sink: impl Sink + 'static) -> Self {
        Self {
            sink: Box::new(sink),
            ..Self::default()
        }
    }
    pub fn observe(&mut self, previous: &[Session], sessions: &[Session], now: u64) {
        let removed: Vec<_> = self
            .live
            .keys()
            .filter(|id| !sessions.iter().any(|s| &s.id == *id))
            .cloned()
            .collect();
        for session in removed {
            if let Some(id) = self.live.remove(&session) {
                self.sink.close(id);
            }
        }
        if self
            .group
            .as_ref()
            .is_some_and(|(_, members)| !sessions.iter().any(|s| members.contains(&s.id)))
        {
            if let Some((id, _)) = self.group.take() {
                self.sink.close(id);
            }
        }
        self.turns
            .retain(|id, _| sessions.iter().any(|s| &s.id == id));
        self.recent
            .retain(|_, (at, _)| now.saturating_sub(*at) <= 10_000);
        for session in sessions {
            let old = previous.iter().find(|p| p.id == session.id);
            if session.state == SessionState::Working
                && old.is_none_or(|p| matches!(p.state, SessionState::Done | SessionState::Failed))
            {
                self.turns.insert(session.id.clone(), now);
            }
            // Existing terminal states establish a baseline on startup; only new outcomes notify.
            if old.is_none() && matches!(session.state, SessionState::Done | SessionState::Failed) {
                continue;
            }
            if old.is_some_and(|p| p.state == session.state) {
                continue;
            }
            if !match session.state {
                SessionState::Waiting => self.preferences.waiting,
                SessionState::Done => self.preferences.done,
                SessionState::Failed => self.preferences.failed,
                _ => false,
            } {
                continue;
            }
            let text = &crate::strings::STRINGS.agent_notifications;
            let body = match session.state {
                SessionState::Waiting => {
                    // Free-form waiting details can contain user content. Only known tool names are allowed.
                    match session.waiting_for.as_deref() {
                        Some(
                            tool @ ("Bash" | "Read" | "Write" | "Edit" | "Glob" | "Grep"
                            | "WebFetch" | "WebSearch" | "Agent"),
                        ) => text.approval.replace("{tool}", tool),
                        _ => text.waiting.clone(),
                    }
                }
                SessionState::Failed => text.failed.clone(),
                SessionState::Done if !session.interrupted => {
                    let duration = session.turn_duration.or_else(|| {
                        self.turns
                            .get(&session.id)
                            .map(|start| now.saturating_sub(*start))
                    });
                    let Some(duration) = duration.filter(|d| *d >= 30_000) else {
                        continue;
                    };
                    let seconds = duration / 1000;
                    let time = if seconds < 60 {
                        format!("{seconds} s")
                    } else {
                        format!("{} min {} s", seconds / 60, seconds % 60)
                    };
                    text.done.replace("{time}", &time)
                }
                _ => continue,
            };
            let provider = match session.provider {
                Provider::Claude => "Claude Code",
                Provider::Codex => "Codex",
            };
            let title = provider.to_string();
            self.recent.insert(session.id.clone(), (now, session.state));
            if self.recent.len() >= 3 {
                for id in self.recent.keys() {
                    if let Some(notification) = self.live.remove(id) {
                        self.sink.close(notification);
                    }
                }
                let replaces = self.group.as_ref().map_or(0, |(id, _)| *id);
                let template = if self
                    .recent
                    .values()
                    .all(|(_, state)| *state == SessionState::Waiting)
                {
                    &text.group_waiting
                } else {
                    &text.group
                };
                let body = template.replace("{count}", &self.recent.len().to_string());
                let id = self.sink.show(replaces, &text.title, &body, &session.id);
                let mut members = self
                    .group
                    .take()
                    .map_or_else(Vec::new, |(_, members)| members);
                for id in self.recent.keys() {
                    if !members.contains(id) {
                        members.push(id.clone());
                    }
                }
                self.group = Some((id, members));
                continue;
            }
            if self
                .group
                .as_ref()
                .is_some_and(|(_, members)| members.contains(&session.id))
            {
                if let Some((id, _)) = self.group.take() {
                    self.sink.close(id);
                }
            }
            let replaces = self.live.get(&session.id).copied().unwrap_or(0);
            let id = self.sink.show(replaces, &title, &body, &session.id);
            self.live.insert(session.id.clone(), id);
        }
    }
}
