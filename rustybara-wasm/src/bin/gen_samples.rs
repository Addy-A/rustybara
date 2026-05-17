use lopdf::{Dictionary, Document, Object, Stream};
use lopdf::content::{Content, Operation};

fn obj(v: f64) -> Object { Object::Real(v as f32) }

fn bbox(r: [f64; 4]) -> Object {
    Object::Array(r.iter().map(|&v| obj(v)).collect())
}

fn op(operator: &str, operands: Vec<Object>) -> Operation {
    Operation::new(operator, operands)
}

fn build_doc(media: [f64; 4], trim: Option<[f64; 4]>, ops: Vec<Operation>) -> Document {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let content = Content { operations: ops };
    let content_bytes = content.encode().expect("content encode");
    let content_id = doc.add_object(Stream::new(Dictionary::new(), content_bytes));

    let mut page = Dictionary::new();
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(pages_id));
    page.set("MediaBox", bbox(media));
    if let Some(tb) = trim {
        page.set("TrimBox", bbox(tb));
    }
    page.set("Contents", Object::Reference(content_id));
    let page_id = doc.add_object(page);

    let mut pages = Dictionary::new();
    pages.set("Type", Object::Name(b"Pages".to_vec()));
    pages.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
    pages.set("Count", Object::Integer(1));
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", Object::Reference(pages_id));
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc
}

fn main() {
    let out_dir = std::env::args().nth(1)
        .unwrap_or_else(|| "../rustybara-website/static/samples".to_string());
    std::fs::create_dir_all(&out_dir).expect("create output dir");

    // 1. sample-with-marks.pdf — A4 with 9.5pt bleed, crop-mark lines outside TrimBox
    {
        let bleed = 9.5_f64;
        let tw = 595.0_f64;
        let th = 842.0_f64;
        let mw = tw + bleed * 2.0;
        let mh = th + bleed * 2.0;
        let ops = vec![
            op("G",  vec![obj(0.4)]),
            op("w",  vec![obj(0.5)]),
            // top-left H
            op("m", vec![obj(0.0),         obj(mh - bleed)]),
            op("l", vec![obj(bleed - 2.0), obj(mh - bleed)]),
            op("S", vec![]),
            // top-left V
            op("m", vec![obj(bleed),         obj(mh)]),
            op("l", vec![obj(bleed),         obj(mh - bleed + 2.0)]),
            op("S", vec![]),
            // top-right H
            op("m", vec![obj(mw),                 obj(mh - bleed)]),
            op("l", vec![obj(mw - bleed + 2.0),   obj(mh - bleed)]),
            op("S", vec![]),
            // top-right V
            op("m", vec![obj(mw - bleed), obj(mh)]),
            op("l", vec![obj(mw - bleed), obj(mh - bleed + 2.0)]),
            op("S", vec![]),
            // bottom-left H
            op("m", vec![obj(0.0),         obj(bleed)]),
            op("l", vec![obj(bleed - 2.0), obj(bleed)]),
            op("S", vec![]),
            // bottom-left V
            op("m", vec![obj(bleed), obj(0.0)]),
            op("l", vec![obj(bleed), obj(bleed - 2.0)]),
            op("S", vec![]),
            // bottom-right H
            op("m", vec![obj(mw),               obj(bleed)]),
            op("l", vec![obj(mw - bleed + 2.0), obj(bleed)]),
            op("S", vec![]),
            // bottom-right V
            op("m", vec![obj(mw - bleed), obj(0.0)]),
            op("l", vec![obj(mw - bleed), obj(bleed - 2.0)]),
            op("S", vec![]),
            // CMYK fill inside trim area
            op("k",  vec![obj(0.0), obj(0.6), obj(1.0), obj(0.0)]),
            op("re", vec![obj(bleed + 40.0), obj(bleed + 680.0), obj(tw - 80.0), obj(80.0)]),
            op("f",  vec![]),
        ];
        let mut doc = build_doc(
            [0.0, 0.0, mw, mh],
            Some([bleed, bleed, bleed + tw, bleed + th]),
            ops,
        );
        let out = format!("{}/sample-with-marks.pdf", out_dir);
        doc.save(&out).expect("save sample-with-marks");
        println!("Wrote {out}");
    }

    // 2. sample-bleed.pdf — A4 tight (no bleed), use resize() to expand
    {
        let ops = vec![
            op("k",  vec![obj(1.0), obj(0.5), obj(0.0), obj(0.0)]),
            op("re", vec![obj(50.0), obj(680.0), obj(495.0), obj(110.0)]),
            op("f",  vec![]),
            op("k",  vec![obj(0.0), obj(0.2), obj(0.8), obj(0.0)]),
            op("re", vec![obj(50.0), obj(50.0),  obj(495.0), obj(580.0)]),
            op("f",  vec![]),
        ];
        let mut doc = build_doc(
            [0.0, 0.0, 595.0, 842.0],
            Some([0.0, 0.0, 595.0, 842.0]),
            ops,
        );
        let out = format!("{}/sample-bleed.pdf", out_dir);
        doc.save(&out).expect("save sample-bleed");
        println!("Wrote {out}");
    }

    // 3. sample-cmyk.pdf — rich black (0 0 0 1) blocks to demonstrate remap_color
    {
        let ops = vec![
            // Pure black block
            op("k",  vec![obj(0.0), obj(0.0), obj(0.0), obj(1.0)]),
            op("re", vec![obj(50.0),  obj(550.0), obj(220.0), obj(220.0)]),
            op("f",  vec![]),
            // Slightly off-black (within 5% tolerance of pure black)
            op("k",  vec![obj(0.02), obj(0.02), obj(0.0), obj(0.97)]),
            op("re", vec![obj(325.0), obj(550.0), obj(220.0), obj(220.0)]),
            op("f",  vec![]),
            // CMYK color strip at bottom
            op("k",  vec![obj(0.0), obj(0.6), obj(1.0), obj(0.0)]),
            op("re", vec![obj(50.0), obj(100.0), obj(150.0), obj(80.0)]),
            op("f",  vec![]),
            op("k",  vec![obj(1.0), obj(0.0), obj(0.0), obj(0.0)]),
            op("re", vec![obj(222.0), obj(100.0), obj(150.0), obj(80.0)]),
            op("f",  vec![]),
            op("k",  vec![obj(0.0), obj(1.0), obj(0.0), obj(0.0)]),
            op("re", vec![obj(394.0), obj(100.0), obj(151.0), obj(80.0)]),
            op("f",  vec![]),
        ];
        let mut doc = build_doc(
            [0.0, 0.0, 595.0, 842.0],
            Some([0.0, 0.0, 595.0, 842.0]),
            ops,
        );
        let out = format!("{}/sample-cmyk.pdf", out_dir);
        doc.save(&out).expect("save sample-cmyk");
        println!("Wrote {out}");
    }
}
