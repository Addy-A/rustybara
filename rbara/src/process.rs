use crate::tui::App;
use rustybara::PdfPipeline;
use std::path::{Path, PathBuf};

pub fn output_path(input: &Path, output_dir: &Option<PathBuf>, new_ext: Option<&str>) -> PathBuf {
    let dir = output_dir
        .as_deref()
        .unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")));
    let stem = input.file_stem().unwrap_or_default();
    let ext =
        new_ext.unwrap_or_else(|| input.extension().and_then(|e| e.to_str()).unwrap_or("pdf"));
    dir.join(format!("{}_out.{}", (stem).to_string_lossy(), ext))
}

pub fn run_trim(input: Vec<PathBuf>, output: Option<PathBuf>) -> rustybara::Result<()> {
    for path in &input {
        let out = output_path(path, &output, None);
        PdfPipeline::open(path)?.trim()?.save_pdf(&out)?;
        println!("{} → {}", path.display(), out.display());
    }
    Ok(())
}

pub fn run_resize(
    input: Vec<PathBuf>,
    bleed: f64,
    output: Option<PathBuf>,
) -> rustybara::Result<()> {
    for path in &input {
        let out = output_path(path, &output, None);
        PdfPipeline::open(path)?.resize(bleed)?.save_pdf(&out)?;
        println!("{} → {}", path.display(), out.display());
    }
    Ok(())
}

pub fn run_image(
    input: Vec<PathBuf>,
    output: Option<PathBuf>,
    format: Option<String>,
    dpi: u32,
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
            let out = output_path(path, &output, Some(fmt.extension()));
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

    match app.selected_action {
        0 => {
            let mut out_paths = Vec::new();
            for path in &input {
                let out = if overwrite {
                    path.clone()
                } else {
                    output_path(path, &None, None)
                };
                PdfPipeline::open(path)?.trim()?.save_pdf(&out)?;
                out_paths.push(out);
            }
            Ok((format!("Trimmed {count} file(s)"), out_paths))
        }
        1 => {
            let mut out_paths = Vec::new();
            for path in &input {
                let out = if overwrite {
                    path.clone()
                } else {
                    output_path(path, &None, None)
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
        2 => {
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
                    let out = output_path(path, &None, Some(fmt.extension()));
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
        3 => Ok(("Preview not yet implemented".into(), Vec::new())),
        _ => Ok(("Unknown action".into(), Vec::new())),
    }
}
