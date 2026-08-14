use crate::cli::{ImageFormat, PanelAxis, RenderingIntent};
use crate::tui::app::{ActionLogEntry, App, ColorSpaceInfo, LogStatus, MenuAction};
use rustybara::PdfPipeline;
use rustybara::pages::PageBoxes;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const POINTS_PER_INCH: f64 = 72.0;

fn invalid_input(message: impl Into<String>) -> rustybara::Error {
    rustybara::Error::Io(io::Error::new(io::ErrorKind::InvalidInput, message.into()))
}

fn positive(value: f64, name: &str) -> rustybara::Result<f64> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(invalid_input(format!("{name} must be a positive number")))
    }
}

pub fn bleed_points(
    points: Option<f64>,
    inches: Option<f64>,
    default_points: f64,
) -> rustybara::Result<f64> {
    let value = match (points, inches) {
        (Some(_), Some(_)) => {
            return Err(invalid_input(
                "choose either points or inches for bleed, not both",
            ));
        }
        (Some(value), None) => value,
        (None, Some(value)) => positive(value, "bleed")? * POINTS_PER_INCH,
        (None, None) => default_points,
    };
    positive(value, "bleed")
}

pub fn output_path(
    input: &Path,
    output_dir: &Option<PathBuf>,
    new_ext: Option<&str>,
    overwrite: bool,
) -> PathBuf {
    output_path_with_suffix(input, output_dir, "processed", new_ext, overwrite)
}

fn output_path_with_suffix(
    input: &Path,
    output_dir: &Option<PathBuf>,
    suffix: &str,
    new_ext: Option<&str>,
    overwrite: bool,
) -> PathBuf {
    if overwrite {
        return input.to_path_buf();
    }
    let dir = output_dir
        .as_deref()
        .unwrap_or_else(|| match input.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        });
    let stem = input.file_stem().unwrap_or_default();
    let ext = new_ext.unwrap_or_else(|| {
        input
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("pdf")
    });
    dir.join(format!("{}_{suffix}.{ext}", stem.to_string_lossy()))
}

fn collision_key(path: &Path) -> String {
    let key = path.to_string_lossy();
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key.into_owned()
    }
}

fn plan_output_paths_with_suffix(
    input: &[PathBuf],
    output_dir: &Option<PathBuf>,
    suffix: &str,
    new_ext: Option<&str>,
    overwrite: bool,
) -> rustybara::Result<Vec<PathBuf>> {
    if let Some(dir) = output_dir
        && !dir.is_dir()
    {
        return Err(invalid_input(format!(
            "output directory does not exist: {}",
            dir.display()
        )));
    }

    let outputs: Vec<PathBuf> = input
        .iter()
        .map(|path| output_path_with_suffix(path, output_dir, suffix, new_ext, overwrite))
        .collect();
    let mut seen: HashMap<String, &Path> = HashMap::new();

    for (source, output) in input.iter().zip(&outputs) {
        if let Some(previous) = seen.insert(collision_key(output), source) {
            return Err(invalid_input(format!(
                "output collision: '{}' and '{}' both map to '{}'",
                previous.display(),
                source.display(),
                output.display()
            )));
        }
    }

    Ok(outputs)
}

fn plan_output_paths(
    input: &[PathBuf],
    output_dir: &Option<PathBuf>,
    new_ext: Option<&str>,
    overwrite: bool,
) -> rustybara::Result<Vec<PathBuf>> {
    plan_output_paths_with_suffix(input, output_dir, "processed", new_ext, overwrite)
}

fn apply_pdf_mutation<F>(
    input: &[PathBuf],
    output: &Option<PathBuf>,
    overwrite: bool,
    operation: &str,
    params: &str,
    mut apply: F,
) -> rustybara::Result<Vec<PathBuf>>
where
    F: FnMut(&mut PdfPipeline) -> rustybara::Result<()>,
{
    let outputs = plan_output_paths(input, output, None, overwrite)?;
    let timestamp = chrono::Local::now().to_rfc3339();
    for (path, out) in input.iter().zip(&outputs) {
        let source_hash = rustybara::xmp::hash_file(path)?;
        let mut pipeline = PdfPipeline::open(path)?;
        apply(&mut pipeline)?;
        pipeline.embed_metadata(&source_hash, &timestamp, &[(operation, params)])?;
        pipeline.save_pdf(out)?;
    }
    Ok(outputs)
}

fn apply_pdf_derivation<F>(
    input: &[PathBuf],
    output: &Option<PathBuf>,
    overwrite: bool,
    suffix: &str,
    operation: &str,
    params: &str,
    mut derive: F,
) -> rustybara::Result<Vec<PathBuf>>
where
    F: FnMut(&PdfPipeline, &Path) -> rustybara::Result<PdfPipeline>,
{
    let outputs = plan_output_paths_with_suffix(input, output, suffix, None, overwrite)?;
    let timestamp = chrono::Local::now().to_rfc3339();
    for (path, out) in input.iter().zip(&outputs) {
        let source_hash = rustybara::xmp::hash_file(path)?;
        let source = PdfPipeline::open(path)?;
        let mut pipeline = derive(&source, path)?;
        pipeline.embed_metadata(&source_hash, &timestamp, &[(operation, params)])?;
        pipeline.save_pdf(out)?;
    }
    Ok(outputs)
}

fn print_outputs(input: &[PathBuf], outputs: &[PathBuf]) {
    for (path, out) in input.iter().zip(outputs) {
        println!("{} -> {}", path.display(), out.display());
    }
}

pub fn run_trim(
    input: Vec<PathBuf>,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    let outputs = apply_pdf_mutation(&input, &output, overwrite, "trim", "", |pipeline| {
        pipeline.trim()?;
        Ok(())
    })?;
    print_outputs(&input, &outputs);
    Ok(())
}

pub fn run_resize(
    input: Vec<PathBuf>,
    bleed: f64,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    let bleed = positive(bleed, "bleed")?;
    let params = format!("bleed_pts={bleed}");
    let outputs = apply_pdf_mutation(&input, &output, overwrite, "resize", &params, |pipeline| {
        pipeline.resize(bleed)?;
        Ok(())
    })?;
    print_outputs(&input, &outputs);
    Ok(())
}

pub fn run_add_trim_box(
    input: Vec<PathBuf>,
    bleed: f64,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    let bleed = positive(bleed, "bleed")?;
    let params = format!("bleed_pts={bleed}");
    let outputs = apply_pdf_mutation(
        &input,
        &output,
        overwrite,
        "add_trim_box",
        &params,
        |pipeline| {
            pipeline.add_trim_box(bleed)?;
            Ok(())
        },
    )?;
    print_outputs(&input, &outputs);
    Ok(())
}

pub fn run_set_media_box(
    input: Vec<PathBuf>,
    width_inches: f64,
    height_inches: f64,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    let width = positive(width_inches, "width")? * POINTS_PER_INCH;
    let height = positive(height_inches, "height")? * POINTS_PER_INCH;
    let params = format!("width_in={width_inches},height_in={height_inches}");
    let outputs = apply_pdf_mutation(
        &input,
        &output,
        overwrite,
        "set_media_box",
        &params,
        |pipeline| {
            pipeline.set_media_box(width, height)?;
            Ok(())
        },
    )?;
    print_outputs(&input, &outputs);
    Ok(())
}

pub fn run_rotate(
    input: Vec<PathBuf>,
    degrees: i32,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    if degrees % 90 != 0 {
        return Err(invalid_input("rotation must be a multiple of 90"));
    }
    let params = format!("degrees={degrees}");
    let outputs = apply_pdf_mutation(&input, &output, overwrite, "rotate", &params, |pipeline| {
        pipeline.rotate(degrees)?;
        Ok(())
    })?;
    print_outputs(&input, &outputs);
    Ok(())
}

fn output_format(format: ImageFormat) -> rustybara::encode::OutputFormat {
    use rustybara::encode::OutputFormat;
    match format {
        ImageFormat::Jpg => OutputFormat::Jpg,
        ImageFormat::Png => OutputFormat::Png,
        ImageFormat::Webp => OutputFormat::WebP,
        ImageFormat::Tiff => OutputFormat::Tiff,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run_image(
    input: Vec<PathBuf>,
    output: Option<PathBuf>,
    format: ImageFormat,
    dpi: u32,
    quality: u8,
    annotations: bool,
    forms: bool,
    overwrite: bool,
) -> rustybara::Result<()> {
    if overwrite {
        return Err(invalid_input(
            "image export cannot overwrite a PDF source; choose an output directory instead",
        ));
    }
    if dpi == 0 {
        return Err(invalid_input("DPI must be greater than zero"));
    }
    if !(1..=100).contains(&quality) {
        return Err(invalid_input("image quality must be between 1 and 100"));
    }

    export_images(
        &input,
        &output,
        format,
        dpi,
        quality,
        annotations,
        forms,
        true,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn export_images(
    input: &[PathBuf],
    output: &Option<PathBuf>,
    format: ImageFormat,
    dpi: u32,
    quality: u8,
    annotations: bool,
    forms: bool,
    print_progress: bool,
) -> rustybara::Result<u32> {
    use rustybara::raster::RenderConfig;

    let fmt = output_format(format);
    let config = RenderConfig {
        dpi,
        render_annotations: annotations,
        render_form_data: forms,
    };
    let base_outputs = plan_output_paths(input, output, Some(fmt.extension()), false)?;
    let mut total = 0;
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
            pipeline.save_page_image(page, &out, &fmt, &config, quality)?;
            total += 1;
            if print_progress {
                println!("{} page {} -> {}", path.display(), page + 1, out.display());
            }
        }
    }
    Ok(total)
}

fn parse_page_ranges(spec: &str) -> rustybara::Result<Vec<(u32, u32)>> {
    let mut ranges = Vec::new();
    for part in spec.split(',').map(str::trim) {
        if part.is_empty() {
            return Err(invalid_input("page selection contains an empty item"));
        }
        if let Some((start, end)) = part.split_once('-') {
            if end.contains('-') {
                return Err(invalid_input(format!("invalid page range: {part}")));
            }
            let start = start
                .trim()
                .parse::<u32>()
                .map_err(|_| invalid_input(format!("invalid page number: {start}")))?;
            let end = end
                .trim()
                .parse::<u32>()
                .map_err(|_| invalid_input(format!("invalid page number: {end}")))?;
            if start == 0 || end == 0 {
                return Err(invalid_input("page numbers start at 1"));
            }
            if start > end {
                return Err(invalid_input(format!("page range is reversed: {part}")));
            }
            ranges.push((start, end));
        } else {
            let page = part
                .parse::<u32>()
                .map_err(|_| invalid_input(format!("invalid page number: {part}")))?;
            if page == 0 {
                return Err(invalid_input("page numbers start at 1"));
            }
            ranges.push((page, page));
        }
    }
    if ranges.is_empty() {
        return Err(invalid_input("page selection cannot be empty"));
    }
    Ok(ranges)
}

pub fn validate_page_selection(spec: &str) -> rustybara::Result<()> {
    parse_page_ranges(spec).map(|_| ())
}

fn page_indices(spec: &str, page_count: usize) -> rustybara::Result<Vec<u32>> {
    let ranges = parse_page_ranges(spec)?;
    if ranges.iter().any(|(_, end)| *end as usize > page_count) {
        return Err(invalid_input(format!(
            "page selection exceeds the document's {page_count} page(s)"
        )));
    }
    let mut pages = BTreeSet::new();
    for (start, end) in ranges {
        pages.extend((start - 1)..=(end - 1));
    }
    Ok(pages.into_iter().collect())
}

pub fn run_extract_pages(
    input: Vec<PathBuf>,
    pages: &str,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    validate_page_selection(pages)?;
    let params = format!("pages={pages}");
    let outputs = apply_pdf_derivation(
        &input,
        &output,
        overwrite,
        "extracted",
        "extract_pages",
        &params,
        |pipeline, path| {
            let page_indices = page_indices(pages, pipeline.page_count()).map_err(|_| {
                invalid_input(format!(
                    "page selection exceeds the {} page(s) in '{}'",
                    pipeline.page_count(),
                    path.display()
                ))
            })?;
            pipeline.extract_pages(&page_indices)
        },
    )?;
    print_outputs(&input, &outputs);
    Ok(())
}

pub fn run_split_pages(
    input: Vec<PathBuf>,
    panel_width_inches: f64,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    run_split_pages_with_layout(
        input,
        panel_width_inches,
        &[],
        PanelAxis::Horizontal,
        output,
        overwrite,
    )
}

pub fn run_split_pages_with_layout(
    input: Vec<PathBuf>,
    panel_width_inches: f64,
    panel_widths_inches: &[f64],
    axis: PanelAxis,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    let width = positive(panel_width_inches, "panel width")? * POINTS_PER_INCH;
    let explicit = panel_widths_inches
        .iter()
        .map(|value| positive(*value, "explicit panel width").map(|value| value * POINTS_PER_INCH))
        .collect::<rustybara::Result<Vec<_>>>()?;
    if !explicit.is_empty() && explicit.len() < 2 {
        return Err(invalid_input(
            "explicit panel plan needs at least two widths",
        ));
    }
    let axis_name = match axis {
        PanelAxis::Horizontal => "horizontal",
        PanelAxis::Vertical => "vertical",
    };
    let params = if explicit.is_empty() {
        format!("panel_width_in={panel_width_inches};axis={axis_name}")
    } else {
        format!(
            "panel_widths_in={};axis={axis_name}",
            panel_widths_inches
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let outputs = apply_pdf_derivation(
        &input,
        &output,
        overwrite,
        "split",
        "split_pages",
        &params,
        |pipeline, _| {
            if explicit.is_empty() {
                pipeline.split_pages(width)
            } else {
                let split_axis = match axis {
                    PanelAxis::Horizontal => rustybara::pages::SplitAxis::Horizontal,
                    PanelAxis::Vertical => rustybara::pages::SplitAxis::Vertical,
                };
                pipeline.split_pages_explicit(&explicit, split_axis)
            }
        },
    )?;
    print_outputs(&input, &outputs);
    Ok(())
}

pub fn run_stitch_pages(
    input: Vec<PathBuf>,
    spread_width_inches: f64,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    let width = positive(spread_width_inches, "spread width")? * POINTS_PER_INCH;
    let params = format!("spread_width_in={spread_width_inches}");
    let outputs = apply_pdf_derivation(
        &input,
        &output,
        overwrite,
        "stitch",
        "stitch_pages",
        &params,
        |pipeline, _| pipeline.stitch_pages(width),
    )?;
    print_outputs(&input, &outputs);
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
        .map_err(|_| invalid_input("--from requires exactly 4 values"))?;
    let to: [f64; 4] = to_vec
        .try_into()
        .map_err(|_| invalid_input("--to requires exactly 4 values"))?;
    if !from
        .iter()
        .chain(&to)
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
    {
        return Err(invalid_input("CMYK values must be between 0 and 1"));
    }
    if !tolerance.is_finite() || !(0.0..=1.0).contains(&tolerance) {
        return Err(invalid_input("tolerance must be between 0 and 1"));
    }
    let params = format!("from={from:?},to={to:?},tolerance={tolerance}");
    let outputs = apply_pdf_mutation(
        &input,
        &output,
        overwrite,
        "remap_color",
        &params,
        |pipeline| {
            pipeline.remap_color(from, to, tolerance)?;
            Ok(())
        },
    )?;
    print_outputs(&input, &outputs);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_convert_color_space(
    input: Vec<PathBuf>,
    output: Option<PathBuf>,
    from_profile: &str,
    to_profile: &str,
    intent: RenderingIntent,
    from_icc: Option<PathBuf>,
    to_icc: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    let intent_name = intent.as_pipeline_str();
    let raw_profiles = match (from_icc, to_icc) {
        (Some(from), Some(to)) => Some((fs::read(from)?, fs::read(to)?)),
        (None, None) => None,
        _ => {
            return Err(invalid_input(
                "--from-icc and --to-icc must be used together",
            ));
        }
    };
    let params = if raw_profiles.is_some() {
        format!("profiles=files,intent={intent_name}")
    } else {
        format!("from={from_profile},to={to_profile},intent={intent_name}")
    };
    let outputs = apply_pdf_mutation(
        &input,
        &output,
        overwrite,
        "convert_color_space",
        &params,
        |pipeline| {
            if let Some((from, to)) = raw_profiles.as_ref() {
                pipeline.convert_color_space_raw(from, to, intent_name)
            } else {
                pipeline.convert_color_space(from_profile, to_profile, intent_name)
            }
        },
    )?;
    print_outputs(&input, &outputs);
    Ok(())
}

pub fn run_flatten_spots(
    input: Vec<PathBuf>,
    output: Option<PathBuf>,
    icc_profile: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    let icc_bytes = icc_profile.as_ref().map(fs::read).transpose()?;
    let params = if icc_bytes.is_some() {
        "icc=custom"
    } else {
        "icc=default"
    };
    let mut replacements = 0u32;
    let outputs = apply_pdf_mutation(
        &input,
        &output,
        overwrite,
        "flatten_spots",
        params,
        |pipeline| {
            replacements += pipeline.flatten_spots_with_icc(icc_bytes.as_deref())?;
            Ok(())
        },
    )?;
    print_outputs(&input, &outputs);
    println!("Flattened {replacements} spot-color operation(s)");
    Ok(())
}

pub fn run_outline_text(
    input: Vec<PathBuf>,
    output: Option<PathBuf>,
    overwrite: bool,
) -> rustybara::Result<()> {
    let outputs = apply_pdf_mutation(&input, &output, overwrite, "outline_text", "", |pipeline| {
        pipeline.outline_text()?;
        Ok(())
    })?;
    print_outputs(&input, &outputs);
    Ok(())
}

pub fn run_info(input: Vec<PathBuf>) -> rustybara::Result<()> {
    for (index, path) in input.iter().enumerate() {
        if index > 0 {
            println!();
        }
        let pipeline = PdfPipeline::open(path)?;
        let page_id = pipeline
            .doc()
            .get_pages()
            .values()
            .next()
            .copied()
            .ok_or_else(|| invalid_input(format!("'{}' has no pages", path.display())))?;
        let boxes = PageBoxes::read(pipeline.doc(), page_id)?;
        let color = match PdfPipeline::detect_color_space(pipeline.doc()) {
            rustybara::DocumentColorKind::PureCMYK => "Pure CMYK",
            rustybara::DocumentColorKind::PureRGB => "Pure RGB",
            rustybara::DocumentColorKind::Mixed => "Mixed",
            rustybara::DocumentColorKind::Unknown => "Unknown",
        };
        let (text_blocks, image_blocks) = pipeline.page_layout_hint(0);
        let dimensions = |rect: &rustybara::geometry::Rect| {
            format!(
                "{:.3} x {:.3} in",
                rect.width / POINTS_PER_INCH,
                rect.height / POINTS_PER_INCH
            )
        };

        println!("{}", path.display());
        println!("  Pages: {}", pipeline.page_count());
        println!("  Size: {} bytes", fs::metadata(path)?.len());
        println!("  MediaBox: {}", dimensions(&boxes.media_box));
        println!(
            "  TrimBox: {}",
            boxes
                .trim_box
                .as_ref()
                .map(dimensions)
                .unwrap_or_else(|| "not set".to_string())
        );
        println!(
            "  BleedBox: {}",
            boxes
                .bleed_box
                .as_ref()
                .map(dimensions)
                .unwrap_or_else(|| "not set".to_string())
        );
        println!("  Color: {color}");
        println!(
            "  First-page layout: {} text block(s), {} image block(s)",
            text_blocks.len(),
            image_blocks.len()
        );
        if let Some(xmp) = pipeline.read_xmp_block() {
            println!("  Rustybara version: {}", xmp.version);
            println!("  Rustybara UUID: {}", xmp.uuid);
            println!("  Processed: {}", xmp.timestamp);
            println!("  Operations: {}", xmp.ops.join(", "));
        } else {
            println!("  Rustybara metadata: none");
        }
    }
    Ok(())
}

pub fn load_local_files(path: &Path) -> Result<Vec<PathBuf>, io::Error> {
    let paths = fs::read_dir(path)?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
            {
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

    let pipeline = PdfPipeline::open(path)?;
    let doc = pipeline.doc();
    let pages = doc.get_pages();
    let first_id = pages
        .values()
        .next()
        .copied()
        .ok_or_else(|| invalid_input("PDF has no pages"))?;
    let boxes = PageBoxes::read(doc, first_id)?;

    let rect_to_arr = |rect: &rustybara::geometry::Rect| -> [f32; 4] {
        [
            rect.x as f32,
            rect.y as f32,
            rect.right() as f32,
            rect.top() as f32,
        ]
    };

    let trimbox = boxes.trim_box.as_ref().map(rect_to_arr);
    let mediabox = rect_to_arr(&boxes.media_box);
    let bleedbox = boxes.bleed_box.as_ref().map(rect_to_arr);
    let bleed_pts = match &boxes.trim_box {
        Some(trim) => (trim.x - boxes.media_box.x).abs() as f32,
        None => 0.0,
    };
    let color_space = match PdfPipeline::detect_color_space(pipeline.doc()) {
        rustybara::DocumentColorKind::PureCMYK => ColorSpaceInfo::PureCMYK,
        rustybara::DocumentColorKind::PureRGB => ColorSpaceInfo::PureRGB,
        rustybara::DocumentColorKind::Mixed => ColorSpaceInfo::Mixed,
        rustybara::DocumentColorKind::Unknown => ColorSpaceInfo::Unknown,
    };
    let file_size_kb = fs::metadata(path)
        .map(|metadata| metadata.len() / 1024)
        .unwrap_or(0);

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
    let input = app.file_paths.clone();
    let count = input.len();
    let overwrite = app.overwrite;
    let output = app.output_dir.clone();
    let mut entry = ActionLogEntry {
        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        action: app.selected_action.label().to_string(),
        status: LogStatus::Ok,
    };

    let (message, paths) = match app.selected_action {
        MenuAction::TrimMarks => {
            let paths = apply_pdf_mutation(&input, &output, overwrite, "trim", "", |pipeline| {
                pipeline.trim()?;
                Ok(())
            })?;
            (format!("Trimmed {count} file(s)"), paths)
        }
        MenuAction::ResizeToBleed => {
            let bleed = positive(app.params.bleed_pts, "bleed")?;
            let params = format!("bleed_pts={bleed}");
            let paths =
                apply_pdf_mutation(&input, &output, overwrite, "resize", &params, |pipeline| {
                    pipeline.resize(bleed)?;
                    Ok(())
                })?;
            (format!("Resized {count} file(s)"), paths)
        }
        MenuAction::AddTrimBox => {
            let bleed = positive(app.params.trim_box_bleed_pts, "trim-box inset")?;
            let params = format!("bleed_pts={bleed}");
            let paths = apply_pdf_mutation(
                &input,
                &output,
                overwrite,
                "add_trim_box",
                &params,
                |pipeline| {
                    pipeline.add_trim_box(bleed)?;
                    Ok(())
                },
            )?;
            (format!("Added TrimBox to {count} file(s)"), paths)
        }
        MenuAction::SetMediaBox => {
            let width = positive(app.params.media_width_inches, "width")? * POINTS_PER_INCH;
            let height = positive(app.params.media_height_inches, "height")? * POINTS_PER_INCH;
            let params = format!(
                "width_in={},height_in={}",
                app.params.media_width_inches, app.params.media_height_inches
            );
            let paths = apply_pdf_mutation(
                &input,
                &output,
                overwrite,
                "set_media_box",
                &params,
                |pipeline| {
                    pipeline.set_media_box(width, height)?;
                    Ok(())
                },
            )?;
            (format!("Set MediaBox on {count} file(s)"), paths)
        }
        MenuAction::Rotate => {
            let degrees = app.params.rotate_degrees;
            if degrees % 90 != 0 {
                return Err(invalid_input("rotation must be a multiple of 90"));
            }
            let params = format!("degrees={degrees}");
            let paths =
                apply_pdf_mutation(&input, &output, overwrite, "rotate", &params, |pipeline| {
                    pipeline.rotate(degrees)?;
                    Ok(())
                })?;
            (format!("Rotated {count} file(s)"), paths)
        }
        MenuAction::ExportImages => {
            let format = match app.params.export_format.as_str() {
                "png" => ImageFormat::Png,
                "webp" => ImageFormat::Webp,
                "tiff" => ImageFormat::Tiff,
                _ => ImageFormat::Jpg,
            };
            let total = export_images(
                &input,
                &output,
                format,
                app.params.export_dpi,
                app.params.export_quality,
                app.params.render_annotations,
                app.params.render_forms,
                false,
            )?;
            entry.action = format!("Export Images ({})", app.params.export_format);
            (format!("Exported {total} image(s)"), Vec::new())
        }
        MenuAction::ExtractPages => {
            validate_page_selection(&app.params.extract_pages)?;
            let params = format!("pages={}", app.params.extract_pages);
            let paths = apply_pdf_derivation(
                &input,
                &output,
                overwrite,
                "extracted",
                "extract_pages",
                &params,
                |pipeline, path| {
                    let page_indices =
                        page_indices(&app.params.extract_pages, pipeline.page_count()).map_err(
                            |_| {
                                invalid_input(format!(
                                    "page selection exceeds the {} page(s) in '{}'",
                                    pipeline.page_count(),
                                    path.display()
                                ))
                            },
                        )?;
                    pipeline.extract_pages(&page_indices)
                },
            )?;
            (format!("Extracted pages from {count} file(s)"), paths)
        }
        MenuAction::SplitPages => {
            let width = positive(app.params.panel_width_inches, "panel width")? * POINTS_PER_INCH;
            let params = format!("panel_width_in={}", app.params.panel_width_inches);
            let paths = apply_pdf_derivation(
                &input,
                &output,
                overwrite,
                "split",
                "split_pages",
                &params,
                |pipeline, _| pipeline.split_pages(width),
            )?;
            (format!("Split {count} file(s)"), paths)
        }
        MenuAction::StitchPages => {
            let width = positive(app.params.spread_width_inches, "spread width")? * POINTS_PER_INCH;
            let params = format!("spread_width_in={}", app.params.spread_width_inches);
            let paths = apply_pdf_derivation(
                &input,
                &output,
                overwrite,
                "stitch",
                "stitch_pages",
                &params,
                |pipeline, _| pipeline.stitch_pages(width),
            )?;
            (format!("Stitched {count} file(s)"), paths)
        }
        MenuAction::RemapColors => {
            let params = format!(
                "from={:?},to={:?},tolerance={}",
                app.params.remap_from, app.params.remap_to, app.params.remap_tolerance
            );
            let paths = apply_pdf_mutation(
                &input,
                &output,
                overwrite,
                "remap_color",
                &params,
                |pipeline| {
                    pipeline.remap_color(
                        app.params.remap_from,
                        app.params.remap_to,
                        app.params.remap_tolerance,
                    )?;
                    Ok(())
                },
            )?;
            (format!("Remapped {count} file(s)"), paths)
        }
        MenuAction::ConvertColorSpace => {
            let intent = app.params.rendering_intent.as_str();
            let params = format!(
                "from={},to={},intent={intent}",
                app.params.from_profile, app.params.to_profile
            );
            let paths = apply_pdf_mutation(
                &input,
                &output,
                overwrite,
                "convert_color_space",
                &params,
                |pipeline| {
                    pipeline.convert_color_space(
                        &app.params.from_profile,
                        &app.params.to_profile,
                        intent,
                    )
                },
            )?;
            (format!("Converted {count} file(s)"), paths)
        }
        MenuAction::FlattenSpots => {
            let icc = if app.params.flatten_icc_path.trim().is_empty() {
                None
            } else {
                Some(fs::read(app.params.flatten_icc_path.trim())?)
            };
            let paths = apply_pdf_mutation(
                &input,
                &output,
                overwrite,
                "flatten_spots",
                if icc.is_some() {
                    "icc=custom"
                } else {
                    "icc=default"
                },
                |pipeline| {
                    pipeline.flatten_spots_with_icc(icc.as_deref())?;
                    Ok(())
                },
            )?;
            (format!("Flattened spots in {count} file(s)"), paths)
        }
        MenuAction::OutlineText => {
            let paths =
                apply_pdf_mutation(&input, &output, overwrite, "outline_text", "", |pipeline| {
                    pipeline.outline_text()?;
                    Ok(())
                })?;
            (format!("Outlined text in {count} file(s)"), paths)
        }
        MenuAction::ToggleOverwrite
        | MenuAction::OutputPath
        | MenuAction::ChangeFiles
        | MenuAction::Quit => return Err(invalid_input("selected action is not processable")),
    };

    Ok((message, paths, entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_overwrite_is_rejected_before_opening_the_source() {
        let result = run_image(
            vec![PathBuf::from("single-page.pdf")],
            None,
            ImageFormat::Png,
            150,
            90,
            false,
            false,
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
        let result = plan_output_paths(&inputs, &None, None, false);

        assert!(result.is_ok());

        let output = std::env::current_dir().unwrap();
        let result = plan_output_paths(&inputs, &Some(output), None, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("output collision"));
    }

    #[test]
    fn page_selection_is_one_based_sorted_and_deduplicated() {
        assert_eq!(page_indices("5, 1,3-5,3", 5).unwrap(), vec![0, 2, 3, 4]);
    }

    #[test]
    fn page_selection_rejects_zero_and_reversed_ranges() {
        assert!(validate_page_selection("0").is_err());
        assert!(validate_page_selection("5-3").is_err());
        assert!(page_indices("1-1000000000", 20).is_err());
    }

    #[test]
    fn bleed_units_are_explicit_and_validated() {
        assert_eq!(bleed_points(None, Some(0.125), 9.0).unwrap(), 9.0);
        assert!(bleed_points(Some(9.0), Some(0.125), 9.0).is_err());
        assert!(bleed_points(Some(-1.0), None, 9.0).is_err());
    }

    #[test]
    fn pdf_commands_write_outputs_with_provenance() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../rustybara/tests/fixtures/pdf_test_data_print_v2.pdf");
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let output_dir = std::env::temp_dir().join(format!(
            "rustybara-rbara-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&output_dir).unwrap();

        run_rotate(vec![fixture.clone()], 90, Some(output_dir.clone()), false).unwrap();
        let output = output_path(&fixture, &Some(output_dir.clone()), None, false);
        let pipeline = PdfPipeline::open(&output).unwrap();
        let metadata = pipeline.read_xmp_block().unwrap();
        assert!(
            metadata
                .ops
                .iter()
                .any(|operation| operation.contains("rotate"))
        );

        fs::remove_file(output).unwrap();
        fs::remove_dir(output_dir).unwrap();
    }
}
