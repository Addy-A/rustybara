use crate::tui::app::{ActionLogEntry, LogStatus};
use crate::tui::app::{App, ColorSpaceInfo, MenuAction};
use chrono;
use core::f64;
use rustybara::PdfPipeline;
use rustybara::pages::PageBoxes;
use std::collections::HashMap;
use std::fs;
use std::io;
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

fn collision_key(path: &Path) -> String {
    let key = path.to_string_lossy();
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key.into_owned()
    }
}

fn plan_output_paths(
    input: &[PathBuf],
    output_dir: &Option<PathBuf>,
    new_ext: Option<&str>,
    overwrite: bool,
) -> rustybara::Result<Vec<PathBuf>> {
    let outputs: Vec<PathBuf> = input
        .iter()
        .map(|path| output_path(path, output_dir, new_ext, overwrite))
        .collect();
    let mut seen: HashMap<String, &Path> = HashMap::new();

    for (source, output) in input.iter().zip(&outputs) {
        if let Some(previous) = seen.insert(collision_key(output), source) {
            return Err(rustybara::Error::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "output collision: '{}' and '{}' both map to '{}'",
                    previous.display(),
                    source.display(),
                    output.display()
                ),
            )));
        }
    }

    Ok(outputs)
}

pub fn run_trim(
    input: Vec<PathBuf>,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    let outputs = plan_output_paths(&input, &output, None, overwrite)?;
    for (path, out) in input.iter().zip(outputs) {
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
    let outputs = plan_output_paths(&input, &output, None, overwrite)?;
    for (path, out) in input.iter().zip(outputs) {
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
    let outputs = plan_output_paths(&input, &output, None, overwrite)?;
    for (path, out) in input.iter().zip(outputs) {
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

    if overwrite {
        return Err(rustybara::Error::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "image export cannot overwrite a PDF source; choose an output directory instead",
        )));
    }

    let base_outputs = plan_output_paths(&input, &output, Some(fmt.extension()), false)?;
    for (path, base_output) in input.iter().zip(base_outputs) {
        let pipeline = PdfPipeline::open(path)?;
        for page in 0..pipeline.page_count() as u32 {
            let out = if pipeline.page_count() > 1 {
                let stem = base_output
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy();
                base_output.with_file_name(format!("{}_{}.{}", stem, page + 1, fmt.extension()))
            } else {
                base_output.clone()
            };
            pipeline.save_page_image(page, &out, &fmt, &config, 90)?;
            print!("{} page {} → {}", path.display(), page + 1, out.display());
        }
    }
    Ok(())
}

pub fn load_local_files(path: &Path) -> Result<Vec<PathBuf>, io::Error> {
    let paths = fs::read_dir(path)?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()? == "pdf" {
                Some(Ok(path))
            } else {
                None
            }
        })
        .collect::<Result<Vec<PathBuf>, io::Error>>()?;
    Ok(paths)
}

pub fn load_metadata(path: &Path) -> rustybara::Result<crate::tui::app::PdfMetadata> {
    use crate::tui::app::PdfMetadata;
    use rustybara::PdfPipeline;

    let pipeline = PdfPipeline::open(path)?;
    let doc = pipeline.doc();
    let pages = doc.get_pages();

    let first_id = match pages.values().next() {
        Some(&id) => id,
        None => {
            return Err(rustybara::Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "PDF has no pages",
            )));
        }
    };
    let boxes = PageBoxes::read(doc, first_id)?;

    let rect_to_arr = |r: &rustybara::geometry::Rect| -> [f32; 4] {
        [r.x as f32, r.y as f32, r.right() as f32, r.top() as f32]
    };

    let trimbox = boxes.trim_box.as_ref().map(rect_to_arr);
    let mediabox = rect_to_arr(&boxes.media_box);
    let bleedbox = boxes.bleed_box.as_ref().map(rect_to_arr);

    let bleed_pts = match &boxes.trim_box {
        Some(trim) => (trim.x - boxes.media_box.x).abs() as f32,
        None => 0.0,
    };

    let color_space = match rustybara::PdfPipeline::detect_color_space(pipeline.doc()) {
        rustybara::DocumentColorKind::PureCMYK => ColorSpaceInfo::PureCMYK,
        rustybara::DocumentColorKind::PureRGB => ColorSpaceInfo::PureRGB,
        rustybara::DocumentColorKind::Mixed => ColorSpaceInfo::Mixed,
        rustybara::DocumentColorKind::Unknown => ColorSpaceInfo::Unknown,
    };

    let file_size_kb = std::fs::metadata(path).map(|m| m.len() / 1024).unwrap_or(0);

    Ok(PdfMetadata {
        trimbox,
        mediabox,
        bleedbox,
        bleed_pts,
        color_space,
        page_count: pipeline.page_count() as u32,
        file_size_kb,
        editing: String::new(),
    })
}

pub fn run_tui_action(app: &App) -> rustybara::Result<(String, Vec<PathBuf>, ActionLogEntry)> {
    let input: Vec<PathBuf> = app.file_paths.to_vec();
    let count = input.len();
    let overwrite = app.overwrite;
    let output_dir = &app.output_dir;
    let mut action_entry = ActionLogEntry {
        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        action: String::new(),
        status: LogStatus::Ok,
    };

    match app.selected_action {
        MenuAction::TrimMarks => {
            let out_paths = plan_output_paths(&input, output_dir, None, overwrite)?;
            for (path, out) in input.iter().zip(&out_paths) {
                PdfPipeline::open(path)?.trim()?.save_pdf(out)?;
            }
            action_entry.action = "TrimMarks".to_string();
            Ok((format!("Trimmed {count} file(s)"), out_paths, action_entry))
        }
        MenuAction::ResizeToBleed => {
            let out_paths = plan_output_paths(&input, output_dir, None, overwrite)?;
            for (path, out) in input.iter().zip(&out_paths) {
                PdfPipeline::open(path)?
                    .resize(app.params.bleed_pts)?
                    .save_pdf(out)?;
            }
            action_entry.action = format!("ResizeToBleed ({})", app.params.bleed_pts);
            let bleed_inch = app.params.bleed_pts / 72.0;
            Ok((
                format!("Resized {count} file(s) (bleed: {}inch)", bleed_inch),
                out_paths,
                action_entry,
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
            let base_outputs = plan_output_paths(&input, output_dir, Some(fmt.extension()), false)?;
            let mut total = 0u32;
            for (path, base_output) in input.iter().zip(base_outputs) {
                let pipeline = PdfPipeline::open(path)?;
                for page in 0..pipeline.page_count() as u32 {
                    let out = if pipeline.page_count() > 1 {
                        let stem = base_output
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy();
                        base_output.with_file_name(format!(
                            "{}_{}.{}",
                            stem,
                            page + 1,
                            fmt.extension()
                        ))
                    } else {
                        base_output.clone()
                    };
                    pipeline.save_page_image(page, &out, &fmt, &config, 90)?;
                    total += 1;
                }
            }
            action_entry.action = format!("ExportImages ({})", app.params.export_format);
            Ok((
                format!(
                    "Exported {total} image(s) ({}, {}dpi)",
                    app.params.export_format, app.params.export_dpi
                ),
                Vec::new(),
                action_entry,
            ))
        }
        MenuAction::RemapColors => {
            let out_paths = plan_output_paths(&input, output_dir, None, overwrite)?;
            for (path, out) in input.iter().zip(&out_paths) {
                PdfPipeline::open(path)?
                    .remap_color(
                        app.params.remap_from,
                        app.params.remap_to,
                        app.params.remap_tolerance,
                    )?
                    .save_pdf(out)?;
            }
            action_entry.action = "RemapColors".to_string();
            Ok((format!("Remapped {count} file(s)"), out_paths, action_entry))
        }
        MenuAction::PreviewPage => {
            action_entry.action = "PreviewPage".to_string();
            Ok((
                "Preview not yet implemented".into(),
                Vec::new(),
                action_entry,
            ))
        }
        _ => Ok(("Unknown action".into(), Vec::new(), action_entry)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_overwrite_is_rejected_before_opening_the_source() {
        let result = run_image(
            vec![PathBuf::from("single-page.pdf")],
            None,
            Some("png".to_string()),
            150,
            true,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot overwrite"));
    }

    #[test]
    fn common_output_directory_rejects_same_stem_collisions() {
        let inputs = vec![
            PathBuf::from("first/report.pdf"),
            PathBuf::from("second/report.pdf"),
        ];
        let result = plan_output_paths(&inputs, &Some(PathBuf::from("output")), None, false);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("output collision"));
    }
}
