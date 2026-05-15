pub mod commands;

use std::sync::Mutex;

use commands::{
    add_trim_box, convert_color_space, export_images, extract_pages, flatten_spots, load_metadata,
    open_file_dialog, remap_colors, resize_to_bleed, split_pages, trim_marks, ProcessingLock,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ProcessingLock(Mutex::new(false)))
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
            load_metadata,
            open_file_dialog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running rbara-gui");
}
