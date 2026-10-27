use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const DEFAULT_SECTIONS_WIDTH: u16 = 22;

#[derive(Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub favorites: Vec<String>,
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default = "default_sections_width")]
    pub sections_width: u16,
}

fn default_sections_width() -> u16 {
    DEFAULT_SECTIONS_WIDTH
}

impl Default for State {
    fn default() -> Self {
        Self {
            favorites: Vec::new(),
            recent: Vec::new(),
            sections_width: DEFAULT_SECTIONS_WIDTH,
        }
    }
}

impl State {
    pub fn is_favorite(&self, name: &str) -> bool {
        self.favorites.iter().any(|f| f == name)
    }

    pub fn toggle_favorite(&mut self, name: &str) -> bool {
        if let Some(pos) = self.favorites.iter().position(|f| f == name) {
            self.favorites.remove(pos);
            false
        } else {
            self.favorites.push(name.to_string());
            true
        }
    }

    pub fn record_recent(&mut self, name: &str) {
        self.recent.retain(|r| r != name);
        self.recent.insert(0, name.to_string());
        self.recent.truncate(10);
    }

    pub fn clamp_width(&mut self) {
        self.sections_width = self.sections_width.clamp(14, 40);
    }
}

pub fn state_dir() -> Option<PathBuf> {
    let base = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            env::var("HOME")
                .ok()
                .filter(|h| !h.is_empty())
                .map(|h| PathBuf::from(h).join(".local/state"))
        })?;
    Some(base.join("bashbox"))
}

fn state_path() -> Option<PathBuf> {
    Some(state_dir()?.join("state.toml"))
}

pub fn load() -> State {
    let Some(path) = state_path() else {
        return State::default();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return State::default();
    };
    let mut state: State = toml::from_str(&text).unwrap_or_default();
    state.clamp_width();
    state
}

pub fn save(state: &State) {
    let Some(path) = state_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(text) = toml::to_string(state) {
        let _ = fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_favorites() {
        let mut state = State::default();
        assert!(!state.is_favorite("Disk usage"));
        assert!(state.toggle_favorite("Disk usage"));
        assert!(state.is_favorite("Disk usage"));
        assert!(!state.toggle_favorite("Disk usage"));
        assert!(!state.is_favorite("Disk usage"));
    }

    #[test]
    fn recent_is_deduplicated_and_capped() {
        let mut state = State::default();
        for i in 0..12 {
            state.record_recent(&format!("task {}", i));
        }
        state.record_recent("task 5");
        assert_eq!(state.recent.len(), 10);
        assert_eq!(state.recent.first().map(String::as_str), Some("task 5"));
        assert_eq!(
            state.recent.iter().filter(|name| *name == "task 5").count(),
            1
        );
    }

    #[test]
    fn width_is_clamped() {
        let mut state = State {
            sections_width: 1,
            ..State::default()
        };
        state.clamp_width();
        assert_eq!(state.sections_width, 14);
        state.sections_width = 999;
        state.clamp_width();
        assert_eq!(state.sections_width, 40);
    }
}
