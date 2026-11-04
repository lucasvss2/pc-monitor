use serde::{Deserialize, Serialize};

/// A panel area dedicated to a data source.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum Module {
    #[default]
    #[serde(alias = "ia")]
    Ai,
    #[serde(alias = "agentes")]
    Agents,
    Github,
    Trello,
    #[serde(alias = "rede")]
    Network,
    #[serde(alias = "sistema")]
    System,
}

impl Module {
    /// Stable identifier shared by the UI and event names.
    pub fn id(self) -> &'static str {
        match self {
            Module::Ai => "ai",
            Module::Agents => "agents",
            Module::Github => "github",
            Module::Trello => "trello",
            Module::Network => "network",
            Module::System => "system",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_trello_anchor_serializes_with_the_stable_tasks_identity() {
        let module: Module = serde_json::from_str("\"trello\"").unwrap();
        assert_eq!(module.id(), "tasks");
        assert_eq!(serde_json::to_value(module).unwrap(), "tasks");
    }
}
