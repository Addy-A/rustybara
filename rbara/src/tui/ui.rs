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
        Screen::ParamInput => draw_param_input(frame, app),
        Screen::Processing => draw_processing(frame, app),
        Screen::Result => draw_result(frame, app),
    }
}
fn draw_main(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // title
        Constraint::Min(0),    // menu
        Constraint::Length(1), // status
        Constraint::Length(1), // footer
    ])
    .split(frame.area());

    let title = Paragraph::new(" Rustybara — Prepress Toolkit")
        .style(Style::default().fg(AppColor::PrimaryOrange.into()).bold())
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // let logo = SomeRatatuiStruct

    let items: Vec<ListItem> = MenuAction::ALL
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let style = if i == app.menu_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(AppColor::PrimaryOrange.into())
                    .bold()
            } else {
                Style::default()
            };
            ListItem::new(format!(" {}", action.label())).style(style)
        })
        .collect();
    let menu = List::new(items).block(Block::default().padding(Padding::top(1)));
    frame.render_widget(menu, chunks[1]);

    let overwrite_tag = if app.overwrite { " [OVERWRITE] " } else { "" };
    let status_text = if app.file_paths.is_empty() {
        format!("No files loaded{overwrite_tag}")
    } else if app.file_paths.len() == 1 {
        format!(" {}{overwrite_tag}", app.file_paths[0].display())
    } else {
        format!(" {} files loaded{overwrite_tag}", app.file_paths.len())
    };
    let status =
        Paragraph::new(status_text).style(Style::default().fg(AppColor::PrimaryOrange.into()));
    frame.render_widget(status, chunks[2]);
    let footer_parts: Vec<String> = MenuAction::ALL
        .iter()
        .filter_map(|a| a.hotkey().map(|k| format!("[{k}]{}", &a.label()[1..])))
        .collect();
    let footer_text = format!(" {} [?]help", footer_parts.join(" "));
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[3]);
}
fn draw_file_select(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());

    let title = Paragraph::new(" Enter file path")
        .style(Style::default().fg(AppColor::PrimaryOrange.into()).bold())
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    let input = Paragraph::new(format!(" Path: {}", app.input_buffer))
        .block(Block::default().padding(Padding::top(1)));
    frame.render_widget(input, chunks[1]);

    let hint_text = if app.file_paths.is_empty() {
        " Enter to confirm • Esc to quit"
    } else {
        " Enter to confirm • Esc to go back"
    };
    let hint = Paragraph::new(hint_text).style(Style::default().fg(Color::DarkGray));
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
        MenuAction::ResizeToBleed => " Bleed size (points)",
        MenuAction::ExportImages => " Export settings (format,dpi)",
        MenuAction::RemapColors => " Remap colors (C M Y K)",
        _ => " Parameters",
    };
    let title = Paragraph::new(prompt)
        .style(Style::default().fg(AppColor::PrimaryOrange.into()).bold())
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);
    let hint_label = match app.selected_action {
        MenuAction::ResizeToBleed => " e.g. 9.0",
        MenuAction::ExportImages => " e.g. jpg,150 | formats: jpg, png, webp, tiff",
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
    let area = centered_rect(36, 16, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(popup, area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
