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
    OutputPath,
    ChangeFiles,
    Quit,
}

pub enum Screen {
    Main,
    FileSelect,
    OutputSelect,
    ParamInput,
    Processing,
    Result,
}

pub enum OutputChoice {
    Same,
    New,
}

pub enum SingleAsciiIconState {
    NoTrimBox,
    TrimBoxPresent,
    InvalidPdf,
    PureCMYK,
    PureRGB,
    Loading,
    ResizingToBleed,
    ExportImages,
    DeleteTrimMarks,
    FailedToLoad,
}

pub enum BatchAsciiIconState {
    NoTrimBox,
    TrimBoxPresent,
    InvalidPdf,
    PureCMYK,
    PureRGB,
    // Loading, NOTE: this could be handled as a single icon since this indexes individually.
    // Deferring for now.
    ResizingToBleed,
    ExportImages,
    DeleteTrimMarks,
    // FailedToLoad, NOTE: similarly to the Loading enum value, single icon could handle this.
    // Deferring for now.
}

pub struct PdfMetadata {
    pub trimbox: Option<[f32; 4]>,
    pub mediabox: [f32; 4],
    pub bleedbox: Option<[f32; 4]>,
    pub bleed_pts: f32,
    pub color_space: ColorSpaceInfo,
    pub page_count: u32,
    pub file_size_kb: u64,
    pub editing: String, // current editing state label
}

impl Default for PdfMetadata {
    fn default() -> Self {
        Self {
            trimbox: None,
            mediabox: [0.0, 0.0, 0.0, 0.0],
            bleedbox: None,
            bleed_pts: 9.0,
            color_space: ColorSpaceInfo::Unknown,
            page_count: 0,
            file_size_kb: 0,
            editing: String::new(),
        }
    }
}

pub enum ColorSpaceInfo {
    PureCMYK,
    PureRGB,
    Mixed,
    CPPE,
    Unknown,
}

pub enum LogStatus {
    Ok,
    Failed,
    Partial,
}

pub struct ActionLogEntry {
    pub timestamp: String,
    pub action: String,
    pub status: LogStatus,
}

impl MenuAction {
    pub const ALL: &[MenuAction] = &[
        Self::TrimMarks,
        Self::ResizeToBleed,
        Self::ExportImages,
        Self::RemapColors,
        Self::PreviewPage,
        Self::ToggleOverwrite,
        Self::OutputPath,
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
            Self::OutputPath => "Output Path",
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
            Self::OutputPath => Some('/'),
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
    pub output_dir: Option<PathBuf>,
    pub output_choice: OutputChoice,
    pub status_message: Option<String>,
    pub file_paths: Vec<PathBuf>,
    pub show_help: bool,
    pub input_buffer: String,
    pub result_message: String,
    pub last_result_ok: bool,
    pub pdf_metadata: Option<PdfMetadata>,
    pub action_log: Vec<ActionLogEntry>,
    pub preview_page: usize,
    pub idle_quip: String,
    pub local_file_index: usize,
    pub local_files: Vec<PathBuf>,
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
            output_dir: None,
            output_choice: OutputChoice::Same,
            status_message: None,
            file_paths: Vec::new(),
            show_help: false,
            input_buffer: String::new(),
            result_message: String::new(),
            last_result_ok: false,
            pdf_metadata: Some(PdfMetadata::default()),
            action_log: Vec::new(),
            preview_page: 0,
            idle_quip: crate::tui::quips::random_quip(),
            local_file_index: 0,
            local_files: crate::process::load_local_files(
                &std::env::current_dir().unwrap_or_default(),
            )
            .unwrap_or_default(),
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

    pub fn ascii_icon_state(&self) -> SingleAsciiIconState {
        if matches!(self.screen, Screen::Processing) {
            return SingleAsciiIconState::Loading;
        }
        match &self.pdf_metadata {
            None => SingleAsciiIconState::NoTrimBox,
            Some(m) if m.trimbox.is_none() => SingleAsciiIconState::NoTrimBox,
            Some(m) => match &m.color_space {
                ColorSpaceInfo::PureRGB => SingleAsciiIconState::PureRGB,
                ColorSpaceInfo::PureCMYK => SingleAsciiIconState::PureCMYK,
                _ => SingleAsciiIconState::TrimBoxPresent,
            },
        }
    }

    pub fn local_file_up(&mut self) {
        if self.local_file_index > 0 {
            self.local_file_index -= 1;
        }
    }

    pub fn local_file_down(&mut self) {
        if self.local_file_index + 1 < self.local_files.len() {
            self.local_file_index += 1;
        }
    }

    pub fn select_local_file(&mut self) {
        if let Some(path) = self.local_files.get(self.local_file_index).cloned() {
            self.file_paths = vec![path.clone()];
            if let Ok(meta) = crate::process::load_metadata(&path) {
                self.pdf_metadata = Some(meta);
            }
            self.navigate(Screen::Main);
        }
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
        let mut action_entry = self::ActionLogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            action: String::new(),
            status: LogStatus::Ok,
        };
        self.selected_action = action;

        match action {
            MenuAction::ToggleOverwrite => {
                self.overwrite = !self.overwrite;
                if self.overwrite {
                    action_entry.action = "ToggleOverwrite (TRUE)".to_string();
                } else {
                    action_entry.action = "ToggleOverwrite (FALSE)".to_string();
                }
                self.action_log.push(action_entry);
            }
            MenuAction::OutputPath => {
                self.input_buffer = self
                    .output_dir
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                self.navigate(Screen::OutputSelect);
            }
            MenuAction::ChangeFiles => {
                self.input_buffer.clear();
                self.navigate(Screen::FileSelect);
            }
            MenuAction::Quit => self.quit(),
            a if a.needs_params() => {
                self.input_buffer = match a {
                    MenuAction::ResizeToBleed => format!("{:.4}", self.params.bleed_pts / 72.0),
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
            Ok((msg, new_paths, entry)) => {
                self.result_message = msg;
                self.last_result_ok = true;
                self.action_log.push(entry);
                if !new_paths.is_empty() {
                    self.file_paths = new_paths;
                }
                if let Some(Ok(meta)) = self
                    .file_paths
                    .first()
                    .map(|p| crate::process::load_metadata(p))
                {
                    self.pdf_metadata = Some(meta);
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
        rustybara::Error::Color(_) => format!("Color conversion failed: {e}"),
    }
}
