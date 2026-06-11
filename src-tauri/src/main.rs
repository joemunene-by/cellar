//! cellar: a Mac mini M4 launcher for Windows games.
//!
//! Single Tauri window. Rust backend orchestrates Wine bottles, calls
//! the GPTK-patched wine64 binary, runs FitGirl installers, and
//! persists a small library at ~/.cellar/library.json. React frontend
//! drives every action through `tauri::invoke`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod archive;
mod d3dmetal;
mod installer;
mod library;
mod prereq;
mod profiles;
mod runtime;
mod tools;
mod watch;
mod wine;

use library::Library;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Library::load())
        .setup(|app| {
            // Watch ~/Games-source for newly-dropped games. No-op if the
            // directory doesn't exist yet.
            watch::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            wine::wine_create_bottle,
            wine::wine_list_bottles,
            wine::wine_remove_bottle,
            wine::wine_inject_dxvk,
            wine::wine_bottle_dxvk_status,
            wine::wine_bottle_smoke_test,
            wine::wine_scan_bottle_exes,
            wine::wine_run_winetricks,
            library::library_list,
            library::library_add,
            library::library_remove,
            library::library_update_settings,
            profiles::profiles_list,
            profiles::profiles_find,
            prereq::prereq_install,
            prereq::prereq_check,
            prereq::prereq_check_all,
            runtime::runtime_status,
            runtime::runtime_test_wine,
            runtime::runtime_launch,
            installer::installer_detect,
            installer::installer_run,
            archive::archive_peek,
            tools::bottle_inspect,
            tools::crash_report,
            d3dmetal::d3dmetal_list,
            d3dmetal::d3dmetal_get,
            d3dmetal::d3dmetal_set,
        ])
        .run(tauri::generate_context!())
        .expect("cellar: failed to start tauri runtime");
}
