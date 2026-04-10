use rustybara::encode::{save, OutputFormat};

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
    save(&img, &path, &OutputFormat::Png).unwrap();
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
    save(&img, &path, &OutputFormat::Jpg).unwrap();
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
    save(&img, &path, &OutputFormat::WebP).unwrap();
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
    save(&img, &path, &OutputFormat::Tiff).unwrap();
    assert!(path.exists());
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    std::fs::remove_file(&path).ok();
}
