pub mod commands;

use std::collections::HashMap;
use std::sync::Mutex;

use commands::{
    add_trim_box, convert_color_space, exit_app, export_images, extract_pages, flatten_spots,
    get_file_size, list_custom_profiles, list_dirs, list_pdf_files, load_icc_profile,
    load_metadata, load_persisted_profiles, load_persisted_settings, load_settings,
    minimize_window, notify_viewer, open_file_dialog, open_in_viewer, open_in_viewer_persistent,
    outline_text, read_xmp_metadata, remap_colors, resize_to_bleed, rotate, save_settings,
    set_media_box, split_pages, stitch_pages, toggle_maximize_window, trim_marks, AppSettings,
    ProcessingLock, ProfileRegistry, SettingsDto, ViewerHandle,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ProcessingLock(Mutex::new(false)))
        .manage(ProfileRegistry(Mutex::new(HashMap::new())))
        .manage(ViewerHandle(Mutex::new(None)))
        .manage(AppSettings(Mutex::new(SettingsDto::default())))
        .setup(|app| {
            load_persisted_profiles(app);
            load_persisted_settings(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            trim_marks,
            resize_to_bleed,
            set_media_box,
            rotate,
            add_trim_box,
            outline_text,
            split_pages,
            stitch_pages,
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
            open_in_viewer_persistent,
            notify_viewer,
            read_xmp_metadata,
            exit_app,
            list_dirs,
            list_pdf_files,
            minimize_window,
            toggle_maximize_window,
            load_settings,
            save_settings,
            get_file_size,
        ])
        .run(tauri::generate_context!())
        .expect("error while running rbara-gui");
}
