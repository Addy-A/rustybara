use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rustybara::encode::OutputFormat;
use rustybara::pages::PageBoxes;
use rustybara::raster::RenderConfig;
use rustybara::PdfPipeline;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

pub struct ProcessingLock(pub Mutex<bool>);

pub(crate) struct CustomProfileEntry {
    description: String,
    color_space: String,
    bytes: Arc<[u8]>,
}

pub struct ProfileRegistry(pub(crate) Mutex<HashMap<String, CustomProfileEntry>>);

#[derive(serde::Serialize, Clone)]
pub struct CustomProfileDto {
    pub name: String,
    pub description: String,
    pub color_space: String,
}

fn profiles_dir<R: tauri::Runtime>(manager: &impl Manager<R>) -> Option<PathBuf> {
    let dir = manager.path().app_data_dir().ok()?.join("profiles");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

pub(crate) fn load_persisted_profiles(app: &tauri::App) {
    let Some(dir) = profiles_dir(app) else { return };
    let registry = app.state::<ProfileRegistry>();
    let Ok(entries) = std::fs::read_dir(&dir) else { return };

    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext != "icc" && ext != "icm" { continue; }

        let Ok(bytes) = std::fs::read(&path) else { continue };
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Custom")
            .to_string();

        let Ok(profile) = rustybara_icc::profiles::IccProfile::from_user_bytes(
            name.clone(),
            name,
            bytes,
        ) else { continue };

        let color_space = match profile.color_space {
            rustybara_icc::ColorSpaceKind::Cmyk => "CMYK",
            rustybara_icc::ColorSpaceKind::Rgb  => "RGB",
            rustybara_icc::ColorSpaceKind::Gray => "Gray",
            _                                   => "Unknown",
        }
        .to_string();

        registry.0.lock().unwrap().insert(
            profile.name,
            CustomProfileEntry { description: profile.description, color_space, bytes: profile.bytes },
        );
    }
}

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
        rustybara::Error::Color(_) => format!("Color space conversion failed: {e}"),
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
pub fn add_trim_box(
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
                p.add_trim_box(bleed_pts)?;
                p.save_pdf(&out)?;
                Ok(())
            })
            .map_err(friendly_error)?;
        output_paths.push(out.to_string_lossy().into_owned());
    }

    Ok(ActionResult {
        ok: true,
        message: format!("Added trim box to {} file(s) (bleed: {}″)", paths.len(), bleed_inches),
        output_paths,
        timestamp: now_timestamp(),
    })
}

#[tauri::command]
pub fn split_pages(
    paths: Vec<String>,
    output_dir: Option<String>,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let _guard = LockGuard::acquire(&state.0)?;
    let output_dir = output_dir.map(PathBuf::from);
    let mut output_paths = Vec::new();
    let mut total_pages = 0u32;

    for path_str in &paths {
        let path = PathBuf::from(path_str);
        let pipeline = PdfPipeline::open(&path).map_err(friendly_error)?;
        let pages = pipeline.split_pages().map_err(friendly_error)?;
        let dir: &std::path::Path = output_dir.as_deref()
            .or_else(|| path.parent().filter(|p| !p.as_os_str().is_empty()))
            .unwrap_or(std::path::Path::new("."));
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        for (i, mut page) in pages.into_iter().enumerate() {
            let out = dir.join(format!("{}_page_{}.pdf", stem, i + 1));
            page.save_pdf(&out).map_err(friendly_error)?;
            output_paths.push(out.to_string_lossy().into_owned());
            total_pages += 1;
        }
    }

    Ok(ActionResult {
        ok: true,
        message: format!("Split {} file(s) into {} page(s)", paths.len(), total_pages),
        output_paths,
        timestamp: now_timestamp(),
    })
}

#[tauri::command]
pub fn extract_pages(
    paths: Vec<String>,
    page_nums: Vec<u32>,
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
            .and_then(|p| {
                let mut extracted = p.extract_pages(&page_nums)?;
                extracted.save_pdf(&out)?;
                Ok(())
            })
            .map_err(friendly_error)?;
        output_paths.push(out.to_string_lossy().into_owned());
    }

    Ok(ActionResult {
        ok: true,
        message: format!("Extracted {} page(s) from {} file(s)", page_nums.len(), paths.len()),
        output_paths,
        timestamp: now_timestamp(),
    })
}

#[tauri::command]
pub fn flatten_spots(
    paths: Vec<String>,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let _guard = LockGuard::acquire(&state.0)?;
    let output_dir = output_dir.map(PathBuf::from);
    let mut output_paths = Vec::new();
    let mut total_spots = 0u32;

    for path_str in &paths {
        let path = PathBuf::from(path_str);
        let out = output_path(&path, &output_dir, None, overwrite);
        let spots = PdfPipeline::open(&path)
            .and_then(|mut p| {
                let n = p.flatten_spots()?;
                p.save_pdf(&out)?;
                Ok(n)
            })
            .map_err(friendly_error)?;
        total_spots += spots;
        output_paths.push(out.to_string_lossy().into_owned());
    }

    Ok(ActionResult {
        ok: true,
        message: format!(
            "Flattened {} spot color use(s) across {} file(s)",
            total_spots,
            paths.len()
        ),
        output_paths,
        timestamp: now_timestamp(),
    })
}

fn resolve_profile_bytes(
    name: &str,
    registry: &State<'_, ProfileRegistry>,
) -> Result<Arc<[u8]>, String> {
    if let Some(p) = rustybara_icc::profiles::by_name(name) {
        return Ok(p.bytes.clone());
    }
    registry
        .0
        .lock()
        .unwrap()
        .get(name)
        .map(|e| e.bytes.clone())
        .ok_or_else(|| {
            format!("Unknown profile '{name}'. Load a custom profile or check the name.")
        })
}

#[tauri::command]
pub fn convert_color_space(
    paths: Vec<String>,
    from_profile: String,
    to_profile: String,
    intent: String,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
    profiles: State<'_, ProfileRegistry>,
) -> Result<ActionResult, String> {
    let _guard = LockGuard::acquire(&state.0)?;
    let from_bytes = resolve_profile_bytes(&from_profile, &profiles)?;
    let to_bytes = resolve_profile_bytes(&to_profile, &profiles)?;
    let output_dir = output_dir.map(PathBuf::from);
    let mut output_paths = Vec::new();

    for path_str in &paths {
        let path = PathBuf::from(path_str);
        let out = output_path(&path, &output_dir, None, overwrite);
        PdfPipeline::open(&path)
            .and_then(|mut p| {
                p.convert_color_space_raw(&from_bytes, &to_bytes, &intent)?;
                p.save_pdf(&out)?;
                Ok(())
            })
            .map_err(friendly_error)?;
        output_paths.push(out.to_string_lossy().into_owned());
    }

    Ok(ActionResult {
        ok: true,
        message: format!(
            "Converted {} file(s): {} → {}",
            paths.len(),
            from_profile,
            to_profile
        ),
        output_paths,
        timestamp: now_timestamp(),
    })
}

#[tauri::command]
pub async fn load_icc_profile(
    app: tauri::AppHandle,
    profiles: State<'_, ProfileRegistry>,
) -> Result<Vec<CustomProfileDto>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .add_filter("ICC Profile", &["icc", "icm"])
        .pick_files(move |files| {
            let _ = tx.send(files);
        });

    let files = rx.recv().map_err(|e| format!("Dialog error: {e}"))?;
    let Some(file_paths) = files else {
        return Ok(Vec::new());
    };

    let mut results = Vec::new();
    for file_path in file_paths {
        let path = file_path
            .into_path()
            .map_err(|e| format!("Invalid path: {e}"))?;
        let bytes = std::fs::read(&path).map_err(|e| format!("Could not read file: {e}"))?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Custom")
            .to_string();

        let profile = rustybara_icc::profiles::IccProfile::from_user_bytes(
            name.clone(),
            name.clone(),
            bytes,
        )
        .map_err(|e| format!("{e}"))?;

        let color_space = match profile.color_space {
            rustybara_icc::ColorSpaceKind::Cmyk    => "CMYK",
            rustybara_icc::ColorSpaceKind::Rgb     => "RGB",
            rustybara_icc::ColorSpaceKind::Gray    => "Gray",
            _                                      => "Unknown",
        }
        .to_string();

        let dto = CustomProfileDto {
            name: profile.name.clone(),
            description: profile.description.clone(),
            color_space: color_space.clone(),
        };

        if let Some(dir) = profiles_dir(&app) {
            let out = dir.join(format!("{}.icc", profile.name));
            let _ = std::fs::write(out, &*profile.bytes);
        }

        profiles.0.lock().unwrap().insert(
            profile.name,
            CustomProfileEntry { description: profile.description, color_space, bytes: profile.bytes },
        );

        results.push(dto);
    }

    Ok(results)
}

#[tauri::command]
pub fn list_custom_profiles(profiles: State<'_, ProfileRegistry>) -> Vec<CustomProfileDto> {
    profiles
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|(name, e)| CustomProfileDto {
            name: name.clone(),
            description: e.description.clone(),
            color_space: e.color_space.clone(),
        })
        .collect()
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
pub fn open_in_viewer(path: String, page: u32, dpi: u32) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Cannot locate executable: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "Cannot determine executable directory".to_string())?;
    let rbv = dir.join(if cfg!(windows) { "rbv.exe" } else { "rbv" });
    std::process::Command::new(&rbv)
        .arg(&path)
        .arg(page.to_string())
        .args(["--dpi", &dpi.to_string()])
        .spawn()
        .map_err(|e| format!("Failed to launch rbv ({}): {e}", rbv.display()))?;
    Ok(())
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
