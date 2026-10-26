//! The route table (`prompts/routes.toml`): per task, the engines that passed
//! its golden set, best first. Written from bake-off results.

use std::collections::BTreeMap;

use kabar_protocol::ai::{EngineId, Task};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteStatus {
    /// Every listed engine passed the task's golden set in the bake-off.
    Vetted,
    /// Best guess before the bake-off ran; answers are badged.
    Provisional,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub engines: Vec<EngineId>,
    pub status: RouteStatus,
    /// Prompt version to use; defaults to the newest bundled one.
    #[serde(default)]
    pub prompt_version: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RouteTable {
    routes: BTreeMap<Task, Route>,
}

impl RouteTable {
    pub fn parse(toml_text: &str) -> Result<Self, String> {
        let raw: BTreeMap<String, Route> = toml::from_str(toml_text).map_err(|e| e.to_string())?;
        let mut routes = BTreeMap::new();
        for (name, route) in raw {
            let task = Task::parse(&name).ok_or_else(|| format!("unknown task `{name}`"))?;
            // Defence in depth: the router also filters at call time.
            if task.is_personal()
                && let Some(e) = route.engines.iter().find(|e| e.trains_on_inputs())
            {
                return Err(format!("`{name}` is personal and must not route to {}", e.as_str()));
            }
            routes.insert(task, route);
        }
        if let Some(missing) = Task::ALL.iter().find(|t| !routes.contains_key(t)) {
            return Err(format!("no route for `{}`", missing.as_str()));
        }
        Ok(Self { routes })
    }

    /// The table shipped with the app.
    pub fn builtin() -> Self {
        Self::parse(include_str!("../../../../prompts/routes.toml")).expect("prompts/routes.toml is valid")
    }

    pub fn get(&self, task: Task) -> &Route {
        &self.routes[&task]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_covers_every_task_and_keeps_personal_off_gemini() {
        let t = RouteTable::builtin();
        for task in Task::ALL {
            let r = t.get(task);
            assert!(!r.engines.is_empty(), "{task:?}");
            if task.is_personal() {
                assert!(!r.engines.contains(&EngineId::GeminiFree), "{task:?}");
            }
        }
    }

    #[test]
    fn rejects_personal_task_on_gemini() {
        let mut text = include_str!("../../../../prompts/routes.toml").to_string();
        text.push_str("\n[__probe]\nengines = []\nstatus = \"vetted\"\n");
        assert!(RouteTable::parse(&text).unwrap_err().contains("unknown task"));

        let bad = include_str!("../../../../prompts/routes.toml")
            .replace("[coach_message]\nengines = [", "[coach_message]\nengines = [\"gemini_free\", ");
        assert!(RouteTable::parse(&bad).unwrap_err().contains("personal"));
    }
}
