pub mod cli;
pub mod process;
pub mod tui;

use clap::Parser;
use cli::{Cli, Command};
use process::{
    bleed_points, run_add_trim_box, run_convert_color_space, run_extract_pages, run_flatten_spots,
    run_image, run_info, run_outline_text, run_remap_color, run_resize, run_rotate,
    run_set_media_box, run_split_pages_with_layout, run_stitch_pages, run_trim,
};

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(command) => run_command(command),
        None => run_tui().map_err(rustybara::Error::Io),
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run_tui() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let result = (|| {
        let mut app = tui::App::new();
        while app.running {
            terminal.draw(|frame| tui::ui::draw(frame, &app))?;
            tui::events::handle_events(&mut app)?;
            app.tick();
        }
        Ok(())
    })();
    ratatui::restore();
    result
}

fn run_command(command: Command) -> rustybara::Result<()> {
    match command {
        Command::Trim { files } => run_trim(files.input, files.output, files.overwrite),
        Command::Resize {
            files,
            bleed,
            bleed_inches,
        } => run_resize(
            files.input,
            bleed_points(bleed, bleed_inches, 9.0)?,
            files.output,
            files.overwrite,
        ),
        Command::AddTrimBox {
            files,
            bleed,
            bleed_inches,
        } => run_add_trim_box(
            files.input,
            bleed_points(bleed, bleed_inches, 9.0)?,
            files.output,
            files.overwrite,
        ),
        Command::SetMediaBox {
            files,
            width_inches,
            height_inches,
        } => run_set_media_box(
            files.input,
            width_inches,
            height_inches,
            files.output,
            files.overwrite,
        ),
        Command::Rotate { files, degrees } => {
            run_rotate(files.input, degrees, files.output, files.overwrite)
        }
        Command::Image {
            files,
            format,
            dpi,
            quality,
            annotations,
            forms,
        } => run_image(
            files.input,
            files.output,
            format,
            dpi,
            quality,
            annotations,
            forms,
            files.overwrite,
        ),
        Command::ExtractPages { files, pages } => {
            run_extract_pages(files.input, &pages, files.output, files.overwrite)
        }
        Command::SplitPages {
            files,
            panel_width,
            panel_widths,
            axis,
        } => run_split_pages_with_layout(
            files.input,
            panel_width,
            &panel_widths,
            axis,
            files.output,
            files.overwrite,
        ),
        Command::StitchPages {
            files,
            spread_width,
        } => run_stitch_pages(files.input, spread_width, files.output, files.overwrite),
        Command::RemapColor {
            files,
            from,
            to,
            tolerance,
        } => run_remap_color(
            files.input,
            files.output,
            from,
            to,
            tolerance,
            files.overwrite,
        ),
        Command::ConvertColorSpace {
            files,
            from_profile,
            to_profile,
            intent,
            from_icc,
            to_icc,
        } => run_convert_color_space(
            files.input,
            files.output,
            from_profile.as_deref().unwrap_or("AdobeRGB1998"),
            to_profile.as_deref().unwrap_or("USWebCoatedSWOP"),
            intent,
            from_icc,
            to_icc,
            files.overwrite,
        ),
        Command::FlattenSpots { files, icc_profile } => {
            run_flatten_spots(files.input, files.output, icc_profile, files.overwrite)
        }
        Command::OutlineText { files } => {
            run_outline_text(files.input, files.output, files.overwrite)
        }
        Command::Info { files } => run_info(files.input),
    }
}
