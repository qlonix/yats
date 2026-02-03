use rdev::Key;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScrollConfig {
    pub sensitivity: u32,
    #[serde(default)]
    pub invert: bool,
    #[serde(default)]
    pub acceleration: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum WindowAction {
    Close,
    Minimize,
    Maximize,
    Move,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum Action {
    MouseClick(MouseButton),
    MouseDoubleClick(MouseButton),
    MouseScroll(ScrollConfig),
    KeyMacro(Vec<Key>),
    Window(WindowAction),
    BrowserBack,
    BrowserForward,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub mappings: HashMap<Key, Action>,
    pub release_delay_ms: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut mappings = HashMap::new();
        // Default mapping: F12 -> Scroll (Standard sensitivity)
        mappings.insert(
            Key::F12,
            Action::MouseScroll(ScrollConfig {
                sensitivity: 100,
                invert: false,
                acceleration: false,
            }),
        );

        Self {
            mappings,
            release_delay_ms: 200,
        }
    }
}

impl AppConfig {
    pub fn load(app: AppHandle) -> Self {
        let path = Self::get_config_path(app);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    return config;
                }
            }
        }
        AppConfig::default()
    }

    pub fn save(&self, app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_config_path(app.clone());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    fn get_config_path(app: AppHandle) -> PathBuf {
        let mut path = app
            .path()
            .app_config_dir()
            .expect("Failed to get config dir");
        path.push("config.json");
        path
    }
}
