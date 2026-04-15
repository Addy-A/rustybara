pub mod cli;
pub mod process;
pub mod tui;

use clap::Parser;
use cli::{Cli, Command};
use process::{run_image, run_remap_color, run_resize, run_trim};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(command) => run_command(command),
        None => {
            let mut terminal = ratatui::init();
            let mut app = tui::App::new();

            while app.running {
                terminal.draw(|frame| tui::ui::draw(frame, &app)).unwrap();
                tui::events::handle_events(&mut app).unwrap();
                app.tick();
            }

            ratatui::restore();
        }
    }
}

fn run_command(command: Command) {
    let result = match command {
        Command::Trim {
            input,
            output,
            overwrite,
        } => run_trim(input, output, overwrite),
        Command::Resize {
            input,
            bleed,
            output,
            overwrite,
        } => run_resize(input, bleed, output, overwrite),
        Command::Image {
            input,
            output,
            format,
            dpi,
            overwrite,
        } => run_image(input, output, format, dpi, overwrite),
        Command::ColorRemap {
            input,
            output,
            from,
            to,
            tolerance,
            overwrite,
        } => run_remap_color(input, output, from, to, tolerance, overwrite),
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
