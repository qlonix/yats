// Windowsのリリースビルドで追加のコンソールウィンドウを防ぐ。削除禁止！！
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri_app_lib::run()
}
