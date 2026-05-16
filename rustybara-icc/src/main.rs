//! PDF ICC color converter — converts a CMYK (or RGB) PDF to a different color space.
//!
//! Usage:
//!   rustybara-icc <input.pdf> <output.pdf> [options]
//!
//! Options:
//!   --from <name>    Source ICC profile name (default: CoatedFOGRA39)
//!                    Any bundled profile name, e.g. USWebCoatedSWOP, UncoatedFOGRA29
//!   --to <name>      Destination profile name, or "srgb" for built-in sRGB (default: srgb)
//!   --intent <name>  Rendering intent: perceptual, relative, saturation, absolute
//!                    (default: relative)
//!
//! Examples:
//!   # Convert FOGRA39 print PDF to sRGB for screen preview
//!   rustybara-icc print.pdf screen.pdf
//!
//!   # Convert SWOP to FOGRA39
//!   rustybara-icc swop.pdf fogra39.pdf --from USWebCoatedSWOP --to CoatedFOGRA39

use rustybara_icc::pdf::PdfColorConverter;
use rustybara_icc::{ColorTransform, RenderingIntent};
use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 || args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage(&args[0]);
        process::exit(if args.len() < 3 { 1 } else { 0 });
    }

    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);

    let from_name = flag_value(&args, "--from").unwrap_or("CoatedFOGRA39");
    let to_name = flag_value(&args, "--to").unwrap_or("srgb");
    let intent_name = flag_value(&args, "--intent").unwrap_or("relative");

    let intent = match intent_name {
        "perceptual" => RenderingIntent::Perceptual,
        "relative" => RenderingIntent::RelativeColorimetric,
        "saturation" => RenderingIntent::Saturation,
        "absolute" => RenderingIntent::AbsoluteColorimetric,
        other => {
            eprintln!("error: unknown intent '{other}'; use perceptual, relative, saturation, or absolute");
            process::exit(1);
        }
    };

    let src_bytes = profile_bytes(from_name);
    let dst_bytes = profile_bytes(to_name);

    let transform = ColorTransform::from_bytes(&src_bytes, &dst_bytes, intent).unwrap_or_else(|e| {
        eprintln!("error: could not build color transform ({from_name} → {to_name}): {e}");
        process::exit(1);
    });

    eprintln!(
        "converting: {} [{:?}] → {} [{:?}], intent={}",
        from_name,
        transform.input_color_space(),
        to_name,
        transform.output_color_space(),
        intent_name,
    );

    let mut doc = lopdf::Document::load(&input).unwrap_or_else(|e| {
        eprintln!("error: could not load '{}': {e}", input.display());
        process::exit(1);
    });

    let report = PdfColorConverter::new(&mut doc, transform)
        .convert_document()
        .unwrap_or_else(|e| {
            eprintln!("error: conversion failed: {e}");
            process::exit(1);
        });

    doc.save(&output).unwrap_or_else(|e| {
        eprintln!("error: could not save '{}': {e}", output.display());
        process::exit(1);
    });

    println!(
        "done: {} page(s), {} spot color(s) flattened → {}",
        report.pages_processed,
        report.spot_colors_flattened,
        output.display()
    );
}

fn profile_bytes(name: &str) -> Vec<u8> {
    if name.eq_ignore_ascii_case("srgb") {
        return lcms2::Profile::new_srgb().icc().unwrap_or_else(|e| {
            eprintln!("error: could not build built-in sRGB profile: {e}");
            process::exit(1);
        });
    }
    match rustybara_icc::profiles::by_name(name) {
        Some(p) => p.bytes.to_vec(),
        None => {
            eprintln!("error: unknown profile '{name}'");
            eprintln!("bundled profiles: CoatedFOGRA27, CoatedFOGRA39, CoatedGRACoL2006,");
            eprintln!("  JapanColor2001Coated, JapanColor2001Uncoated, JapanColor2002Newspaper,");
            eprintln!("  JapanColor2003WebCoated, JapanWebCoated, UncoatedFOGRA29,");
            eprintln!("  USWebCoatedSWOP, USWebUncoated, WebCoatedFOGRA28,");
            eprintln!("  WebCoatedSWOP2006Grade3, WebCoatedSWOP2006Grade5,");
            eprintln!("  AdobeRGB1998, AppleRGB, ColorMatchRGB, PAL_SECAM,");
            eprintln!("  SMPTE-C, VideoHD, VideoNTSC, VideoPAL, srgb");
            process::exit(1);
        }
    }
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].as_str())
}

fn print_usage(program: &str) {
    eprintln!("Usage: {program} <input.pdf> <output.pdf> [options]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --from <name>    Source ICC profile (default: CoatedFOGRA39)");
    eprintln!("  --to <name>      Destination profile or 'srgb' (default: srgb)");
    eprintln!("  --intent <name>  perceptual|relative|saturation|absolute (default: relative)");
    eprintln!();
    eprintln!("Example:");
    eprintln!("  {program} print.pdf screen.pdf --from USWebCoatedSWOP");
}
