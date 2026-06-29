use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
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

/// Owns the handle to a running `rbv --listen` process.
pub struct RbvHandle {
    child: Child,
    stdin: BufWriter<ChildStdin>,
}

/// Tauri-managed state wrapping the single persistent rbv process.
pub struct ViewerHandle(pub Mutex<Option<RbvHandle>>);

#[derive(serde::Serialize, Clone)]
pub struct CustomProfileDto {
    pub name: String,
    pub description: String,
    pub color_space: String,
}

/// ISO 8601 timestamp for XMP embedding.
fn xmp_timestamp() -> String {
    chrono::Local::now().to_rfc3339()
}

fn profiles_dir<R: tauri::Runtime>(manager: &impl Manager<R>) -> Option<PathBuf> {
    let dir = manager.path().app_data_dir().ok()?.join("profiles");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

pub(crate) fn load_persisted_profiles(app: &tauri::App) {
    let Some(dir) = profiles_dir(app) else { return };
    let registry = app.state::<ProfileRegistry>();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "icc" && ext != "icm" {
            continue;
        }

        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Custom")
            .to_string();

        let Ok(profile) =
            rustybara_icc::profiles::IccProfile::from_user_bytes(name.clone(), name, bytes)
        else {
            continue;
        };

        let color_space = match profile.color_space {
            rustybara_icc::ColorSpaceKind::Cmyk => "CMYK",
            rustybara_icc::ColorSpaceKind::Rgb => "RGB",
            rustybara_icc::ColorSpaceKind::Gray => "Gray",
            _ => "Unknown",
        }
        .to_string();

        registry.0.lock().unwrap().insert(
            profile.name,
            CustomProfileEntry {
                description: profile.description,
                color_space,
                bytes: profile.bytes,
            },
        );
    }
}

/// Resolves the path to `settings.json` inside the Tauri app data directory.
/// Creates the directory if it does not yet exist. Returns `None` if the path
/// cannot be resolved (e.g. sandboxed environment without write access).
fn settings_path<R: tauri::Runtime>(manager: &impl Manager<R>) -> Option<PathBuf> {
    let path = manager.path().app_data_dir().ok()?;
    Some(path.join("settings.json"))
}

/// Reads `settings.json` from the app data directory and stores the result in
/// the `AppSettings` managed state. Falls back silently to `SettingsDto::default()`
/// on any I/O or parse error so a corrupt or missing file never prevents startup.
pub(crate) fn load_persisted_settings(app: &tauri::App) {
    let Some(path) = settings_path(app) else {
        return;
    };
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(parsed_settings) = serde_json::from_str::<SettingsDto>(&contents) else {
        return;
    };
    *app.state::<AppSettings>().0.lock().unwrap() = parsed_settings
}

#[derive(serde::Serialize)]
pub struct ActionResult {
    pub ok: bool,
    pub message: String,
    pub output_paths: Vec<String>,
    pub timestamp: String,
}

#[derive(serde::Serialize)]
pub struct XmpInfoDto {
    pub uuid: String,
    pub version: String,
    pub timestamp: String,
    pub source_hash: String,
    pub parent_id: String,
    pub ops: Vec<String>,
    /// `true` if the source file was found and its hash differs from the recorded one,
    /// `false` if they match, `null` if the source file could not be located.
    pub source_stale: Option<bool>,
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
    pub text_blocks: Vec<[f32; 4]>,
    pub image_blocks: Vec<[f32; 4]>,
    pub spot_colors: Vec<String>,
    pub has_spots: bool,
}

/// Default parameter values for each action, persisted across sessions.
/// Mirrors the `params` object in App.svelte — every field maps 1:1 to a
/// ParamsPanel control. On first launch the `Default` impl provides the same
/// values that were previously hardcoded in the frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionDefaultsDto {
    #[serde(default)]
    bleed_inches: f64,
    #[serde(default)]
    export_format: String,
    #[serde(default)]
    export_dpi: u32,
    #[serde(default)]
    remap_tolerance: f64,
    #[serde(default)]
    trim_box_bleed_inches: f64,
    #[serde(default)]
    split_panel_inches: f64,
    #[serde(default)]
    stitch_spread_inches: f64,
    #[serde(default)]
    color_intent: String,
}

impl Default for ActionDefaultsDto {
    fn default() -> Self {
        Self {
            bleed_inches: 0.125,
            export_format: "jpg".to_string(),
            export_dpi: 300,
            remap_tolerance: 1.0,
            trim_box_bleed_inches: 0.125,
            split_panel_inches: 3.67,
            stitch_spread_inches: 8.5,
            color_intent: "RelativeColorimetric".to_string(),
        }
    }
}

/// Full application settings written to `{appDataDir}/settings.json`.
/// `#[serde(default)]` on every field ensures forward compatibility — fields
/// added in future versions fall back to their `Default` values when reading
/// an older settings file rather than causing a parse error. Fields removed
/// in this version (`theme`, `responsive_threshold`) are silently ignored on
/// deserialize since `deny_unknown_fields` is not set.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SettingsDto {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    theme_preset: String,
    #[serde(default)]
    layout_override: Option<String>,
    #[serde(default)]
    wide_breakpoint_px: u32,
    #[serde(default)]
    sidebar_width: u32,
    #[serde(default)]
    font_sans: String,
    #[serde(default)]
    font_mono: String,
    #[serde(default)]
    shortcuts: HashMap<String, String>,
    #[serde(default)]
    defaults: Box<ActionDefaultsDto>,
    #[serde(default)]
    quips_enabled: bool,
    #[serde(default)]
    custom_quips: Option<Vec<String>>,
    /// Files larger than this (in MB) are hard-blocked on add — the app cannot
    /// parse very large PDFs without freezing, so they are refused outright
    /// rather than processed. `0` disables the block. See README "Known
    /// Limitations".
    #[serde(default = "default_block_size_mb")]
    resource_warn_size_mb: u32,
    /// When `true`, the Friendly Overwrite Reminder fires on context shifts
    /// (scope change or new file added) while overwrite mode is active.
    #[serde(default)]
    for_enabled: bool,
}

/// Hard file-size block threshold, in MB. Used both by `impl Default` and by
/// `#[serde(default = …)]` so a settings file that predates this field fills it
/// with this sensible value instead of `u32::default()` (0, which would disable
/// the block).
fn default_block_size_mb() -> u32 {
    200
}

impl Default for SettingsDto {
    fn default() -> Self {
        Self {
            version: 1,
            theme_preset: "ember-dark".to_string(),
            layout_override: None,
            wide_breakpoint_px: 900,
            sidebar_width: 240,
            font_sans: "Inter".to_string(),
            font_mono: "JetBrains Mono".to_string(),
            shortcuts: HashMap::new(),
            defaults: Box::new(ActionDefaultsDto::default()),
            quips_enabled: true,
            custom_quips: None,
            resource_warn_size_mb: default_block_size_mb(),
            for_enabled: true,
        }
    }
}

/// Tauri-managed state holding the live application settings.
/// Initialized from `settings.json` at startup; updated by `save_settings`.
pub struct AppSettings(pub Mutex<SettingsDto>);

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

/// Runs a blocking PDF action off the Tauri main thread.
///
/// Tauri executes synchronous commands on the main (webview) thread, so any
/// command that calls `PdfPipeline::open` directly would freeze the UI while
/// lopdf parses the file — badly so on large documents. This helper centralizes
/// the correct pattern for every action command:
///
/// 1. Acquire the single processing lock synchronously (rejecting re-entry).
/// 2. Move the owned work closure onto the blocking thread pool via
///    `spawn_blocking` so the main thread stays responsive.
/// 3. Release the lock **unconditionally** once the task finishes — including
///    when the task panics and `await` yields a `JoinError`. Returning early on
///    a join error without releasing would wedge the lock forever ("A file is
///    already being processed" on every later action).
///
/// `work` owns all of its inputs (`Vec<String>` paths, resolved ICC bytes, …);
/// nothing borrowing from `State` may cross the closure boundary, since the
/// blocking thread outlives the command's borrow.
async fn run_blocking_action<F>(lock: &Mutex<bool>, work: F) -> Result<ActionResult, String>
where
    F: FnOnce() -> Result<ActionResult, String> + Send + 'static,
{
    {
        let mut guard = lock
            .lock()
            .map_err(|_| "Processing lock poisoned".to_string())?;
        if *guard {
            return Err("A file is already being processed".to_string());
        }
        *guard = true;
    }

    let joined = tauri::async_runtime::spawn_blocking(work).await;

    // Release the lock regardless of whether the task succeeded, errored, or
    // panicked — must happen before we propagate any error.
    if let Ok(mut guard) = lock.lock() {
        *guard = false;
    }

    joined.map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn trim_marks(
    paths: Vec<String>,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let output_dir = output_dir.map(PathBuf::from);
    run_blocking_action(&state.0, move || {
        let ts = xmp_timestamp();
        let mut output_paths = Vec::new();

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let out = output_path(&path, &output_dir, None, overwrite);
            PdfPipeline::open(&path)
                .and_then(|mut p| {
                    p.trim()?;
                    p.embed_metadata(&hash, &ts, &[("trim", "")])?;
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
    })
    .await
}

#[tauri::command]
pub async fn resize_to_bleed(
    paths: Vec<String>,
    bleed_inches: f64,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let output_dir = output_dir.map(PathBuf::from);
    let bleed_pts = bleed_inches * 72.0;
    run_blocking_action(&state.0, move || {
        let mut output_paths = Vec::new();
        let params = format!("bleed_in={bleed_inches}");
        let ts = xmp_timestamp();

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let out = output_path(&path, &output_dir, None, overwrite);
            PdfPipeline::open(&path)
                .and_then(|mut p| {
                    p.resize(bleed_pts)?;
                    p.embed_metadata(&hash, &ts, &[("resize", &params)])?;
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
    })
    .await
}

#[tauri::command]
pub async fn set_media_box(
    paths: Vec<String>,
    output_dir: Option<String>,
    width_inches: f64,
    height_inches: f64,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let output_dir = output_dir.map(PathBuf::from);
    let width_pts = width_inches * 72.0;
    let height_pts = height_inches * 72.0;
    run_blocking_action(&state.0, move || {
        let mut output_paths = Vec::new();
        let params = format!("w_in={width_inches},h_in={height_inches}");
        let ts = xmp_timestamp();

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let out = output_path(&path, &output_dir, None, overwrite);
            PdfPipeline::open(&path)
                .and_then(|mut p| {
                    p.set_media_box(width_pts, height_pts)?;
                    p.embed_metadata(&hash, &ts, &[("set_media_box", &params)])?;
                    p.save_pdf(&out)?;
                    Ok(())
                })
                .map_err(friendly_error)?;
            output_paths.push(out.to_string_lossy().into_owned());
        }

        Ok(ActionResult {
            ok: true,
            message: format!(
                "Resized {} file(s) (width: {}in x height: {}in)",
                paths.len(),
                width_inches,
                height_inches,
            ),
            output_paths,
            timestamp: now_timestamp(),
        })
    })
    .await
}

/// Rotates every page of each input PDF by `degrees` (a multiple of 90) and
/// writes the result. Mirrors the other action commands: acquires the processing
/// lock and runs the lopdf work off the main thread via `run_blocking_action`.
#[tauri::command]
pub async fn rotate(
    paths: Vec<String>,
    degrees: i32,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let output_dir = output_dir.map(PathBuf::from);
    run_blocking_action(&state.0, move || {
        let mut output_paths = Vec::new();
        let params = format!("degrees={degrees}");
        let ts = xmp_timestamp();

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let out = output_path(&path, &output_dir, None, overwrite);
            PdfPipeline::open(&path)
                .and_then(|mut p| {
                    p.rotate(degrees)?;
                    p.embed_metadata(&hash, &ts, &[("rotate", &params)])?;
                    p.save_pdf(&out)?;
                    Ok(())
                })
                .map_err(friendly_error)?;
            output_paths.push(out.to_string_lossy().into_owned());
        }

        Ok(ActionResult {
            ok: true,
            message: format!("Rotated {} file(s) by {} degrees", paths.len(), degrees),
            output_paths,
            timestamp: now_timestamp(),
        })
    })
    .await
}

#[tauri::command]
pub async fn export_images(
    paths: Vec<String>,
    format: String,
    dpi: u32,
    output_dir: Option<String>,
    state: State<'_, ProcessingLock>,
    quality: u8,
) -> Result<ActionResult, String> {
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

    run_blocking_action(&state.0, move || {
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
                    .save_page_image(page, &out, &fmt, &config, quality)
                    .map_err(friendly_error)?;
                output_paths.push(out.to_string_lossy().into_owned());
                total_images += 1;
            }
        }

        Ok(ActionResult {
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
}

#[tauri::command]
pub async fn remap_colors(
    paths: Vec<String>,
    from: [f64; 4],
    to: [f64; 4],
    tolerance: f64,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let output_dir = output_dir.map(PathBuf::from);
    run_blocking_action(&state.0, move || {
        let mut output_paths = Vec::new();
        let ts = xmp_timestamp();
        let params = format!("from={from:?},to={to:?},tol={tolerance}");

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let out = output_path(&path, &output_dir, None, overwrite);
            PdfPipeline::open(&path)
                .and_then(|mut p| {
                    p.remap_color(from, to, tolerance)?;
                    p.embed_metadata(&hash, &ts, &[("remap_color", &params)])?;
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
    })
    .await
}

#[tauri::command]
pub async fn add_trim_box(
    paths: Vec<String>,
    bleed_inches: f64,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let output_dir = output_dir.map(PathBuf::from);
    let bleed_pts = bleed_inches * 72.0;
    run_blocking_action(&state.0, move || {
        let mut output_paths = Vec::new();
        let params = format!("bleed_in={bleed_inches}");
        let ts = xmp_timestamp();

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let out = output_path(&path, &output_dir, None, overwrite);
            PdfPipeline::open(&path)
                .and_then(|mut p| {
                    p.add_trim_box(bleed_pts)?;
                    p.embed_metadata(&hash, &ts, &[("add_trim_box", &params)])?;
                    p.save_pdf(&out)?;
                    Ok(())
                })
                .map_err(friendly_error)?;
            output_paths.push(out.to_string_lossy().into_owned());
        }

        Ok(ActionResult {
            ok: true,
            message: format!(
                "Added trim box to {} file(s) (bleed: {}″)",
                paths.len(),
                bleed_inches
            ),
            output_paths,
            timestamp: now_timestamp(),
        })
    })
    .await
}

#[tauri::command]
pub async fn split_pages(
    paths: Vec<String>,
    panel_width_pts: f64,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let _ = overwrite; // path is always _split; overwrite signals intent, not path selection
    let output_dir = output_dir.map(PathBuf::from);
    run_blocking_action(&state.0, move || {
        let mut output_paths = Vec::new();
        let mut total_pages = 0u32;
        let ts = xmp_timestamp();
        let params = format!("panel_width_pts={panel_width_pts}");

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let pipeline = PdfPipeline::open(&path).map_err(friendly_error)?;
            let mut result = pipeline
                .split_pages(panel_width_pts)
                .map_err(friendly_error)?;
            let dir: &std::path::Path = output_dir
                .as_deref()
                .or_else(|| path.parent().filter(|p| !p.as_os_str().is_empty()))
                .unwrap_or(std::path::Path::new("."));
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            // Always uses _split suffix — overwrite controls whether an existing _split file
            // is replaced. The source file is never touched regardless of overwrite state.
            let out = dir.join(format!("{}_split.pdf", stem));
            let page_count = result.page_count() as u32;
            result
                .embed_metadata(&hash, &ts, &[("split_pages", &params)])
                .map_err(friendly_error)?;
            result.save_pdf(&out).map_err(friendly_error)?;
            output_paths.push(out.to_string_lossy().into_owned());
            total_pages += page_count;
        }

        Ok(ActionResult {
            ok: true,
            message: format!("Split {} file(s) into {} page(s)", paths.len(), total_pages),
            output_paths,
            timestamp: now_timestamp(),
        })
    })
    .await
}

#[tauri::command]
pub async fn extract_pages(
    paths: Vec<String>,
    page_nums: Vec<u32>,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let output_dir = output_dir.map(PathBuf::from);
    run_blocking_action(&state.0, move || {
        let mut output_paths = Vec::new();
        let ts = xmp_timestamp();
        let params = format!("pages={page_nums:?}");

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let out = output_path(&path, &output_dir, None, overwrite);
            PdfPipeline::open(&path)
                .and_then(|p| {
                    let mut extracted = p.extract_pages(&page_nums)?;
                    extracted.embed_metadata(&hash, &ts, &[("extract_pages", &params)])?;
                    extracted.save_pdf(&out)?;
                    Ok(())
                })
                .map_err(friendly_error)?;
            output_paths.push(out.to_string_lossy().into_owned());
        }

        Ok(ActionResult {
            ok: true,
            message: format!(
                "Extracted {} page(s) from {} file(s)",
                page_nums.len(),
                paths.len()
            ),
            output_paths,
            timestamp: now_timestamp(),
        })
    })
    .await
}

#[tauri::command]
pub async fn flatten_spots(
    paths: Vec<String>,
    output_dir: Option<String>,
    overwrite: bool,
    icc_profile: Option<String>,
    state: State<'_, ProcessingLock>,
    profiles: State<'_, ProfileRegistry>,
) -> Result<ActionResult, String> {
    let output_dir = output_dir.map(PathBuf::from);
    let params = icc_profile
        .as_deref()
        .map(|n| format!("icc={n}"))
        .unwrap_or_default();

    // Resolve ICC bytes up front — touches the registry mutex and clones an
    // `Arc<[u8]>` (cheap), so it stays on the main thread; the resulting owned
    // bytes move into the blocking closure without borrowing `State`.
    let dst_bytes: Option<Arc<[u8]>> = match &icc_profile {
        Some(name) => Some(resolve_profile_bytes(name, &profiles)?),
        None => None,
    };

    run_blocking_action(&state.0, move || {
        let mut output_paths = Vec::new();
        let mut total_spots = 0u32;
        let ts = xmp_timestamp();

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let out = output_path(&path, &output_dir, None, overwrite);
            let spots = PdfPipeline::open(&path)
                .and_then(|mut p| {
                    let n = p.flatten_spots_with_icc(dst_bytes.as_deref())?;
                    p.embed_metadata(&hash, &ts, &[("flatten_spots", &params)])?;
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
    })
    .await
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
pub async fn convert_color_space(
    paths: Vec<String>,
    from_profile: String,
    to_profile: String,
    intent: String,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
    profiles: State<'_, ProfileRegistry>,
) -> Result<ActionResult, String> {
    // Resolve both ICC profiles up front (cheap `Arc` clones) so no `State`
    // borrow crosses into the blocking closure.
    let from_bytes = resolve_profile_bytes(&from_profile, &profiles)?;
    let to_bytes = resolve_profile_bytes(&to_profile, &profiles)?;
    let output_dir = output_dir.map(PathBuf::from);
    let params = format!("from={from_profile},to={to_profile},intent={intent}");

    run_blocking_action(&state.0, move || {
        let mut output_paths = Vec::new();
        let ts = xmp_timestamp();

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let out = output_path(&path, &output_dir, None, overwrite);
            PdfPipeline::open(&path)
                .and_then(|mut p| {
                    p.convert_color_space_raw(&from_bytes, &to_bytes, &intent)?;
                    p.embed_metadata(&hash, &ts, &[("convert_color_space", &params)])?;
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
    })
    .await
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

        let profile =
            rustybara_icc::profiles::IccProfile::from_user_bytes(name.clone(), name.clone(), bytes)
                .map_err(|e| format!("{e}"))?;

        let color_space = match profile.color_space {
            rustybara_icc::ColorSpaceKind::Cmyk => "CMYK",
            rustybara_icc::ColorSpaceKind::Rgb => "RGB",
            rustybara_icc::ColorSpaceKind::Gray => "Gray",
            _ => "Unknown",
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
            CustomProfileEntry {
                description: profile.description,
                color_space,
                bytes: profile.bytes,
            },
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
pub async fn load_metadata(path: String) -> Result<PdfMetadataDto, String> {
    tauri::async_runtime::spawn_blocking(move || load_metadata_inner(path))
        .await
        .map_err(|e| format!("Task join error: {e}"))?
}

fn load_metadata_inner(path: String) -> Result<PdfMetadataDto, String> {
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

    let file_size_kb = std::fs::metadata(&path)
        .map(|m| m.len() / 1024)
        .unwrap_or(0);

    let (text_blocks, image_blocks) = pipeline.page_layout_hint(0);

    let raw_spots = rustybara_icc::pdf::find_spot_colorspaces(doc);
    let mut spot_colors: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        raw_spots
            .into_iter()
            .map(|(_, ink)| ink)
            .filter(|ink| seen.insert(ink.clone()))
            .collect()
    };
    spot_colors.sort();
    let has_spots = !spot_colors.is_empty();

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
        text_blocks,
        image_blocks,
        spot_colors,
        has_spots,
    })
}

#[tauri::command]
pub async fn outline_text(
    paths: Vec<String>,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let output_dir = output_dir.map(PathBuf::from);
    run_blocking_action(&state.0, move || {
        let mut output_paths = Vec::new();
        let ts = xmp_timestamp();

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let out = output_path(&path, &output_dir, None, overwrite);
            PdfPipeline::open(&path)
                .and_then(|mut p| {
                    p.outline_text()?;
                    p.embed_metadata(&hash, &ts, &[("outline_text", "")])?;
                    p.save_pdf(&out)?;
                    Ok(())
                })
                .map_err(friendly_error)?;
            output_paths.push(out.to_string_lossy().into_owned());
        }

        Ok(ActionResult {
            ok: true,
            message: format!("Outlined text in {} file(s)", paths.len()),
            output_paths,
            timestamp: now_timestamp(),
        })
    })
    .await
}

#[tauri::command]
pub async fn stitch_pages(
    paths: Vec<String>,
    spread_width_pts: f64,
    output_dir: Option<String>,
    overwrite: bool,
    state: State<'_, ProcessingLock>,
) -> Result<ActionResult, String> {
    let _ = overwrite; // path is always _stitch; overwrite controls replacement, not source
    let output_dir = output_dir.map(PathBuf::from);
    run_blocking_action(&state.0, move || {
        let mut output_paths = Vec::new();
        let ts = xmp_timestamp();
        let params = format!("spread_width_pts={spread_width_pts}");

        for path_str in &paths {
            let path = PathBuf::from(path_str);
            let hash = rustybara::xmp::hash_file(&path).map_err(friendly_error)?;
            let pipeline = PdfPipeline::open(&path).map_err(friendly_error)?;
            let mut result = pipeline
                .stitch_pages(spread_width_pts)
                .map_err(friendly_error)?;
            let dir: &std::path::Path = output_dir
                .as_deref()
                .or_else(|| path.parent().filter(|p| !p.as_os_str().is_empty()))
                .unwrap_or(std::path::Path::new("."));
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            let out = dir.join(format!("{}_stitch.pdf", stem));
            result
                .embed_metadata(&hash, &ts, &[("stitch_pages", &params)])
                .map_err(friendly_error)?;
            result.save_pdf(&out).map_err(friendly_error)?;
            output_paths.push(out.to_string_lossy().into_owned());
        }

        Ok(ActionResult {
            ok: true,
            message: format!("Stitched {} file(s) into spreads", paths.len()),
            output_paths,
            timestamp: now_timestamp(),
        })
    })
    .await
}

fn try_detect_stale(processed_path: &Path, source_hash: &str) -> Option<bool> {
    let stem = processed_path.file_stem()?.to_str()?;
    let ext = processed_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("pdf");
    let source_stem = stem.strip_suffix("_processed")?;
    let source_path = processed_path.with_file_name(format!("{source_stem}.{ext}"));
    if !source_path.exists() {
        return None;
    }
    let current_hash = rustybara::xmp::hash_file(&source_path).ok()?;
    Some(current_hash != source_hash)
}

/// Read the `rbara:` XMP block embedded in a PDF by a previous rustybara run.
///
/// Returns `null` for files that have never been processed by rustybara, or if the
/// file cannot be opened. Includes a `source_stale` flag when the original source
/// file is still present alongside the processed output.
#[tauri::command]
pub async fn read_xmp_metadata(path: String) -> Option<XmpInfoDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(&path);
        let pipeline = PdfPipeline::open(&path).ok()?;
        let block = pipeline.read_xmp_block()?;
        let source_stale = try_detect_stale(&path, &block.source_hash);
        Some(XmpInfoDto {
            source_stale,
            uuid: block.uuid,
            version: block.version,
            timestamp: block.timestamp,
            source_hash: block.source_hash,
            parent_id: block.parent_id,
            ops: block.ops,
        })
    })
    .await
    .ok()
    .flatten()
}

/// Notify the persistent rbv process of a new file path, but **only if rbv is
/// already running**.  Unlike [`open_in_viewer_persistent`] this never spawns a
/// new process — it is intended for auto-update after a processing action.
#[tauri::command]
pub fn notify_viewer(path: String, handle: State<'_, ViewerHandle>) -> Result<(), String> {
    use std::io::Write;
    let mut guard = handle
        .0
        .lock()
        .map_err(|_| "viewer handle mutex poisoned".to_string())?;

    let alive = guard
        .as_mut()
        .map_or(false, |h| matches!(h.child.try_wait(), Ok(None)));

    if alive {
        let h = guard.as_mut().unwrap();
        let cmd = format!("OPEN {path}\n");
        h.stdin
            .write_all(cmd.as_bytes())
            .and_then(|_| h.stdin.flush())
            .map_err(|e| format!("IPC notify failed (rbv may have closed): {e}"))?;
    }
    // rbv not running — silently succeed; caller fire-and-forgets this.
    Ok(())
}

#[tauri::command]
pub fn open_in_viewer(
    app: tauri::AppHandle,
    path: String,
    page: u32,
    dpi: u32,
) -> Result<(), String> {
    let rbv_name = if cfg!(windows) { "rbv.exe" } else { "rbv" };
    // resource_dir() returns the correct directory on all platforms:
    // Windows/Linux: same directory as the executable
    // macOS .app: Contents/Resources/ (where Tauri bundles resources)
    let rbv = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Cannot locate resource directory: {e}"))?
        .join(rbv_name);

    let mut child = std::process::Command::new(&rbv)
        .arg(&path)
        .arg(page.to_string())
        .args(["--dpi", &dpi.to_string()])
        .spawn()
        .map_err(|e| format!("Failed to launch rbv ({}): {e}", rbv.display()))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

// Open a PDF in the persistent rbv viewer process.
///
/// If rbv is already running and alive, sends `OPEN` (and optionally `PAGE`)
/// commands over its stdin pipe — no new process is spawned.
/// If rbv has exited or was never started, a fresh process is spawned with
/// `--listen` so subsequent calls can reuse it.
#[tauri::command]
pub fn open_in_viewer_persistent(
    app: tauri::AppHandle,
    handle: State<ViewerHandle>,
    path: String,
    page: u32,
    dpi: u32,
) -> Result<(), String> {
    let mut guard = handle
        .0
        .lock()
        .map_err(|_| "viewer handle mutex poisoned".to_string())?;

    // Check whether the existing child process is still alive.
    let alive = guard
        .as_mut()
        .map_or(false, |h| matches!(h.child.try_wait(), Ok(None)));

    if alive {
        // Reuse the existing process — send IPC commands.
        let h = guard.as_mut().unwrap();
        let cmd = format!("OPEN {path}\nPAGE {page}\n");
        h.stdin
            .write_all(cmd.as_bytes())
            .and_then(|_| h.stdin.flush())
            .map_err(|e| format!("IPC write failed (rbv may have closed): {e}"))?;
        return Ok(());
    }

    // No alive process — spawn a fresh rbv with --listen.
    let rbv_name = if cfg!(windows) { "rbv.exe" } else { "rbv" };
    let rbv = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Cannot locate resource directory: {e}"))?
        .join(rbv_name);

    let mut child = std::process::Command::new(&rbv)
        .arg(&path)
        .arg(page.to_string())
        .args(["--dpi", &dpi.to_string()])
        .arg("--listen")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to launch rbv ({}): {e}", rbv.display()))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to open rbv stdin pipe".to_string())?;

    *guard = Some(RbvHandle {
        child,
        stdin: BufWriter::new(stdin),
    });

    Ok(())
}

#[tauri::command]
pub fn list_dirs(path: String) -> Vec<String> {
    let p = std::path::Path::new(&path);
    if !p.is_dir() {
        return vec![];
    }
    let mut dirs: Vec<String> = std::fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    dirs.sort();
    dirs
}

#[tauri::command]
pub fn list_pdf_files(path: String) -> Vec<String> {
    let p = std::path::Path::new(&path);
    if !p.is_dir() {
        return vec![];
    }
    let mut files: Vec<String> = std::fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            e.path().is_file()
                && e.path()
                    .extension()
                    .map(|x| x.to_ascii_lowercase() == "pdf")
                    .unwrap_or(false)
        })
        .filter_map(|e| e.path().to_str().map(str::to_string))
        .collect();
    files.sort();
    files
}

#[tauri::command]
pub fn minimize_window(window: tauri::WebviewWindow) {
    let _ = window.minimize();
}

#[tauri::command]
pub async fn toggle_maximize_window(window: tauri::WebviewWindow) {
    if window.is_maximized().unwrap_or(false) {
        let _ = window.unmaximize();
    } else {
        let _ = window.maximize();
    }
}

#[tauri::command]
pub fn exit_app() {
    std::process::exit(0);
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
        .map(|paths| paths.into_iter().map(|p| p.to_string()).collect::<Vec<_>>())
        .unwrap_or_default())
}

/// Returns the file size in kilobytes using only a filesystem metadata call —
/// no PDF parsing. Use this to pre-check large files before calling
/// `load_metadata` so lopdf never opens a file that should be chunked first.
#[tauri::command]
pub fn get_file_size(path: String) -> Result<u64, String> {
    std::fs::metadata(&path)
        .map(|m| m.len() / 1024)
        .map_err(|e| e.to_string())
}

/// Returns a clone of the current settings from managed state.
/// Called once by the frontend at startup to hydrate the reactive settings store.
#[tauri::command]
pub fn load_settings(state: State<'_, AppSettings>) -> SettingsDto {
    state.0.lock().unwrap().clone()
}

/// Serializes `settings` to `{appDataDir}/settings.json` and replaces the
/// in-memory copy atomically. Returns an error string if the file cannot be
/// written so the frontend can surface a notification without crashing.
#[tauri::command]
pub fn save_settings(
    settings: SettingsDto,
    state: State<'_, AppSettings>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let path = settings_path(&app).ok_or("Cannot resolve app data dir")?;
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    *state.0.lock().unwrap() = settings;
    Ok(())
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn settings_dto_default_version_is_one() {
        let s = SettingsDto::default();
        assert_eq!(s.version, 1);
    }

    #[test]
    fn settings_dto_default_shortcuts_is_empty() {
        let s = SettingsDto::default();
        assert!(s.shortcuts.is_empty());
    }

    #[test]
    fn settings_dto_default_quips_enabled() {
        let s = SettingsDto::default();
        assert!(s.quips_enabled);
    }

    #[test]
    fn settings_dto_default_theme_preset_is_ember_dark() {
        assert_eq!(SettingsDto::default().theme_preset, "ember-dark");
    }

    #[test]
    fn settings_dto_default_wide_breakpoint_px_is_900() {
        assert_eq!(SettingsDto::default().wide_breakpoint_px, 900);
    }

    #[test]
    fn settings_dto_default_custom_quips_is_none() {
        assert!(SettingsDto::default().custom_quips.is_none());
    }

    #[test]
    fn settings_dto_default_fonts_are_nonempty() {
        let s = SettingsDto::default();
        assert!(!s.font_sans.is_empty());
        assert!(!s.font_mono.is_empty());
    }

    #[test]
    fn settings_dto_roundtrips_through_json() {
        let original = SettingsDto::default();
        let json = serde_json::to_string(&original).unwrap();
        let parsed: SettingsDto = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.theme_preset, original.theme_preset);
        assert_eq!(parsed.version, original.version);
        assert_eq!(parsed.quips_enabled, original.quips_enabled);
        assert_eq!(parsed.wide_breakpoint_px, original.wide_breakpoint_px);
    }

    #[test]
    fn settings_dto_unknown_fields_ignored_on_deserialize() {
        // Verifies #[serde(deny_unknown_fields)] is NOT present — required for
        // forward compatibility, and also ensures old settings files that still
        // carry the removed `theme` and `responsive_threshold` fields load cleanly.
        let json = r#"{
            "version": 1,
            "theme": "dark",
            "responsive_threshold": 1.4,
            "theme_preset": "ember-dark",
            "unknown_future_field": 42,
            "quips_enabled": true
        }"#;
        let result = serde_json::from_str::<SettingsDto>(json);
        assert!(
            result.is_ok(),
            "unknown/removed fields must not cause a parse failure"
        );
        assert_eq!(result.unwrap().theme_preset, "ember-dark");
    }

    #[test]
    fn settings_dto_custom_quips_roundtrips() {
        let mut s = SettingsDto::default();
        s.custom_quips = Some(vec!["hello world".to_string(), "test quip".to_string()]);
        let json = serde_json::to_string(&s).unwrap();
        let parsed: SettingsDto = serde_json::from_str(&json).unwrap();
        let quips = parsed
            .custom_quips
            .expect("custom_quips should survive roundtrip");
        assert_eq!(quips.len(), 2);
        assert_eq!(quips[0], "hello world");
    }

    #[test]
    fn settings_dto_default_resource_warn_size_mb_is_200() {
        assert_eq!(SettingsDto::default().resource_warn_size_mb, 200);
    }

    #[test]
    fn settings_dto_default_for_enabled_is_true() {
        assert!(SettingsDto::default().for_enabled);
    }

    #[test]
    fn action_defaults_roundtrip_through_json() {
        let d = ActionDefaultsDto::default();
        let json = serde_json::to_string(&d).unwrap();
        let parsed: ActionDefaultsDto = serde_json::from_str(&json).unwrap();
        assert!((parsed.bleed_inches - d.bleed_inches).abs() < f64::EPSILON);
        assert_eq!(parsed.export_format, d.export_format);
        assert_eq!(parsed.export_dpi, d.export_dpi);
    }

    #[test]
    fn action_defaults_export_dpi_is_300() {
        assert_eq!(ActionDefaultsDto::default().export_dpi, 300);
    }

    #[test]
    fn action_defaults_bleed_is_eighth_inch() {
        assert!((ActionDefaultsDto::default().bleed_inches - 0.125).abs() < f64::EPSILON);
    }
}
