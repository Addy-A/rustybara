#![cfg(feature = "bundled-profiles")]

use lopdf::content::Operation;
use lopdf::{Document, Object, Stream, dictionary};
use rustybara_icc::pdf::{ConversionReport, PdfColorConverter};
use rustybara_icc::{ColorTransform, RenderingIntent, profiles};

const FIXTURE: &[u8] = include_bytes!("fixtures/rgb-vector-image.pdf");
const RGB_IMAGE_PIXELS: &[u8] = &[
    255, 0, 0, // red
    0, 255, 0, // green
    0, 0, 255, // blue
    255, 255, 255, // white
];

fn srgb_to_swop() -> ColorTransform {
    let srgb = lcms2::Profile::new_srgb().icc().unwrap();
    ColorTransform::from_bytes(
        &srgb,
        &profiles::US_WEB_COATED_SWOP.bytes,
        RenderingIntent::RelativeColorimetric,
    )
    .unwrap()
}

fn page_operations(doc: &Document) -> Vec<Operation> {
    let page_id = *doc
        .get_pages()
        .values()
        .next()
        .expect("fixture has one page");
    doc.get_and_decode_page_content(page_id)
        .expect("page content decodes")
        .operations
}

fn image_stream(doc: &Document) -> &Stream {
    doc.objects
        .values()
        .find_map(|object| match object {
            Object::Stream(stream)
                if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image") =>
            {
                Some(stream)
            }
            _ => None,
        })
        .expect("fixture contains an image XObject")
}

fn image_color_space(stream: &Stream) -> &[u8] {
    stream
        .dict
        .get(b"ColorSpace")
        .and_then(Object::as_name)
        .expect("image has a direct device color space")
}

fn image_pixels(stream: &Stream) -> Vec<u8> {
    if stream.dict.get(b"Filter").is_ok() {
        stream.decompressed_content().expect("image stream decodes")
    } else {
        stream.content.clone()
    }
}

fn converted_document() -> (Document, ConversionReport) {
    let mut doc = Document::load_mem(FIXTURE).expect("fixture parses");
    let report = PdfColorConverter::new(&mut doc, srgb_to_swop())
        .convert_document()
        .expect("conversion completes");
    (doc, report)
}

#[test]
fn fixture_is_entirely_device_rgb() {
    let doc = Document::load_mem(FIXTURE).expect("fixture parses");
    let operators = page_operations(&doc);

    assert!(
        operators.iter().any(|op| op.operator == "rg"),
        "fixture needs an RGB fill"
    );
    assert!(
        operators.iter().any(|op| op.operator == "RG"),
        "fixture needs an RGB stroke"
    );
    assert!(
        !operators
            .iter()
            .any(|op| matches!(op.operator.as_str(), "k" | "K"))
    );
    assert_eq!(image_color_space(image_stream(&doc)), b"DeviceRGB");
    assert_eq!(image_pixels(image_stream(&doc)), RGB_IMAGE_PIXELS);
}

#[test]
fn rgb_page_operators_become_cmyk() {
    let (doc, _) = converted_document();
    let operators = page_operations(&doc);

    assert!(
        !operators
            .iter()
            .any(|op| matches!(op.operator.as_str(), "rg" | "RG"))
    );
    assert!(
        operators.iter().any(|op| op.operator == "k"),
        "RGB fill should become CMYK fill"
    );
    assert!(
        operators.iter().any(|op| op.operator == "K"),
        "RGB stroke should become CMYK stroke"
    );
}

#[test]
fn rgb_image_dictionary_becomes_cmyk() {
    let (doc, _) = converted_document();
    let image = image_stream(&doc);

    assert_eq!(image_color_space(image), b"DeviceCMYK");
    assert_eq!(
        image.dict.get(b"Width").and_then(Object::as_i64).unwrap(),
        2
    );
    assert_eq!(
        image.dict.get(b"Height").and_then(Object::as_i64).unwrap(),
        2
    );
    assert_eq!(
        image
            .dict
            .get(b"BitsPerComponent")
            .and_then(Object::as_i64)
            .unwrap(),
        8
    );
}

#[test]
fn rgb_image_pixels_are_icc_converted() {
    let (doc, _) = converted_document();
    let expected = srgb_to_swop().convert(RGB_IMAGE_PIXELS);
    let actual = image_pixels(image_stream(&doc));

    assert_eq!(
        actual.len(),
        2 * 2 * 4,
        "CMYK requires four bytes per pixel"
    );
    assert_eq!(actual, expected);
}

#[test]
fn conversion_report_counts_the_image() {
    let (_, report) = converted_document();

    assert_eq!(report.pages_processed, 1);
    assert_eq!(report.images_converted, 1);
    assert!(report.warnings.is_empty());
}

fn image_id(doc: &Document) -> lopdf::ObjectId {
    *doc.objects
        .iter()
        .find(|(_, object)| {
            object.as_stream().is_ok_and(|stream| {
                stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image")
            })
        })
        .unwrap()
        .0
}

#[test]
fn flate_image_is_decoded_and_reencoded_losslessly() {
    let mut doc = Document::load_mem(FIXTURE).unwrap();
    let id = image_id(&doc);
    let image = doc.get_object_mut(id).unwrap().as_stream_mut().unwrap();
    let rgb = RGB_IMAGE_PIXELS.repeat(256);
    image.dict.set("Width", 512_i64);
    image.set_plain_content(rgb.clone());
    image.compress().unwrap();
    assert_eq!(image.filters().unwrap(), vec![b"FlateDecode".as_slice()]);
    // Explicit default Decode must be removed when the channel count changes.
    image.dict.set(
        "Decode",
        vec![0.into(), 1.into(), 0.into(), 1.into(), 0.into(), 1.into()],
    );
    let report = PdfColorConverter::new(&mut doc, srgb_to_swop())
        .convert_document()
        .unwrap();
    let image = image_stream(&doc);
    assert_eq!(image_pixels(image), srgb_to_swop().convert(&rgb));
    assert_eq!(image_color_space(image), b"DeviceCMYK");
    assert!(image.dict.get(b"Decode").is_err());
    assert!(image.dict.get(b"DecodeParms").is_err());
    assert_eq!(report.images_converted, 1);
}

#[test]
fn malformed_image_fails_before_document_mutation() {
    for (key, value) in [
        ("Width", Object::Integer(0)),
        ("Width", Object::Integer(i64::MAX)),
        ("BitsPerComponent", Object::Integer(16)),
        (
            "Decode",
            Object::Array(vec![
                1.into(),
                0.into(),
                0.into(),
                1.into(),
                0.into(),
                1.into(),
            ]),
        ),
        ("Filter", Object::Name(b"DCTDecode".to_vec())),
        ("Filter", Object::Integer(42)),
        (
            "DecodeParms",
            Object::Dictionary(lopdf::dictionary! {"Predictor" => 12_i64}),
        ),
        (
            "Mask",
            Object::Array(vec![
                0.into(),
                0.into(),
                0.into(),
                0.into(),
                0.into(),
                0.into(),
            ]),
        ),
    ] {
        let mut doc = Document::load_mem(FIXTURE).unwrap();
        let id = image_id(&doc);
        doc.get_object_mut(id)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set(key, value);
        let before = format!("{:?}", doc.objects);
        assert!(
            PdfColorConverter::new(&mut doc, srgb_to_swop())
                .convert_document()
                .is_err(),
            "{key}"
        );
        assert_eq!(format!("{:?}", doc.objects), before, "{key}");
    }
    let mut doc = Document::load_mem(FIXTURE).unwrap();
    let id = image_id(&doc);
    doc.get_object_mut(id)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_plain_content(vec![0; 11]);
    let error = PdfColorConverter::new(&mut doc, srgb_to_swop())
        .convert_document()
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("expected 12 sample bytes, got 11")
    );
}

#[test]
fn image_conversion_survives_save_and_reload() {
    let (mut doc, _) = converted_document();
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    let doc = Document::load_mem(&bytes).unwrap();
    assert_eq!(doc.get_pages().len(), 1);
    assert_eq!(image_color_space(image_stream(&doc)), b"DeviceCMYK");
    assert_eq!(
        image_pixels(image_stream(&doc)),
        srgb_to_swop().convert(RGB_IMAGE_PIXELS)
    );
    let before = Document::load_mem(FIXTURE).unwrap();
    let drawing = |d: &Document| {
        page_operations(d)
            .into_iter()
            .filter(|op| !matches!(op.operator.as_str(), "rg" | "RG" | "k" | "K"))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        lopdf::content::Content {
            operations: drawing(&doc)
        }
        .encode()
        .unwrap(),
        lopdf::content::Content {
            operations: drawing(&before)
        }
        .encode()
        .unwrap()
    );
}

#[test]
fn indirect_inherited_and_nested_resources_convert_shared_image_once() {
    let mut doc = Document::load_mem(FIXTURE).unwrap();
    let page_id = *doc.get_pages().values().next().unwrap();
    let page = doc.get_object_mut(page_id).unwrap().as_dict_mut().unwrap();
    let parent = page.get(b"Parent").unwrap().as_reference().unwrap();
    let resources = page.remove(b"Resources").unwrap();
    let resources_id = doc.add_object(resources);
    // A form references the same resource dictionary, creating a resource cycle.
    let form_id = doc.add_object(Stream::new(
        lopdf::dictionary! {
            "Type" => "XObject", "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 2.into(), 2.into()],
            "Resources" => Object::Reference(resources_id),
        },
        b"q /ImRGB Do Q".to_vec(),
    ));
    let resources = doc
        .get_object_mut(resources_id)
        .unwrap()
        .as_dict_mut()
        .unwrap();
    resources
        .get_mut(b"XObject")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Form", Object::Reference(form_id));
    doc.get_object_mut(parent)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Resources", Object::Reference(resources_id));
    // Also exercise the same-space transform: a second visit would actually change pixels.
    let transform = ColorTransform::new(
        &profiles::ADOBE_RGB_1998,
        &profiles::APPLE_RGB,
        RenderingIntent::Perceptual,
    )
    .unwrap();
    let expected = transform.convert(RGB_IMAGE_PIXELS);
    let report = PdfColorConverter::new(&mut doc, transform)
        .convert_document()
        .unwrap();
    assert_eq!(report.images_converted, 1);
    assert_eq!(image_pixels(image_stream(&doc)), expected);
}

#[test]
fn unreferenced_images_and_soft_masks_are_preserved() {
    let mut doc = Document::load_mem(FIXTURE).unwrap();
    let id = image_id(&doc);
    let unused = doc.add_object(image_stream(&doc).clone());
    let mask_id = doc.add_object(Stream::new(
        lopdf::dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "ColorSpace" => "DeviceGray",
            "Width" => 2_i64, "Height" => 2_i64, "BitsPerComponent" => 8_i64,
        },
        vec![255, 0, 127, 255],
    ));
    doc.get_object_mut(id)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set("SMask", Object::Reference(mask_id));
    let mask_before = format!("{:?}", doc.get_object(mask_id).unwrap());
    let unused_before = format!("{:?}", doc.get_object(unused).unwrap());
    let report = PdfColorConverter::new(&mut doc, srgb_to_swop())
        .convert_document()
        .unwrap();
    assert_eq!(report.images_converted, 1);
    assert_eq!(
        format!("{:?}", doc.get_object(mask_id).unwrap()),
        mask_before
    );
    assert_eq!(
        format!("{:?}", doc.get_object(unused).unwrap()),
        unused_before
    );
    assert_eq!(
        doc.get_object(id)
            .unwrap()
            .as_stream()
            .unwrap()
            .dict
            .get(b"SMask")
            .unwrap()
            .as_reference()
            .unwrap(),
        mask_id
    );
}

#[test]
fn cmyk_image_can_be_converted_back_to_rgb() {
    let (mut doc, _) = converted_document();
    let source = image_pixels(image_stream(&doc));
    let srgb = lcms2::Profile::new_srgb().icc().unwrap();
    let transform = ColorTransform::from_bytes(
        &profiles::US_WEB_COATED_SWOP.bytes,
        &srgb,
        RenderingIntent::RelativeColorimetric,
    )
    .unwrap();
    let expected = transform.convert(&source);
    let report = PdfColorConverter::new(&mut doc, transform)
        .convert_document()
        .unwrap();
    assert_eq!(report.images_converted, 1);
    assert_eq!(image_color_space(image_stream(&doc)), b"DeviceRGB");
    assert_eq!(image_pixels(image_stream(&doc)), expected);
    assert_eq!(expected.len(), 12);
}

#[test]
fn image_only_reachable_through_form_is_converted() {
    let mut doc = Document::load_mem(FIXTURE).unwrap();
    let page_id = *doc.get_pages().values().next().unwrap();
    let resources = doc
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .remove(b"Resources")
        .unwrap();
    let form_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 2.into(), 2.into()],
            "Resources" => resources,
        },
        b"q /ImRGB Do Q".to_vec(),
    ));
    let xobjects_id = doc.add_object(dictionary! {"F" => Object::Reference(form_id)});
    doc.get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "Resources",
            dictionary! {"XObject" => Object::Reference(xobjects_id)},
        );
    let report = PdfColorConverter::new(&mut doc, srgb_to_swop())
        .convert_document()
        .unwrap();
    assert_eq!(report.images_converted, 1);
    assert_eq!(
        image_pixels(image_stream(&doc)),
        srgb_to_swop().convert(RGB_IMAGE_PIXELS)
    );
}

#[test]
fn failing_second_image_does_not_commit_first_image() {
    let mut doc = Document::load_mem(FIXTURE).unwrap();
    let mut bad_image = image_stream(&doc).clone();
    bad_image.set_plain_content(vec![0; 11]);
    let bad_id = doc.add_object(bad_image);
    let page_id = *doc.get_pages().values().next().unwrap();
    doc.get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"XObject")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Bad", Object::Reference(bad_id));
    let before = format!("{:?}", doc.objects);
    assert!(
        PdfColorConverter::new(&mut doc, srgb_to_swop())
            .convert_document()
            .is_err()
    );
    assert_eq!(format!("{:?}", doc.objects), before);
}
