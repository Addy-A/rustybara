use crate::tui::app::{ActionLogEntry, LogStatus, MenuAction, OutputChoice};
use crate::tui::{App, Screen};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

pub fn handle_events(app: &mut App) -> io::Result<()> {
    let mut action_entry = ActionLogEntry {
        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        action: String::new(),
        status: LogStatus::Ok,
    };

    if !event::poll(Duration::from_millis(50))? {
        return Ok(());
    }

    if let Event::Key(key) = event::read()? {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        if let KeyCode::Char('?') = key.code {
            app.toggle_help();
            return Ok(());
        }

        if app.show_help {
            app.show_help = false;
            return Ok(());
        }

        match app.screen {
            Screen::Main => match key.code {
                KeyCode::Up => app.menu_up(),
                KeyCode::Down => app.menu_down(),
                KeyCode::Enter => app.select_menu_item(),
                KeyCode::Char(ch) => {
                    if ch == 'q' {
                        app.quit();
                        return Ok(());
                    }
                    if let Some(idx) = MenuAction::ALL.iter().position(|a| a.hotkey() == Some(ch)) {
                        app.menu_index = idx;
                        app.select_menu_item();
                    }
                }
                KeyCode::Esc => app.quit(),
                _ => {}
            },
            Screen::FileSelect => match key.code {
                KeyCode::Up => app.local_file_up(),
                KeyCode::Down => app.local_file_down(),
                KeyCode::Tab => {
                    let local_dir = &std::env::current_dir().unwrap_or_default();
                    let path = PathBuf::from(&local_dir);
                    if path.is_dir() {
                        app.file_paths = std::fs::read_dir(&path)?
                            .filter_map(|e| e.ok())
                            .map(|e| e.path())
                            .filter(|p| {
                                p.extension()
                                    .and_then(|e| e.to_str())
                                    .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
                            })
                            .collect();
                        if app.file_paths.is_empty() {
                            app.status_message =
                                Some("No PDF files found in directory".to_string());
                        } else {
                            let count = app.file_paths.len();
                            app.status_message = Some(format!("{count} file(s) loaded"));
                            app.input_buffer.clear();
                            app.navigate(Screen::Main);
                            if let Some(Ok(meta)) = app
                                .file_paths
                                .first()
                                .map(|p| crate::process::load_metadata(p))
                            {
                                app.pdf_metadata = Some(meta);
                            }
                        }
                    } else {
                        app.status_message = Some("No PDFs found in local directory".into());
                    }
                }
                KeyCode::Esc => {
                    if app.file_paths.is_empty() {
                        app.quit();
                    } else {
                        app.navigate(Screen::Main);
                    }
                }
                KeyCode::Enter => {
                    if app.input_buffer.is_empty() {
                        app.select_local_file();
                    } else {
                        let trimmed = app.input_buffer.trim().trim_matches('"');
                        let unescaped = trimmed.replace("\\ ", " ");
                        let path = PathBuf::from(&unescaped);
                        if path.is_dir() {
                            app.file_paths = std::fs::read_dir(&path)?
                                .filter_map(|e| e.ok())
                                .map(|e| e.path())
                                .filter(|p| {
                                    p.extension()
                                        .and_then(|e| e.to_str())
                                        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
                                })
                                .collect();
                            if app.file_paths.is_empty() {
                                app.status_message =
                                    Some("No PDF files found in directory".to_string());
                            } else {
                                let count = app.file_paths.len();
                                app.status_message = Some(format!("{count} file(s) loaded"));
                                app.input_buffer.clear();
                                app.navigate(Screen::Main);
                                if let Some(Ok(meta)) = app
                                    .file_paths
                                    .first()
                                    .map(|p| crate::process::load_metadata(p))
                                {
                                    app.pdf_metadata = Some(meta);
                                }
                            }
                        } else if !path.exists() {
                            app.status_message =
                                Some(format!("Path not found: {}", path.display()));
                        } else if path
                            .extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
                        {
                            app.file_paths = vec![path];
                            app.status_message = Some("1 file(s) loaded".into());
                            app.input_buffer.clear();
                            app.navigate(Screen::Main);
                            if let Some(Ok(meta)) = app
                                .file_paths
                                .first()
                                .map(|p| crate::process::load_metadata(p))
                            {
                                app.pdf_metadata = Some(meta);
                            }
                        } else {
                            app.status_message = Some("Not a PDF file".into());
                        }
                    }
                }
                KeyCode::Char(c) => app.input_buffer.push(c),
                KeyCode::Backspace => {
                    app.input_buffer.pop();
                }
                _ => {}
            },
            Screen::OutputSelect => match key.code {
                KeyCode::Up | KeyCode::Down => {
                    app.output_choice = match app.output_choice {
                        OutputChoice::Same => OutputChoice::New,
                        OutputChoice::New => OutputChoice::Same,
                    }
                }
                KeyCode::Enter => {
                    match app.output_choice {
                        OutputChoice::Same => {
                            app.output_dir = None;
                            app.status_message = Some("Output: same location".into());
                            action_entry.action = "OutputSelect (SAME)".to_string();
                            app.action_log.push(action_entry);
                        }
                        OutputChoice::New => {
                            let trimmed = app.input_buffer.trim().trim_matches('"');
                            let path = PathBuf::from(&trimmed);
                            if path.is_dir() {
                                app.status_message = Some(format!("Output: {}", path.display()));
                                app.output_dir = Some(path);
                                action_entry.action = "OutputSelect (NEW)".to_string();
                                app.action_log.push(action_entry);
                            } else {
                                app.status_message =
                                    Some(format!("{} is not a directory", path.display()));
                                return Ok(());
                            }
                        }
                    }
                    app.input_buffer.clear();
                    app.navigate(Screen::Main);
                }
                KeyCode::Char(c) => {
                    if matches!(app.output_choice, OutputChoice::New) {
                        app.input_buffer.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if matches!(app.output_choice, OutputChoice::New) {
                        app.input_buffer.pop();
                    }
                }
                KeyCode::Esc => app.navigate(Screen::Main),
                _ => {}
            },
            Screen::ParamInput => match key.code {
                KeyCode::Esc => app.navigate(Screen::Main),
                KeyCode::Enter => {
                    let trimmed = app.input_buffer.trim().to_string();
                    if let Err(message) = parse_action_params(app, &trimmed) {
                        app.result_message = message;
                        app.last_result_ok = false;
                        app.action_log.push(ActionLogEntry {
                            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                            action: app.selected_action.label().to_string(),
                            status: LogStatus::Failed,
                        });
                        app.navigate(Screen::Result);
                    } else {
                        app.execute_action();
                    }
                }
                KeyCode::Char(c) => app.input_buffer.push(c),
                KeyCode::Backspace => {
                    app.input_buffer.pop();
                }
                _ => {}
            },
            Screen::Processing => {
                if key.code == KeyCode::Esc {
                    app.navigate(Screen::Main)
                };
            }
            Screen::Result => {
                if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                    app.navigate(Screen::Main)
                };
            }
        }
    }

    Ok(())
}

fn parse_positive(value: &str, label: &str) -> Result<f64, String> {
    let parsed = value
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("{label} must be a number"))?;
    if parsed.is_finite() && parsed > 0.0 {
        Ok(parsed)
    } else {
        Err(format!("{label} must be greater than zero"))
    }
}

fn parse_action_params(app: &mut App, input: &str) -> Result<(), String> {
    match app.selected_action {
        MenuAction::ResizeToBleed => {
            app.params.bleed_pts = parse_positive(input, "Bleed")? * 72.0;
        }
        MenuAction::AddTrimBox => {
            app.params.trim_box_bleed_pts = parse_positive(input, "TrimBox inset")? * 72.0;
        }
        MenuAction::SetMediaBox => {
            let parts: Vec<_> = input.split(',').collect();
            if parts.len() != 2 {
                return Err("MediaBox requires width,height in inches".into());
            }
            app.params.media_width_inches = parse_positive(parts[0], "Width")?;
            app.params.media_height_inches = parse_positive(parts[1], "Height")?;
        }
        MenuAction::Rotate => {
            let degrees = input
                .parse::<i32>()
                .map_err(|_| "Rotation must be a whole number".to_string())?;
            if degrees % 90 != 0 {
                return Err("Rotation must be a multiple of 90".into());
            }
            app.params.rotate_degrees = degrees;
        }
        MenuAction::ExportImages => {
            let parts: Vec<_> = input.split(',').map(str::trim).collect();
            if parts.len() != 3 {
                return Err("Export settings require format,dpi,quality".into());
            }
            let format = parts[0].to_lowercase();
            if !["jpg", "png", "webp", "tiff"].contains(&format.as_str()) {
                return Err("Format must be jpg, png, webp, or tiff".into());
            }
            let dpi = parts[1]
                .parse::<u32>()
                .map_err(|_| "DPI must be a positive whole number".to_string())?;
            let quality = parts[2]
                .parse::<u8>()
                .map_err(|_| "Quality must be from 1 through 100".to_string())?;
            if dpi == 0 {
                return Err("DPI must be greater than zero".into());
            }
            if !(1..=100).contains(&quality) {
                return Err("Quality must be from 1 through 100".into());
            }
            app.params.export_format = format;
            app.params.export_dpi = dpi;
            app.params.export_quality = quality;
        }
        MenuAction::ExtractPages => {
            crate::process::validate_page_selection(input).map_err(|error| error.to_string())?;
            app.params.extract_pages = input.to_string();
        }
        MenuAction::SplitPages => {
            app.params.panel_width_inches = parse_positive(input, "Panel width")?;
        }
        MenuAction::StitchPages => {
            app.params.spread_width_inches = parse_positive(input, "Spread width")?;
        }
        MenuAction::RemapColors => {
            let parts: Vec<_> = input.split(',').collect();
            if parts.len() != 3 {
                return Err("Remap requires from-CMYK,to-CMYK,tolerance".into());
            }
            let parse_cmyk = |value: &str| -> Result<[f64; 4], String> {
                let values: Vec<f64> = value
                    .split_whitespace()
                    .map(|item| {
                        item.parse::<f64>()
                            .map_err(|_| format!("Invalid CMYK value: {item}"))
                    })
                    .collect::<Result<_, _>>()?;
                values
                    .try_into()
                    .map_err(|_| "Each CMYK color requires four values".to_string())
            };
            let from = parse_cmyk(parts[0])?;
            let to = parse_cmyk(parts[1])?;
            if !from
                .iter()
                .chain(&to)
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            {
                return Err("CMYK values must be between 0 and 1".into());
            }
            let tolerance = parts[2]
                .trim()
                .parse::<f64>()
                .map_err(|_| "Tolerance must be a number".to_string())?;
            if !tolerance.is_finite() || !(0.0..=1.0).contains(&tolerance) {
                return Err("Tolerance must be between 0 and 1".into());
            }
            app.params.remap_from = from;
            app.params.remap_to = to;
            app.params.remap_tolerance = tolerance;
        }
        MenuAction::ConvertColorSpace => {
            let parts: Vec<_> = input.split(',').map(str::trim).collect();
            if parts.len() != 3 || parts[0].is_empty() || parts[1].is_empty() {
                return Err("Conversion requires source-profile,destination-profile,intent".into());
            }
            let intent = match parts[2].to_ascii_lowercase().as_str() {
                "perceptual" => "Perceptual",
                "relativecolorimetric" | "relative" => "RelativeColorimetric",
                "saturation" => "Saturation",
                "absolutecolorimetric" | "absolute" => "AbsoluteColorimetric",
                _ => {
                    return Err(
                        "Intent must be perceptual, relative, saturation, or absolute".into(),
                    );
                }
            };
            app.params.from_profile = parts[0].to_string();
            app.params.to_profile = parts[1].to_string();
            app.params.rendering_intent = intent.to_string();
        }
        MenuAction::FlattenSpots => {
            app.params.flatten_icc_path = input.trim_matches('"').to_string();
        }
        MenuAction::TrimMarks
        | MenuAction::OutlineText
        | MenuAction::ToggleOverwrite
        | MenuAction::OutputPath
        | MenuAction::ChangeFiles
        | MenuAction::Quit => {}
    }
    Ok(())
}
