use crate::tui::App;
use crate::tui::app::MenuAction;
use rustybara::PdfPipeline;
use std::path::{Path, PathBuf};

pub fn output_path(
    input: &Path,
    output_dir: &Option<PathBuf>,
    new_ext: Option<&str>,
    overwrite: bool,
) -> PathBuf {
    if overwrite {
        return input.to_path_buf();
    }
    let dir = output_dir
        .as_deref()
        .unwrap_or_else(|| match input.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        });
    let stem = input.file_stem().unwrap_or_default();
    let ext =
        new_ext.unwrap_or_else(|| input.extension().and_then(|e| e.to_str()).unwrap_or("pdf"));
    dir.join(format!("{}_processed.{}", (stem).to_string_lossy(), ext))
}

pub fn run_trim(
    input: Vec<PathBuf>,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    for path in &input {
        let out = output_path(path, &output, None, overwrite);
        PdfPipeline::open(path)?.trim()?.save_pdf(&out)?;
        println!("{} → {}", path.display(), out.display());
    }
    Ok(())
}

pub fn run_resize(
    input: Vec<PathBuf>,
    bleed: f64,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    for path in &input {
        let out = output_path(path, &output, None, overwrite);
        PdfPipeline::open(path)?.resize(bleed)?.save_pdf(&out)?;
        println!("{} → {}", path.display(), out.display());
    }
    Ok(())
}

pub fn run_remap_color(
    input: Vec<PathBuf>,
    output: Option<PathBuf>,
    from_vec: Vec<f64>,
    to_vec: Vec<f64>,
    tolerance: f64,
    overwrite: bool,
) -> rustybara::Result<()> {
    let from: [f64; 4] = from_vec
        .try_into()
        .expect("--from requires exactly 4 values");
    let to: [f64; 4] = to_vec.try_into().expect("--to requires exactly 4 values");
    for path in &input {
        let out = output_path(path, &output, None, overwrite);
        PdfPipeline::open(path)?
            .remap_color(from, to, tolerance)?
            .save_pdf(&out)?;
        println!("{} → {}", path.display(), out.display());
    }
    Ok(())
}

pub fn run_image(
    input: Vec<PathBuf>,
    output: Option<PathBuf>,
    format: Option<String>,
    dpi: u32,
    overwrite: bool,
) -> rustybara::Result<()> {
    use rustybara::encode::OutputFormat;
    use rustybara::raster::RenderConfig;

    let fmt = match format.as_deref() {
        Some("png") => OutputFormat::Png,
        Some("jpg") => OutputFormat::Jpg,
        Some("webp") => OutputFormat::WebP,
        Some("tiff") => OutputFormat::Tiff,
        _ => OutputFormat::Jpg,
    };
    let config = RenderConfig {
        dpi,
        render_annotations: false,
        render_form_data: false,
    };

    for path in &input {
        let pipeline = PdfPipeline::open(path)?;
        for page in 0..pipeline.page_count() as u32 {
            let out = output_path(path, &output, Some(fmt.extension()), overwrite);
            let out = if pipeline.page_count() > 1 {
                let stem = out.file_stem().unwrap_or_default().to_string_lossy();
                out.with_file_name(format!("{}_{}.{}", stem, page + 1, fmt.extension()))
            } else {
                out
            };
            pipeline.save_page_image(page, &out, &fmt, &config)?;
            print!("{} page {} → {}", path.display(), page + 1, out.display());
        }
    }
    Ok(())
}

pub fn run_tui_action(app: &App) -> rustybara::Result<(String, Vec<PathBuf>)> {
    let input: Vec<PathBuf> = app.file_paths.to_vec();
    let count = input.len();
    let overwrite = app.overwrite;
    let output_dir = &app.output_dir;

    match app.selected_action {
        MenuAction::TrimMarks => {
            let mut out_paths = Vec::new();
            for path in &input {
                let out = if overwrite {
                    path.clone()
                } else {
                    output_path(path, output_dir, None, false)
                };
                PdfPipeline::open(path)?.trim()?.save_pdf(&out)?;
                out_paths.push(out);
            }
            Ok((format!("Trimmed {count} file(s)"), out_paths))
        }
        MenuAction::ResizeToBleed => {
            let mut out_paths = Vec::new();
            for path in &input {
                let out = if overwrite {
                    path.clone()
                } else {
                    output_path(path, output_dir, None, false)
                };
                PdfPipeline::open(path)?
                    .resize(app.params.bleed_pts)?
                    .save_pdf(&out)?;
                out_paths.push(out);
            }
            Ok((
                format!(
                    "Resized {count} file(s) (bleed: {}pt)",
                    app.params.bleed_pts
                ),
                out_paths,
            ))
        }
        MenuAction::ExportImages => {
            use rustybara::encode::OutputFormat;
            use rustybara::raster::RenderConfig;

            let fmt = match app.params.export_format.as_str() {
                "png" => OutputFormat::Png,
                "jpg" => OutputFormat::Jpg,
                "tiff" => OutputFormat::Tiff,
                "webp" => OutputFormat::WebP,
                _ => OutputFormat::Jpg,
            };
            let config = RenderConfig {
                dpi: app.params.export_dpi,
                render_annotations: false,
                render_form_data: false,
            };
            let mut total = 0u32;
            for path in &input {
                let pipeline = PdfPipeline::open(path)?;
                for page in 0..pipeline.page_count() as u32 {
                    let out = output_path(path, output_dir, Some(fmt.extension()), false);
                    let out = if pipeline.page_count() > 1 {
                        let stem = out.file_stem().unwrap_or_default().to_string_lossy();
                        out.with_file_name(format!("{}_{}.{}", stem, page + 1, fmt.extension()))
                    } else {
                        out
                    };
                    pipeline.save_page_image(page, &out, &fmt, &config)?;
                    total += 1;
                }
            }
            Ok((
                format!(
                    "Exported {total} image(s) ({}, {}dpi)",
                    app.params.export_format, app.params.export_dpi
                ),
                Vec::new(),
            ))
        }
        MenuAction::RemapColors => {
            let mut out_paths = Vec::new();
            for path in &input {
                let out = if overwrite {
                    path.clone()
                } else {
                    output_path(path, output_dir, None, false)
                };
                PdfPipeline::open(path)?
                    .remap_color(
                        app.params.remap_from,
                        app.params.remap_to,
                        app.params.remap_tolerance,
                    )?
                    .save_pdf(&out)?;
                out_paths.push(out);
            }
            Ok((format!("Remapped {count} file(s)"), out_paths))
        }
        MenuAction::PreviewPage => Ok(("Preview not yet implemented".into(), Vec::new())),
        _ => Ok(("Unknown action".into(), Vec::new())),
    }
}
