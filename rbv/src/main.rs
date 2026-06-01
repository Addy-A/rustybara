pub mod box_overlay_style;
pub mod export;
pub mod renderer;
pub mod separation;
pub mod tiles;
pub mod ui_state;
pub mod viewer;

use clap::Parser;
use rustybara::raster::RenderConfig;

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn defaults() {
        let args = Args::parse_from(["rbv", "doc.pdf"]);
        assert_eq!(args.page, 0);
        assert_eq!(args.dpi, 300);
        assert_eq!(args.file.to_str().unwrap(), "doc.pdf");
    }

    #[test]
    fn explicit_page_and_dpi() {
        let args = Args::parse_from(["rbv", "doc.pdf", "3", "--dpi", "300"]);
        assert_eq!(args.page, 3);
        assert_eq!(args.dpi, 300);
    }

    #[test]
    fn missing_file_fails() {
        assert!(Args::try_parse_from(["rbv"]).is_err());
    }

    #[test]
    fn invalid_dpi_fails() {
        assert!(Args::try_parse_from(["rbv", "doc.pdf", "0", "--dpi", "notanumber"]).is_err());
    }

    #[test]
    fn listen_flag_defaults_false() {
        let args = Args::parse_from(["rbv", "doc.pdf"]);
        assert!(!args.listen);
    }

    #[test]
    fn listen_flag_explicit() {
        let args = Args::parse_from(["rbv", "doc.pdf", "--listen"]);
        assert!(args.listen);
    }
}

#[derive(clap::Parser)]
#[command(name = "rbv")]
struct Args {
    file: std::path::PathBuf,
    #[arg(default_value_t = 0)]
    page: u32,
    #[arg(long, default_value_t = 300)]
    dpi: u32,
    #[arg(long, default_value_t = false)]
    listen: bool,
}

fn main() {
    let args = Args::parse();
    let config = RenderConfig {
        dpi: args.dpi,
        render_annotations: true,
        render_form_data: false,
    };
    viewer::run(args.file, args.page, config, args.listen);
}
