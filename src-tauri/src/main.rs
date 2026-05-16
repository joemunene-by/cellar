//! cellar: a Mac mini M4 launcher for Windows games.
//!
//! Single Tauri window. Rust backend orchestrates Wine bottles, calls
//! the GPTK-patched wine64 binary, runs FitGirl installers, and
//! persists a small library at ~/.cellar/library.json. React frontend
//! drives every action through `tauri::invoke`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod installer;
mod library;
mod runtime;
mod wine;

use library::Library;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Library::load())
        .invoke_handler(tauri::generate_handler![
            wine::wine_create_bottle,
            wine::wine_list_bottles,
            wine::wine_remove_bottle,
            wine::wine_inject_dxvk,
            wine::wine_bottle_dxvk_status,
            library::library_list,
            library::library_add,
            library::library_remove,
            library::library_update_settings,
            runtime::runtime_status,
            runtime::runtime_test_wine,
            runtime::runtime_launch,
            installer::installer_detect,
            installer::installer_run,
        ])
        .run(tauri::generate_context!())
        .expect("cellar: failed to start tauri runtime");
}
