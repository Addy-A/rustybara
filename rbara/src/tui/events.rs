use crate::tui::app::{MenuAction, OutputChoice};
use crate::tui::{App, Screen};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

pub fn handle_events(app: &mut App) -> io::Result<()> {
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
                KeyCode::Esc => {
                    if app.file_paths.is_empty() {
                        app.quit();
                    } else {
                        app.navigate(Screen::Main);
                    }
                }
                KeyCode::Enter => {
                    if !app.input_buffer.is_empty() {
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
                                    Some(format!("No PDF files found in directory").into());
                            } else {
                                let count = app.file_paths.len();
                                app.status_message = Some(format!("{count} file(s) loaded"));
                                app.input_buffer.clear();
                                app.navigate(Screen::Main);
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
                        }
                        OutputChoice::New => {
                            let trimmed = app.input_buffer.trim().trim_matches('"');
                            let path = PathBuf::from(&trimmed);
                            if path.is_dir() {
                                app.status_message = Some(format!("Output: {}", path.display()));
                                app.output_dir = Some(path);
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
                    match app.selected_action {
                        MenuAction::ResizeToBleed => {
                            if let Ok(val) = trimmed.parse::<f64>() {
                                app.params.bleed_pts = val;
                            }
                        }
                        MenuAction::ExportImages => {
                            let parts: Vec<&str> = trimmed.split(',').collect();
                            if let Some(&fmt) = parts.first() {
                                let fmt = fmt.trim().to_lowercase();
                                if ["jpg", "png", "webp", "tiff"].contains(&fmt.as_str()) {
                                    app.params.export_format = fmt;
                                }
                            }
                            if let Some(&dpi_str) = parts.get(1)
                                && let Ok(val) = dpi_str.trim().parse::<u32>()
                            {
                                app.params.export_dpi = val;
                            }
                        }
                        MenuAction::RemapColors => {
                            let parts: Vec<&str> = trimmed.split(',').collect();
                            if let Some(from_str) = parts.first() {
                                let vals: Vec<f64> = from_str
                                    .split_whitespace()
                                    .filter_map(|v| v.parse().ok())
                                    .collect();
                                if vals.len() == 4 {
                                    app.params.remap_from = [vals[0], vals[1], vals[2], vals[3]];
                                }
                            }
                            if let Some(to_str) = parts.get(1) {
                                let vals: Vec<f64> = to_str
                                    .split_whitespace()
                                    .filter_map(|v| v.parse().ok())
                                    .collect();
                                if vals.len() == 4 {
                                    app.params.remap_to = [vals[0], vals[1], vals[2], vals[3]];
                                }
                            }
                            if let Some(tol_str) = parts.get(2) {
                                if let Ok(val) = tol_str.trim().parse::<f64>() {
                                    app.params.remap_tolerance = val;
                                }
                            }
                        }
                        MenuAction::PreviewPage => {}
                        _ => {}
                    }
                    app.execute_action();
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
