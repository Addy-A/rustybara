use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "rustybara",
    version,
    about = "Prepress PDF manipulation toolkit",
    arg_required_else_help = false
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Args, Debug)]
pub struct PdfFiles {
    /// One or more PDF files to process.
    #[arg(required = true, value_name = "PDF")]
    pub input: Vec<PathBuf>,

    /// Directory where processed files will be written.
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<PathBuf>,

    /// Replace each source PDF instead of creating a suffixed copy.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Args, Debug)]
pub struct InputFiles {
    /// One or more PDF files to inspect.
    #[arg(required = true, value_name = "PDF")]
    pub input: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ImageFormat {
    Jpg,
    Png,
    Webp,
    Tiff,
}

impl ImageFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jpg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Tiff => "tiff",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum RenderingIntent {
    Perceptual,
    RelativeColorimetric,
    Saturation,
    AbsoluteColorimetric,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum PanelAxis {
    Horizontal,
    Vertical,
}

impl RenderingIntent {
    pub fn as_pipeline_str(self) -> &'static str {
        match self {
            Self::Perceptual => "Perceptual",
            Self::RelativeColorimetric => "RelativeColorimetric",
            Self::Saturation => "Saturation",
            Self::AbsoluteColorimetric => "AbsoluteColorimetric",
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Remove trim marks and crop to the TrimBox.
    Trim {
        #[command(flatten)]
        files: PdfFiles,
    },

    /// Resize page content to create bleed around the TrimBox.
    Resize {
        #[command(flatten)]
        files: PdfFiles,

        /// Bleed in PDF points. Retained for compatibility (72 points = 1 inch).
        #[arg(short, long, value_name = "POINTS", conflicts_with = "bleed_inches")]
        bleed: Option<f64>,

        /// Bleed in inches.
        #[arg(long, value_name = "INCHES")]
        bleed_inches: Option<f64>,
    },

    /// Add a TrimBox inset from the MediaBox.
    AddTrimBox {
        #[command(flatten)]
        files: PdfFiles,

        /// Inset in PDF points (72 points = 1 inch).
        #[arg(long, value_name = "POINTS", conflicts_with = "bleed_inches")]
        bleed: Option<f64>,

        /// Inset in inches (default: 0.125).
        #[arg(long, value_name = "INCHES")]
        bleed_inches: Option<f64>,
    },

    /// Set every page's MediaBox dimensions.
    SetMediaBox {
        #[command(flatten)]
        files: PdfFiles,

        /// Page width in inches.
        #[arg(long, default_value_t = 8.5, value_name = "INCHES")]
        width_inches: f64,

        /// Page height in inches.
        #[arg(long, default_value_t = 11.0, value_name = "INCHES")]
        height_inches: f64,
    },

    /// Rotate every page clockwise.
    Rotate {
        #[command(flatten)]
        files: PdfFiles,

        /// Clockwise rotation in degrees; must be a multiple of 90.
        #[arg(short, long, default_value_t = 90)]
        degrees: i32,
    },

    /// Export PDF pages as raster images.
    Image {
        #[command(flatten)]
        files: PdfFiles,

        #[arg(long, value_enum, default_value_t = ImageFormat::Jpg)]
        format: ImageFormat,

        #[arg(long, default_value_t = 150)]
        dpi: u32,

        /// JPEG/WebP encoder quality from 1 through 100.
        #[arg(long, default_value_t = 90)]
        quality: u8,

        /// Render PDF annotations into exported pages.
        #[arg(long)]
        annotations: bool,

        /// Render interactive form values into exported pages.
        #[arg(long)]
        forms: bool,
    },

    /// Extract a page selection into a new PDF.
    ExtractPages {
        #[command(flatten)]
        files: PdfFiles,

        /// One-based page list and ranges, for example: 1,3-5,8.
        #[arg(long, value_name = "PAGES")]
        pages: String,
    },

    /// Split wide pages into panels.
    SplitPages {
        #[command(flatten)]
        files: PdfFiles,

        /// Target panel width in inches.
        #[arg(long, default_value_t = 5.83, value_name = "INCHES")]
        panel_width: f64,

        /// Explicit ordered panel sizes in inches, for example 3.625,3.6875,3.6875.
        /// When supplied this replaces the uniform panel width.
        #[arg(long, value_delimiter = ',', value_name = "INCHES")]
        panel_widths: Vec<f64>,

        /// Direction in which panels advance across the page.
        #[arg(
            long,
            value_enum,
            default_value_t = PanelAxis::Horizontal,
            requires = "panel_widths"
        )]
        axis: PanelAxis,
    },

    /// Stitch adjacent pages into spreads.
    StitchPages {
        #[command(flatten)]
        files: PdfFiles,

        /// Target spread width in inches.
        #[arg(long, default_value_t = 8.5, value_name = "INCHES")]
        spread_width: f64,
    },

    /// Remap one CMYK color to another.
    #[command(name = "remap-color", visible_alias = "color-remap")]
    RemapColor {
        #[command(flatten)]
        files: PdfFiles,

        #[arg(long, num_args = 4, value_names = ["C", "M", "Y", "K"])]
        from: Vec<f64>,

        #[arg(long, num_args = 4, value_names = ["C", "M", "Y", "K"])]
        to: Vec<f64>,

        #[arg(long, default_value_t = 1.0)]
        tolerance: f64,
    },

    /// Convert document colors using named profiles or ICC files.
    ConvertColorSpace {
        #[command(flatten)]
        files: PdfFiles,

        /// Built-in source profile name (default: AdobeRGB1998).
        #[arg(long, conflicts_with_all = ["from_icc", "to_icc"])]
        from_profile: Option<String>,

        /// Built-in destination profile name (default: USWebCoatedSWOP).
        #[arg(long, conflicts_with_all = ["from_icc", "to_icc"])]
        to_profile: Option<String>,

        /// Rendering intent.
        #[arg(long, value_enum, default_value_t = RenderingIntent::RelativeColorimetric)]
        intent: RenderingIntent,

        /// Source ICC profile file; requires --to-icc.
        #[arg(long, value_name = "FILE", requires = "to_icc")]
        from_icc: Option<PathBuf>,

        /// Destination ICC profile file; requires --from-icc.
        #[arg(long, value_name = "FILE", requires = "from_icc")]
        to_icc: Option<PathBuf>,
    },

    /// Flatten spot colors, optionally through a destination ICC profile.
    FlattenSpots {
        #[command(flatten)]
        files: PdfFiles,

        #[arg(long, value_name = "FILE")]
        icc_profile: Option<PathBuf>,
    },

    /// Convert text into vector outlines.
    OutlineText {
        #[command(flatten)]
        files: PdfFiles,
    },

    /// Print page, box, color-space, and Rustybara metadata information.
    Info {
        #[command(flatten)]
        files: InputFiles,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn accepts_legacy_color_remap_name() {
        let cli = Cli::try_parse_from([
            "rustybara",
            "color-remap",
            "sample.pdf",
            "--from",
            "0",
            "0",
            "0",
            "1",
            "--to",
            "0",
            "0",
            "0",
            "0",
        ])
        .unwrap();

        assert!(matches!(cli.command, Some(Command::RemapColor { .. })));
    }

    #[test]
    fn rejects_incompatible_bleed_units() {
        let result = Cli::try_parse_from([
            "rustybara",
            "resize",
            "sample.pdf",
            "--bleed",
            "9",
            "--bleed-inches",
            "0.125",
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn parses_explicit_vertical_panel_layout() {
        let cli = Cli::try_parse_from([
            "rustybara",
            "split-pages",
            "sample.pdf",
            "--panel-widths",
            "3.625,3.6875,3.6875",
            "--axis",
            "vertical",
            "--overwrite",
        ])
        .unwrap();

        let Some(Command::SplitPages {
            panel_widths,
            axis,
            files,
            ..
        }) = cli.command
        else {
            panic!("split-pages command was not parsed");
        };
        assert_eq!(panel_widths, vec![3.625, 3.6875, 3.6875]);
        assert_eq!(axis, PanelAxis::Vertical);
        assert!(files.overwrite);
    }

    #[test]
    fn rejects_axis_without_explicit_panel_layout() {
        let result = Cli::try_parse_from([
            "rustybara",
            "split-pages",
            "sample.pdf",
            "--axis",
            "vertical",
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn keeps_legacy_uniform_panel_layout() {
        let cli = Cli::try_parse_from([
            "rustybara",
            "split-pages",
            "sample.pdf",
            "--panel-width",
            "5.83",
        ])
        .unwrap();

        let Some(Command::SplitPages {
            panel_width,
            panel_widths,
            axis,
            ..
        }) = cli.command
        else {
            panic!("split-pages command was not parsed");
        };
        assert_eq!(panel_width, 5.83);
        assert!(panel_widths.is_empty());
        assert_eq!(axis, PanelAxis::Horizontal);
    }
}
