pub mod cli;
pub mod process;
pub mod tui;

use clap::Parser;
use cli::{Cli, Command};
use process::{output_path, run_image, run_resize, run_trim};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(command) => run_command(command),
        None => {
            println!("TUI not implemented yet");
        }
    }
}

fn run_command(command: Command) {
    let result = match command {
        Command::Trim { input, output } => run_trim(input, output),
        Command::Resize {
            input,
            bleed,
            output,
        } => run_resize(input, bleed, output),
        Command::Image {
            input,
            output,
            format,
            dpi,
        } => run_image(input, output, format, dpi),
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
