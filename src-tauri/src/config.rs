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
    #[serde(default = "default_sensitivity")]
    pub scroll_sensitivity: u32,
    #[serde(default = "default_scroll_speed")]
    pub scroll_speed: u32,
    #[serde(default)]
    pub scroll_invert: bool,
    #[serde(default = "default_linux_min_distance")]
    pub linux_min_distance: i32,
    #[serde(default = "default_linux_min_speed")]
    pub linux_min_speed: f32,
    #[serde(default = "default_linux_min_scroll_speed")]
    pub linux_min_scroll_speed: f32,
    #[serde(default = "default_linux_max_scroll_speed")]
    pub linux_max_scroll_speed: f32,
    #[serde(default)]
    pub linux_use_scroll_curve: bool,
    #[serde(default = "default_linux_scroll_curve")]
    pub linux_scroll_curve: Vec<(f32, f32)>,
    #[serde(default = "default_palm_rejection")]
    pub palm_rejection: bool,
    #[serde(default = "default_palm_rejection_delay")]
    pub palm_rejection_delay_ms: u64,
}

fn default_sensitivity() -> u32 {
    1
}

fn default_scroll_speed() -> u32 {
    5
}

fn default_linux_min_distance() -> i32 {
    0
}

fn default_linux_min_speed() -> f32 {
    0.0
}

fn default_linux_min_scroll_speed() -> f32 {
    1.0
}

fn default_linux_max_scroll_speed() -> f32 {
    #[cfg(target_os = "linux")]
    return 100.0;
    #[cfg(target_os = "windows")]
    return 1000.0;
}

fn default_linux_scroll_curve() -> Vec<(f32, f32)> {
    vec![
        (0.0, 1.6335227),
        (1305.0, 3.90625),
        (1730.0, 21.448864),
        (2000.0, 79.360794),
    ]
}

fn default_palm_rejection() -> bool {
    true
}

fn default_palm_rejection_delay() -> u64 {
    500
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut mappings = HashMap::new();
        // デフォルトマッピング (v2.4.0)
        mappings.insert(Key::KeyD, Action::MouseClick(MouseButton::Right));
        mappings.insert(Key::KeyF, Action::MouseClick(MouseButton::Left));
        mappings.insert(
            Key::KeyG,
            Action::KeyMacro(vec![vec![Key::Alt, Key::LeftArrow]]),
        );
        mappings.insert(
            Key::KeyH,
            Action::KeyMacro(vec![vec![Key::Alt, Key::RightArrow]]),
        );
        mappings.insert(Key::KeyJ, Action::MouseClick(MouseButton::Left));
        mappings.insert(Key::KeyK, Action::MouseClick(MouseButton::Right));
        mappings.insert(
            Key::KeyL,
            Action::MouseScroll(ScrollConfig {
                sensitivity: 100,
                invert: false,
                acceleration: false,
            }),
        );
        mappings.insert(Key::KeyQ, Action::Window(WindowAction::Close));
        mappings.insert(Key::KeyR, Action::Window(WindowAction::Maximize));
        mappings.insert(
            Key::KeyS,
            Action::MouseScroll(ScrollConfig {
                sensitivity: 100,
                invert: false,
                acceleration: false,
            }),
        );
        mappings.insert(Key::KeyV, Action::MouseDoubleClick(MouseButton::Left));
        mappings.insert(Key::KeyW, Action::Window(WindowAction::Minimize));

        Self {
            mappings,
            release_delay_ms: 200,
            scroll_sensitivity: 1,
            scroll_speed: 5,
            scroll_invert: true,
            linux_min_distance: 0,
            linux_min_speed: 0.0,
            linux_min_scroll_speed: 1.0,
            linux_max_scroll_speed: 100.0,
            linux_use_scroll_curve: true,
            linux_scroll_curve: default_linux_scroll_curve(),
            palm_rejection: true,
            palm_rejection_delay_ms: 500,
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
