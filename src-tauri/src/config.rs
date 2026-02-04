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
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum Action {
    MouseClick(MouseButton),
    MouseDoubleClick(MouseButton),
    MouseScroll(ScrollConfig),
    KeyMacro(Vec<Vec<Key>>),
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
        // デフォルトマッピング: F12 -> スクロール (標準感度)
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
                // 最初に直接デシリアライズを試みる
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    return config;
                }

                // 失敗した場合、KeyMacro 型の手動移行を試みる
                if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(mappings) =
                        value.get_mut("mappings").and_then(|m| m.as_object_mut())
                    {
                        for (_, action) in mappings {
                            if action.get("type")
                                == Some(&serde_json::Value::String("KeyMacro".to_string()))
                            {
                                if let Some(val) = action.get_mut("value") {
                                    if val.is_array() {
                                        let arr = val.as_array().unwrap();
                                        // 文字列のフラット配列の場合、ネストする
                                        if !arr.is_empty() && arr[0].is_string() {
                                            let migrated = serde_json::json!([arr]);
                                            *val = migrated;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // 移行したオブジェクトのデシリアライズを試みる
                    if let Ok(config) = serde_json::from_value::<AppConfig>(value) {
                        return config;
                    }
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
