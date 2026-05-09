pub mod commands;

use std::sync::Mutex;

use commands::{
    export_images, load_metadata, open_file_dialog, remap_colors, resize_to_bleed, trim_marks,
    ProcessingLock,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ProcessingLock(Mutex::new(false)))
        .invoke_handler(tauri::generate_handler![
            trim_marks,
            resize_to_bleed,
            export_images,
            remap_colors,
            load_metadata,
            open_file_dialog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running rbara-gui");
}
