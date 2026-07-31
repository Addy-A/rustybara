use crate::tui::app::MenuAction;
use crate::tui::{App, Screen};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub enum AppColor {
    PrimaryOrange,
    SecondaryOrange,
    TertiaryOrange,
}

impl AppColor {
    pub fn app_colors(&self) -> Color {
        match self {
            AppColor::PrimaryOrange => Color::Rgb(232, 104, 7),
            AppColor::SecondaryOrange => Color::Rgb(200, 80, 5),
            AppColor::TertiaryOrange => Color::Rgb(160, 60, 3),
        }
    }
}

impl From<AppColor> for Color {
    fn from(app_color: AppColor) -> Color {
        app_color.app_colors()
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    if app.show_help {
        draw_help(frame);
        return;
    }

    match app.screen {
        Screen::Main => draw_main(frame, app),
        Screen::FileSelect => draw_file_select(frame, app),
        Screen::OutputSelect => draw_output_input(frame, app),
        Screen::ParamInput => draw_param_input(frame, app),
        Screen::Processing => draw_processing(frame, app),
        Screen::Result => draw_result(frame, app),
    }
}
fn draw_main(frame: &mut Frame, app: &App) {
    let outer = Layout::vertical([
        Constraint::Length(3), // title
        Constraint::Min(0),    // content
        Constraint::Length(1), // status
        Constraint::Length(1), // footer
    ])
    .split(frame.area());

    let columns = Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(outer[1]);

    let right_halves =
        Layout::vertical([Constraint::Min(0), Constraint::Length(12)]).split(columns[1]);

    let right_bottom =
        Layout::vertical([Constraint::Min(0), Constraint::Length(5)]).split(right_halves[1]);

    let title = Paragraph::new(" Rustybara — Prepress Toolkit")
        .style(Style::default().fg(AppColor::PrimaryOrange.into()).bold())
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, outer[0]);

    let visible_rows = columns[0].height.saturating_sub(1).max(1) as usize;
    let max_start = MenuAction::ALL.len().saturating_sub(visible_rows);
    let start = app
        .menu_index
        .saturating_sub(visible_rows.saturating_sub(1))
        .min(max_start);
    let end = (start + visible_rows).min(MenuAction::ALL.len());
    let items: Vec<ListItem> = MenuAction::ALL[start..end]
        .iter()
        .enumerate()
        .map(|(offset, action)| {
            let i = start + offset;
            let style = if i == app.menu_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(AppColor::PrimaryOrange.into())
                    .bold()
            } else {
                Style::default()
            };
            let marker = if i == start && start > 0 {
                "^"
            } else if i + 1 == end && end < MenuAction::ALL.len() {
                "v"
            } else {
                " "
            };
            ListItem::new(format!("{marker} {}", action.label())).style(style)
        })
        .collect();
    let menu = List::new(items).block(Block::default().padding(Padding::top(1)));
    frame.render_widget(menu, columns[0]);

    draw_ascii_icon(frame, app, right_halves[0]);
    draw_inspection_table(frame, app, right_bottom[0]);
    draw_action_log(frame, app, right_bottom[1]);

    let overwrite_tag = if app.overwrite { " [OVERWRITE] " } else { "" };
    let status_text = if app.file_paths.is_empty() {
        format!("No files loaded{overwrite_tag}")
    } else if app.file_paths.len() == 1 {
        format!(" {}{overwrite_tag}", app.file_paths[0].display())
    } else {
        format!(" {} files loaded{overwrite_tag}", app.file_paths.len())
    };
    frame.render_widget(
        Paragraph::new(status_text).style(Style::default().fg(AppColor::PrimaryOrange.into())),
        outer[2],
    );

    frame.render_widget(
        Paragraph::new(" Up/Down navigate  Enter run  hotkeys select  [?] help  [q] quit")
            .style(Style::default().fg(Color::DarkGray)),
        outer[3],
    );
}
fn draw_ascii_icon(frame: &mut Frame, app: &App, area: Rect) {
    use crate::tui::app::SingleAsciiIconState;
    use crate::tui::ascii_icon::icon_lines;

    let state = if !app.last_result_ok && matches!(app.screen, Screen::Result) {
        SingleAsciiIconState::FailedToLoad
    } else {
        app.ascii_icon_state()
    };

    let subtitle = format!(
        r#" {} {} "#,
        app.file_paths.len(),
        if app.file_paths.len() == 1 {
            "file"
        } else {
            "files"
        }
    );

    let lines = icon_lines(&state, &subtitle);
    let text = lines.join("\n");

    let icon = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(AppColor::PrimaryOrange.into()))
        .block(Block::default());
    frame.render_widget(icon, area);
}
fn draw_inspection_table(frame: &mut Frame, app: &App, area: Rect) {
    use crate::tui::app::ColorSpaceInfo;

    let orange: Color = AppColor::PrimaryOrange.into();
    let filled = Style::default().fg(Color::Black).bg(orange);

    let fmt_box = |b: Option<[f32; 4]>| -> String {
        b.map(|v| {
            format!(
                "{:.2} x {:.2} in",
                (v[2] - v[0]) / 72.0,
                (v[3] - v[1]) / 72.0
            )
        })
        .unwrap_or_else(|| "—".into())
    };

    let rows: Vec<Row> = if let Some(m) = &app.pdf_metadata {
        let color_label = match &m.color_space {
            ColorSpaceInfo::PureCMYK => "Pure CMYK",
            ColorSpaceInfo::PureRGB => "Pure RGB",
            ColorSpaceInfo::Mixed => "Mixed",
            ColorSpaceInfo::CPPE => "CPPE",
            ColorSpaceInfo::Unknown => "Unknown",
        };
        let plain = Style::default();
        vec![
            Row::new(vec!["TrimBox".to_string(), fmt_box(m.trimbox)]).style(filled),
            Row::new(vec!["MediaBox".to_string(), fmt_box(Some(m.mediabox))]).style(plain),
            Row::new(vec!["BleedBox".to_string(), fmt_box(m.bleedbox)]).style(filled),
            Row::new(vec![
                "Bleed".to_string(),
                format!("{:.3} in", m.bleed_pts / 72.0),
            ])
            .style(plain),
            Row::new(vec!["Color".to_string(), color_label.to_string()]).style(filled),
            Row::new(vec!["Pages".to_string(), m.page_count.to_string()]).style(plain),
            Row::new(vec![
                "File Size".to_string(),
                format!("{} KB", m.file_size_kb),
            ])
            .style(filled),
            Row::new(vec!["Editing".to_string(), m.editing.clone()]).style(plain),
        ]
    } else {
        vec![Row::new(vec!["No metadata".to_string(), "—".to_string()])]
    };

    let table = Table::new(rows, [Constraint::Length(12), Constraint::Min(0)])
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(table, area);
}
fn draw_action_log(frame: &mut Frame, app: &App, area: Rect) {
    use crate::tui::app::LogStatus;

    let lines: Vec<String> = app
        .action_log
        .iter()
        .rev()
        .take(3)
        .map(|e| {
            let tag = match &e.status {
                LogStatus::Ok => "OK  ",
                LogStatus::Failed => "FAIL",
                LogStatus::Partial => "PART",
            };
            format!(" [{}] {}  {}", e.timestamp, tag, e.action)
        })
        .collect();

    let (content, style) = if lines.is_empty() {
        (app.idle_quip.clone(), Style::default().fg(Color::DarkGray))
    } else {
        (
            lines.join("\n"),
            Style::default().fg(AppColor::PrimaryOrange.into()),
        )
    };

    let log = Paragraph::new(content).style(style).block(
        Block::default()
            .borders(Borders::TOP)
            .title(" Last Actions "),
    );
    frame.render_widget(log, area);
}
fn draw_file_select(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());

    let title = Paragraph::new(" Enter file path (leave \"Path\" empty to select a local PDF)")
        .style(Style::default().fg(AppColor::PrimaryOrange.into()).bold())
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    let input = Paragraph::new(format!(" Path: {}", app.input_buffer))
        .block(Block::default().padding(Padding::top(1)));
    frame.render_widget(input, chunks[1]);

    let orange: Color = AppColor::PrimaryOrange.into();
    let local_items: Vec<ListItem> = if app.local_files.is_empty() {
        vec![
            ListItem::new(" No PDFs found in current directory")
                .style(Style::default().fg(Color::DarkGray)),
        ]
    } else {
        app.local_files
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                let style = if i == app.local_file_index {
                    Style::default().fg(Color::Black).bg(orange).bold()
                } else {
                    Style::default()
                };
                ListItem::new(format!(" {name}")).style(style)
            })
            .collect()
    };
    let local_list = List::new(local_items).block(
        Block::default()
            .borders(Borders::TOP)
            .title(" Local PDFs  [Tab] load all as batch ")
            .title_style(Style::default().fg(orange)),
    );
    frame.render_widget(local_list, chunks[2]);

    let hint_text = if app.input_buffer.is_empty() {
        if app.file_paths.is_empty() {
            " ↑/↓ select • Enter to confirm • Tab load all • Esc to quit"
        } else {
            " ↑/↓ select • Enter to confirm • Tab load all • Esc to go back"
        }
    } else if app.file_paths.is_empty() {
        " Enter to confirm • Esc to quit"
    } else {
        " Enter to confirm • Esc to go back"
    };
    let hint = Paragraph::new(hint_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint, chunks[3]);
}

fn draw_output_input(frame: &mut Frame, app: &App) {
    use crate::tui::app::OutputChoice;

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());

    let title = Paragraph::new(" Output Location")
        .style(Style::default().fg(AppColor::PrimaryOrange.into()).bold())
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    let same_style = if matches!(app.output_choice, OutputChoice::Same) {
        Style::default()
            .fg(Color::Black)
            .bg(AppColor::PrimaryOrange.into())
            .bold()
    } else {
        Style::default()
    };
    let new_style = if matches!(app.output_choice, OutputChoice::New) {
        Style::default()
            .fg(Color::Black)
            .bg(AppColor::PrimaryOrange.into())
            .bold()
    } else {
        Style::default()
    };

    let current = app
        .output_dir
        .as_ref()
        .map(|p| format!(" (current: {})", p.display()))
        .unwrap_or_default();

    let items = vec![
        ListItem::new(format!(" Same location (safe suffix){current}")).style(same_style),
        ListItem::new(" New location").style(new_style),
    ];
    let mut list_area = chunks[1];

    // If "New" is selected, split the content area for list + path input
    if matches!(app.output_choice, OutputChoice::New) {
        let inner = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(chunks[1]);
        list_area = inner[0];

        let path_input = Paragraph::new(format!("   Path: {}", app.input_buffer))
            .block(Block::default().padding(Padding::top(1)));
        frame.render_widget(path_input, inner[1]);
    }

    let menu = List::new(items).block(Block::default().padding(Padding::top(1)));
    frame.render_widget(menu, list_area);

    let hint = Paragraph::new(" ↑/↓ to choose • Enter to confirm • Esc to cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint, chunks[2]);
}
fn draw_param_input(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());

    let prompt = match app.selected_action {
        MenuAction::ResizeToBleed => " Bleed size (inches)",
        MenuAction::AddTrimBox => " TrimBox inset (inches)",
        MenuAction::SetMediaBox => " MediaBox size (width,height in inches)",
        MenuAction::Rotate => " Clockwise rotation (degrees)",
        MenuAction::ExportImages => " Export settings (format,dpi,quality)",
        MenuAction::ExtractPages => " Pages to extract (one-based)",
        MenuAction::SplitPages => " Panel width (inches)",
        MenuAction::StitchPages => " Spread width (inches)",
        MenuAction::RemapColors => " Remap colors (from-CMYK,to-CMYK,tolerance)",
        MenuAction::ConvertColorSpace => " Color profiles (source,destination,intent)",
        MenuAction::FlattenSpots => " Destination ICC profile (optional)",
        _ => " Parameters",
    };
    let title = Paragraph::new(prompt)
        .style(Style::default().fg(AppColor::PrimaryOrange.into()).bold())
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);
    let hint_label = match app.selected_action {
        MenuAction::ResizeToBleed => " e.g. 0.125  (1/8 inch standard bleed)",
        MenuAction::AddTrimBox => " e.g. 0.125  (1/8 inch standard bleed)",
        MenuAction::SetMediaBox => " e.g. 8.5,11",
        MenuAction::Rotate => " e.g. 90 | must be a multiple of 90",
        MenuAction::ExportImages => " e.g. jpg,150,90 | formats: jpg, png, webp, tiff",
        MenuAction::ExtractPages => " e.g. 1,3-5,8",
        MenuAction::SplitPages => " e.g. 5.83",
        MenuAction::StitchPages => " e.g. 8.5",
        MenuAction::ConvertColorSpace => " e.g. AdobeRGB1998,USWebCoatedSWOP,relative",
        MenuAction::FlattenSpots => " leave empty for the bundled default profile",
        MenuAction::RemapColors => {
            " e.g. 1.0 1.0 1.0 1.0,0.6 0.4 0.2 1.0,1 | CMYK → CMYK (tolerance)"
        }
        _ => "",
    };
    let input = Paragraph::new(format!(" > {}   {hint_label}", app.input_buffer))
        .block(Block::default().padding(Padding::top(1)));
    frame.render_widget(input, chunks[1]);

    let hint = Paragraph::new(" Enter to confirm • Esc to cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint, chunks[2]);
}
fn draw_result(frame: &mut Frame, app: &App) {
    let (title, style) = if app.last_result_ok {
        (
            " Done ",
            Style::default().fg(AppColor::PrimaryOrange.into()),
        )
    } else {
        (" Error ", Style::default().fg(Color::Red))
    };

    let text = Paragraph::new(format!("{}\n\nPress Enter to continue", app.result_message))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(style),
        );
    let area = centered_rect(50, 7, frame.area());
    frame.render_widget(text, area);
}
fn draw_processing(frame: &mut Frame, _app: &App) {
    let text = Paragraph::new("Processing...")
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" Working "));
    let area = centered_rect(40, 5, frame.area());
    frame.render_widget(text, area);
}
fn draw_help(frame: &mut Frame) {
    let mut lines = vec![
        "↑/↓    Navigate menu".to_string(),
        "Enter  Select".to_string(),
        "Esc    Back / Quit".to_string(),
    ];
    for action in MenuAction::ALL {
        if let Some(k) = action.hotkey() {
            lines.push(format!("{k}      {}", action.label()));
        }
    }
    lines.push("?      Toggle this help".to_string());
    let help_text = lines.join("\n");
    let popup = Paragraph::new(help_text).alignment(Alignment::Left).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Keyboard Reference ")
            .padding(Padding::uniform(1)),
    );
    let height = (lines.len() as u16 + 4).min(frame.area().height);
    let area = centered_rect(46, height, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(popup, area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
