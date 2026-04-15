use crate::process::run_tui_action;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    TrimMarks,
    ResizeToBleed,
    ExportImages,
    RemapColors,
    PreviewPage,
    ToggleOverwrite,
    ChangeFiles,
    Quit,
}

impl MenuAction {
    pub const ALL: &[MenuAction] = &[
        Self::TrimMarks,
        Self::ResizeToBleed,
        Self::ExportImages,
        Self::RemapColors,
        Self::PreviewPage,
        Self::ToggleOverwrite,
        Self::ChangeFiles,
        Self::Quit,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::TrimMarks => "Trim Marks",
            Self::ResizeToBleed => "Resize to Bleed",
            Self::ExportImages => "Export Images",
            Self::RemapColors => "Remap Colors",
            Self::PreviewPage => "Preview Page",
            Self::ToggleOverwrite => "Toggle Overwrite",
            Self::ChangeFiles => "Change Files",
            Self::Quit => "Quit",
        }
    }

    pub fn hotkey(self) -> Option<char> {
        match self {
            Self::TrimMarks => Some('t'),
            Self::ResizeToBleed => Some('r'),
            Self::ExportImages => Some('x'),
            Self::RemapColors => Some('m'),
            Self::PreviewPage => Some('p'),
            Self::ToggleOverwrite => Some('o'),
            Self::ChangeFiles => Some('f'),
            Self::Quit => Some('q'),
        }
    }

    pub fn needs_params(self) -> bool {
        matches!(
            self,
            Self::ResizeToBleed | Self::ExportImages | Self::RemapColors
        )
    }
}

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
    pub remap_from: [f64; 4],
    pub remap_to: [f64; 4],
    pub remap_tolerance: f64,
}

impl Default for ActionParams {
    fn default() -> Self {
        Self {
            bleed_pts: 9.0,
            export_format: "jpg".into(),
            export_dpi: 150,
            remap_from: [1.0, 1.0, 1.0, 1.0],
            remap_to: [0.6, 0.4, 0.2, 1.0],
            remap_tolerance: 1.0,
        }
    }
}

pub struct App {
    pub screen: Screen,
    pub running: bool,
    pub menu_index: usize,
    pub selected_action: MenuAction,
    pub params: ActionParams,
    pub overwrite: bool,
    pub status_message: Option<String>,
    pub file_paths: Vec<PathBuf>,
    pub show_help: bool,
    pub input_buffer: String,
    pub result_message: String,
    pub last_result_ok: bool,
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
            selected_action: MenuAction::ChangeFiles,
            params: ActionParams::default(),
            overwrite: false,
            status_message: None,
            file_paths: Vec::new(),
            show_help: false,
            input_buffer: String::new(),
            result_message: String::new(),
            last_result_ok: false,
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
        if self.menu_index + 1 < MenuAction::ALL.len() {
            self.menu_index += 1;
        }
    }
    pub fn select_menu_item(&mut self) {
        let action = MenuAction::ALL[self.menu_index];
        self.selected_action = action;

        match action {
            MenuAction::ToggleOverwrite => self.overwrite = !self.overwrite,
            MenuAction::ChangeFiles => {
                self.input_buffer.clear();
                self.navigate(Screen::FileSelect);
            }
            MenuAction::Quit => self.quit(),
            a if a.needs_params() => {
                self.input_buffer = match a {
                    MenuAction::ResizeToBleed => self.params.bleed_pts.to_string(),
                    MenuAction::ExportImages => {
                        format!("{},{}", self.params.export_format, self.params.export_dpi,)
                    }
                    MenuAction::RemapColors => {
                        let from = self.params.remap_from;
                        let to = self.params.remap_to;
                        let tolerance = self.params.remap_tolerance;
                        format!(
                            "{} {} {} {},{} {} {} {},{}",
                            from[0],
                            from[1],
                            from[2],
                            from[3],
                            to[0],
                            to[1],
                            to[2],
                            to[3],
                            tolerance,
                        )
                    }
                    _ => String::new(),
                };
                self.navigate(Screen::ParamInput);
            }
            _ => self.execute_action(),
        }
    }

    pub fn execute_action(&mut self) {
        if self.file_paths.is_empty() {
            self.result_message = "No files loaded. Press [c] to select files.".into();
            self.navigate(Screen::FileSelect);
            return;
        }
        let missing: Vec<_> = self.file_paths.iter().filter(|p| !p.exists()).collect();
        if !missing.is_empty() {
            self.result_message = format!(
                "File not found:\n{}",
                missing
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            self.navigate(Screen::FileSelect);
            return;
        }

        match run_tui_action(self) {
            Ok((msg, new_paths)) => {
                self.result_message = msg;
                self.last_result_ok = true;
                if !new_paths.is_empty() {
                    self.file_paths = new_paths;
                }
            }
            Err(e) => {
                self.result_message = friendly_error(e);
                self.last_result_ok = false;
            }
        }
        self.navigate(Screen::Result);
    }
}

fn friendly_error(e: rustybara::Error) -> String {
    match &e {
        rustybara::Error::Io(ioe) => match ioe.kind() {
            std::io::ErrorKind::NotFound => format!("File not found: {e}"),
            std::io::ErrorKind::PermissionDenied => format!("Permission denied: {e}"),
            _ => format!("I/O error: {e}"),
        },
        rustybara::Error::Render(_) => format!(
            "Render failed - Pdfium library not found or failed to initialize.\n\
            Place pdfium.dll (or MAC OS: libpdfium.dylib) in the executable directory.\n\
            Details: {e}"
        ),
        rustybara::Error::Pdf(_) => format!(
            "Failed to parse PDF — the file may be corrupted or password-protected.\n\n\
             Details: {e}"
        ),
        rustybara::Error::Image(_) => format!("Image encoding failed: {e}"),
        #[cfg(feature = "color")]
        rustybara::Error::Color(_) => format!("Color conversion failed: {e}"),
        _ => format!("Unknown error: {e}"),
    }
}
