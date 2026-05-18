pub mod commands;

use std::collections::HashMap;
use std::sync::Mutex;

use commands::{
    add_trim_box, convert_color_space, export_images, extract_pages, flatten_spots,
    list_custom_profiles, load_icc_profile, load_metadata, load_persisted_profiles,
    open_file_dialog, open_in_viewer, remap_colors, resize_to_bleed, split_pages, trim_marks,
    ProcessingLock, ProfileRegistry,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ProcessingLock(Mutex::new(false)))
        .manage(ProfileRegistry(Mutex::new(HashMap::new())))
        .setup(|app| {
            load_persisted_profiles(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            trim_marks,
            resize_to_bleed,
            add_trim_box,
            split_pages,
            extract_pages,
            export_images,
            remap_colors,
            flatten_spots,
            convert_color_space,
            load_icc_profile,
            list_custom_profiles,
            load_metadata,
            open_file_dialog,
            open_in_viewer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running rbara-gui");
}
