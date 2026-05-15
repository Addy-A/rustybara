use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rustybara::encode::OutputFormat;
use rustybara::pages::PageBoxes;
use rustybara::raster::RenderConfig;
use rustybara::PdfPipeline;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

pub struct ProcessingLock(pub Mutex<bool>);

#[derive(serde::Serialize)]
pub struct ActionResult {
    pub ok: bool,
    pub message: String,
    pub output_paths: Vec<String>,
    pub timestamp: String,
}

#[derive(serde::Serialize)]
pub struct PdfMetadataDto {
    pub trimbox: Option<[f32; 4]>,
    pub mediabox: [f32; 4],
    pub bleedbox: Option<[f32; 4]>,
    pub bleed_pts: f32,
    pub bleed_inches: f32,
    pub color_space: String,
    pub page_count: u32,
    pub file_size_kb: u64,
    pub has_trimbox: bool,
    pub has_bleedbox: bool,
}

fn output_path(
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
    dir.join(format!("{}_processed.{}", stem.to_string_lossy(), ext))
}

fn friendly_error(e: rustybara::Error) -> String {
    match &e {
        rustybara::Error::Io(ioe) => match ioe.kind() {
            std::io::ErrorKind::NotFound => format!("File not found: {e}"),
            std::io::ErrorKind::PermissionDenied => format!("Permission denied: {e}"),
            _ => format!("I/O error: {e}"),
        },
        rustybara::Error::Render(_) => format!(
            "Render failed - Pdfium library not found or failed to initialize.\n\
            Place pdfium.dll (or MAC OS: libpdfium.dylib) in the executable directory.\n\
            Details: {e}"
        ),
        rustybara::Error::Pdf(_) => format!(
            "Failed to parse PDF — the file may be corrupted or password-protected.\n\n\
             Details: {e}"
        ),
        rustybara::Error::Image(_) => format!("Image encoding failed: {e}"),
        rustybara::Error::Color(_) => format!("Color conversion failed: {e}"),
    }
}

fn now_timestamp() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

/// RAII guard that releases the processing lock on drop, even on early returns.
struct LockGuard<'a> {
    lock: &'a Mutex<bool>,
}

impl<'a> LockGuard<'a> {
    fn acquire(lock: &'a Mutex<bool>) -> Result<Self, String> {
        let mut guard = lock
            .lock()
            .map_err(|_| "Processing lock poisoned".to_string())?;
        if *guard {
            return Err("A file is already being processed".to_string());
        }
        *guard = true;
        Ok(Self { lock })
    }
}

impl Drop for LockGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.lock.lock() {
            *guard = false;
        }
    }
}

#[tauri::command]
pub fn trim_marks(
    paths: Vec<String>,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let _guard = LockGuard::acquire(&state.0)?;
    let output_dir = output_dir.map(PathBuf::from);
    let mut output_paths = Vec::new();

    for path_str in &paths {
        let path = PathBuf::from(path_str);
        let out = output_path(&path, &output_dir, None, overwrite);
        PdfPipeline::open(&path)
            .and_then(|mut p| {
                p.trim()?;
                p.save_pdf(&out)?;
                Ok(())
            })
            .map_err(friendly_error)?;
        output_paths.push(out.to_string_lossy().into_owned());
    }

    Ok(ActionResult {
        ok: true,
        message: format!("Trimmed {} file(s)", paths.len()),
        output_paths,
        timestamp: now_timestamp(),
    })
}

#[tauri::command]
pub fn resize_to_bleed(
    paths: Vec<String>,
    bleed_inches: f64,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let _guard = LockGuard::acquire(&state.0)?;
    let output_dir = output_dir.map(PathBuf::from);
    let bleed_pts = bleed_inches * 72.0;
    let mut output_paths = Vec::new();

    for path_str in &paths {
        let path = PathBuf::from(path_str);
        let out = output_path(&path, &output_dir, None, overwrite);
        PdfPipeline::open(&path)
            .and_then(|mut p| {
                p.resize(bleed_pts)?;
                p.save_pdf(&out)?;
                Ok(())
            })
            .map_err(friendly_error)?;
        output_paths.push(out.to_string_lossy().into_owned());
    }

    Ok(ActionResult {
        ok: true,
        message: format!(
            "Resized {} file(s) (bleed: {} in)",
            paths.len(),
            bleed_inches
        ),
        output_paths,
        timestamp: now_timestamp(),
    })
}

#[tauri::command]
pub async fn export_images(
    paths: Vec<String>,
    format: String,
    dpi: u32,
    output_dir: Option<String>,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    // Acquire lock synchronously before spawning the blocking render task.
    {
        let mut guard = state
            .0
            .lock()
            .map_err(|_| "Processing lock poisoned".to_string())?;
        if *guard {
            return Err("A file is already being processed".to_string());
        }
        *guard = true;
    }

    let fmt = match format.as_str() {
        "png" => OutputFormat::Png,
        "webp" => OutputFormat::WebP,
        "tiff" => OutputFormat::Tiff,
        _ => OutputFormat::Jpg,
    };
    let config = RenderConfig {
        dpi,
        render_annotations: false,
        render_form_data: false,
    };
    let output_dir = output_dir.map(PathBuf::from);
    let format_label = format.clone();

    // Run the slow pdfium render on a blocking thread so the UI stays responsive.
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut output_paths = Vec::new();
        let mut total_images = 0u32;

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let pipeline = PdfPipeline::open(&path).map_err(friendly_error)?;
            let page_count = pipeline.page_count() as u32;

            for page in 0..page_count {
                let base = output_path(&path, &output_dir, Some(fmt.extension()), false);
                let out = if page_count > 1 {
                    let stem = base.file_stem().unwrap_or_default().to_string_lossy();
                    base.with_file_name(format!("{}_{}.{}", stem, page + 1, fmt.extension()))
                } else {
                    base
                };
                pipeline
                    .save_page_image(page, &out, &fmt, &config)
                    .map_err(friendly_error)?;
                output_paths.push(out.to_string_lossy().into_owned());
                total_images += 1;
            }
        }

        Ok::<ActionResult, String>(ActionResult {
            ok: true,
            message: format!(
                "Exported {} image(s) ({}, {}dpi)",
                total_images, format_label, dpi
            ),
            output_paths,
            timestamp: now_timestamp(),
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    // Release lock whether the task succeeded or failed.
    if let Ok(mut guard) = state.0.lock() {
        *guard = false;
    }

    result
}

#[tauri::command]
pub fn remap_colors(
    paths: Vec<String>,
    from: [f64; 4],
    to: [f64; 4],
    tolerance: f64,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let _guard = LockGuard::acquire(&state.0)?;
    let output_dir = output_dir.map(PathBuf::from);
    let mut output_paths = Vec::new();

    for path_str in &paths {
        let path = PathBuf::from(path_str);
        let out = output_path(&path, &output_dir, None, overwrite);
        PdfPipeline::open(&path)
            .and_then(|mut p| {
                p.remap_color(from, to, tolerance)?;
                p.save_pdf(&out)?;
                Ok(())
            })
            .map_err(friendly_error)?;
        output_paths.push(out.to_string_lossy().into_owned());
    }

    Ok(ActionResult {
        ok: true,
        message: format!("Remapped {} file(s)", paths.len()),
        output_paths,
        timestamp: now_timestamp(),
    })
}

#[tauri::command]
pub fn load_metadata(path: String) -> Result<PdfMetadataDto, String> {
    use rustybara::DocumentColorKind;

    let path = PathBuf::from(path);
    let pipeline = PdfPipeline::open(&path).map_err(friendly_error)?;
    let doc = pipeline.doc();
    let pages = doc.get_pages();

    let first_id = pages
        .values()
        .next()
        .copied()
        .ok_or_else(|| "PDF has no pages".to_string())?;

    let boxes = PageBoxes::read(doc, first_id).map_err(friendly_error)?;

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

    let color_space = match PdfPipeline::detect_color_space(doc) {
        DocumentColorKind::PureCMYK => "PureCMYK",
        DocumentColorKind::PureRGB => "PureRGB",
        DocumentColorKind::Mixed => "Mixed",
        DocumentColorKind::Unknown => "Unknown",
    }
    .to_string();

    let file_size_kb = std::fs::metadata(&path).map(|m| m.len() / 1024).unwrap_or(0);

    Ok(PdfMetadataDto {
        has_trimbox: trimbox.is_some(),
        has_bleedbox: bleedbox.is_some(),
        trimbox,
        mediabox,
        bleedbox,
        bleed_pts,
        bleed_inches: bleed_pts / 72.0,
        color_space,
        page_count: pipeline.page_count() as u32,
        file_size_kb,
    })
}

#[tauri::command]
pub async fn open_file_dialog(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .add_filter("PDF", &["pdf"])
        .pick_files(move |files| {
            let _ = tx.send(files);
        });

    let files = rx
        .recv()
        .map_err(|e| format!("Dialog channel error: {e}"))?;

    Ok(files
        .map(|paths| {
            paths
                .into_iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default())
}
