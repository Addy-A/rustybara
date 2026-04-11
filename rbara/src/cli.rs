use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "rustybara",
    version,
    about = "Prepress PDF manipulation toolkit"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    Trim {
        #[arg(required = true)]
        input: Vec<PathBuf>,

        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Resize {
        #[arg(required = true)]
        input: Vec<PathBuf>,

        #[arg(short, long)]
        output: Option<PathBuf>,

        #[arg(long, default_value_t = 9.0)]
        bleed: f64,
    },
    Image {
        #[arg(required = true)]
        input: Vec<PathBuf>,

        #[arg(short, long)]
        output: Option<PathBuf>,

        #[arg(long, default_value = "jpg")]
        format: Option<String>,

        #[arg(long, default_value_t = 150)]
        dpi: u32,
    },
}
