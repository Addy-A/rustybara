//! Generate the deterministic RGB vector-and-image PDF used by color conversion tests.

use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, Stream, dictionary};
use std::path::PathBuf;

const RGB_IMAGE_PIXELS: &[u8] = &[
    255, 0, 0, // red
    0, 255, 0, // green
    0, 0, 255, // blue
    255, 255, 255, // white
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("rustybara-icc/tests/fixtures/rgb-vector-image.pdf"));

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut doc = Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let page_id = doc.new_object_id();

    let image_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 2_i64,
            "Height" => 2_i64,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8_i64,
            "Interpolate" => false,
        },
        RGB_IMAGE_PIXELS.to_vec(),
    ));

    let operations = vec![
        Operation::new("q", vec![]),
        Operation::new("rg", vec![1.into(), 0.into(), 0.into()]),
        Operation::new("re", vec![36.into(), 120.into(), 120.into(), 72.into()]),
        Operation::new("f", vec![]),
        Operation::new("RG", vec![0.into(), 0.into(), 1.into()]),
        Operation::new("w", vec![4.into()]),
        Operation::new("re", vec![180.into(), 120.into(), 120.into(), 72.into()]),
        Operation::new("S", vec![]),
        Operation::new("q", vec![]),
        Operation::new(
            "cm",
            vec![
                144.into(),
                0.into(),
                0.into(),
                144.into(),
                36.into(),
                300.into(),
            ],
        ),
        Operation::new("Do", vec![Object::Name(b"ImRGB".to_vec())]),
        Operation::new("Q", vec![]),
        Operation::new("Q", vec![]),
    ];
    let content_id = doc.add_object(Stream::new(
        Dictionary::new(),
        Content { operations }.encode()?,
    ));

    let mut xobjects = Dictionary::new();
    xobjects.set("ImRGB", Object::Reference(image_id));
    let resources = dictionary! {
        "ProcSet" => vec![Object::Name(b"PDF".to_vec()), Object::Name(b"ImageC".to_vec())],
        "XObject" => Object::Dictionary(xobjects),
    };

    doc.objects.insert(
        page_id,
        Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.into(), 0.into(), 360.into(), 504.into()],
            "Resources" => Object::Dictionary(resources),
            "Contents" => Object::Reference(content_id),
        }),
    );
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1_i64,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc.save(&output)?;

    println!("wrote {}", output.display());
    Ok(())
}
