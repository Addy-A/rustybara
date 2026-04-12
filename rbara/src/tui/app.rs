use crate::process::run_tui_action;
use std::path::PathBuf;
const MENU_ITEMS: &[&str] = &[
    "Trim Marks",
    "Resize to Bleed",
    "Export Images",
    "Preview Page",
    "Toggle Overwrite",
    "Change Files",
    "Quit",
];
pub enum Screen {
    Main,
    FileSelect,
    ParamInput,
    Processing,
    Result,
}

pub struct ActionParams {
    pub bleed_pts: f64,
    pub export_format: String,
    pub export_dpi: u32,
}

impl Default for ActionParams {
    fn default() -> Self {
        Self {
            bleed_pts: 9.0,
            export_format: "jpg".into(),
            export_dpi: 150,
        }
    }
}

pub struct App {
    pub screen: Screen,
    pub running: bool,
    pub menu_index: usize,
    pub selected_action: usize,
    pub params: ActionParams,
    pub overwrite: bool,
    pub status_message: Option<String>,
    pub file_paths: Vec<PathBuf>,
    pub show_help: bool,
    pub input_buffer: String,
    pub result_message: String,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::FileSelect,
            running: true,
            menu_index: 0,
            selected_action: 0,
            params: ActionParams::default(),
            overwrite: false,
            status_message: None,
            file_paths: Vec::new(),
            show_help: false,
            input_buffer: String::new(),
            result_message: String::new(),
        }
    }
    pub fn tick(&mut self) {
        // TODO: Just a placeholder for future periodic updates (spinner, etc.)
    }
    pub fn quit(&mut self) {
        self.running = false;
    }
    pub fn navigate(&mut self, screen: Screen) {
        self.screen = screen;
    }
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }
    pub fn menu_up(&mut self) {
        if self.menu_index > 0 {
            self.menu_index -= 1;
        }
    }
    pub fn menu_down(&mut self) {
        if self.menu_index + 1 < MENU_ITEMS.len() {
            self.menu_index += 1;
        }
    }
    pub fn select_menu_item(&mut self) {
        match self.menu_index {
            0 => {
                self.selected_action = 0;
                self.execute_action();
            }
            1 => {
                self.selected_action = 1;
                self.input_buffer = self.params.bleed_pts.to_string();
                self.navigate(Screen::ParamInput);
            }
            2 => {
                self.selected_action = 2;
                self.input_buffer =
                    format!("{},{}", self.params.export_format, self.params.export_dpi);
                self.navigate(Screen::ParamInput);
            }
            3 => {
                self.selected_action = 3;
                self.execute_action();
            }
            4 => self.overwrite = !self.overwrite,
            5 => {
                self.input_buffer.clear();
                self.navigate(Screen::FileSelect);
            }
            6 => self.quit(),
            _ => {}
        }
    }

    pub fn execute_action(&mut self) {
        match run_tui_action(self) {
            Ok((msg, new_paths)) => {
                self.result_message = msg;
                if !new_paths.is_empty() {
                    self.file_paths = new_paths;
                }
            }
            Err(e) => self.result_message = format!("Error: {e}"),
        }
        self.navigate(Screen::Result);
    }

    pub fn menu_items() -> &'static [&'static str] {
        MENU_ITEMS
    }
}
