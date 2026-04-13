use rustybara::encode::{save, OutputFormat};

#[test]
fn encode_jpg_embeds_dpi_in_jfif_header() {
    // JPEG JFIF APP0 header layout (bytes after SOI 0xFFD8):
    //   0xFFE0 0x0010 'J','F','I','F','\0' version(2) units(1) Xdensity(2) Ydensity(2)
    // units=0x01 means pixels per inch; density fields are big-endian u16.
    let img = image::DynamicImage::new_rgb8(10, 10);
    let dir = std::env::temp_dir().join("rustybara_test_encode");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("encode_dpi_test.jpg");
    save(&img, &path, &OutputFormat::Jpg, 300).unwrap();

    let bytes = std::fs::read(&path).unwrap();
    // SOI marker is bytes 0-1; APP0 marker starts at byte 2
    assert_eq!(&bytes[0..2], &[0xFF, 0xD8], "missing JPEG SOI");
    assert_eq!(&bytes[2..4], &[0xFF, 0xE0], "missing JFIF APP0 marker");
    // units byte is at offset 11 (2 SOI + 2 marker + 2 length + 5 identifier + 2 version = 13, units is index 13)
    assert_eq!(bytes[13], 0x01, "JFIF units should be 0x01 (pixels/inch)");
    // Xdensity big-endian u16 at bytes 14-15
    let x_density = u16::from_be_bytes([bytes[14], bytes[15]]);
    assert_eq!(x_density, 300, "JPEG Xdensity should be 300");
    // Ydensity big-endian u16 at bytes 16-17
    let y_density = u16::from_be_bytes([bytes[16], bytes[17]]);
    assert_eq!(y_density, 300, "JPEG Ydensity should be 300");

    std::fs::remove_file(&path).ok();
}

// ── OutputFormat extension tests ────────────────────────────────────
// Ported from: pdf-2-image

#[test]
fn extension_jpg() {
    assert_eq!(OutputFormat::Jpg.extension(), "jpg");
}

#[test]
fn extension_png() {
    assert_eq!(OutputFormat::Png.extension(), "png");
}

#[test]
fn extension_webp() {
    assert_eq!(OutputFormat::WebP.extension(), "webp");
}

#[test]
fn extension_tiff() {
    assert_eq!(OutputFormat::Tiff.extension(), "tiff");
}

// ── encode save tests ───────────────────────────────────────────────
// Ported from: pdf-2-image (adapted paths for rustybara)

#[test]
fn encode_save_png_writes_file() {
    let img = image::DynamicImage::new_rgb8(10, 10);
    let dir = std::env::temp_dir().join("rustybara_test_encode");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("encode_test.png");
    save(&img, &path, &OutputFormat::Png, 150).unwrap();
    assert!(path.exists());
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn encode_save_jpg_writes_file() {
    let img = image::DynamicImage::new_rgb8(10, 10);
    let dir = std::env::temp_dir().join("rustybara_test_encode");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("encode_test.jpg");
    save(&img, &path, &OutputFormat::Jpg, 150).unwrap();
    assert!(path.exists());
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn encode_save_webp_writes_file() {
    let img = image::DynamicImage::new_rgb8(10, 10);
    let dir = std::env::temp_dir().join("rustybara_test_encode");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("encode_test.webp");
    save(&img, &path, &OutputFormat::WebP, 150).unwrap();
    assert!(path.exists());
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn encode_save_tiff_writes_file() {
    let img = image::DynamicImage::new_rgb8(10, 10);
    let dir = std::env::temp_dir().join("rustybara_test_encode");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("encode_test.tiff");
    save(&img, &path, &OutputFormat::Tiff, 150).unwrap();
    assert!(path.exists());
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    std::fs::remove_file(&path).ok();
}
