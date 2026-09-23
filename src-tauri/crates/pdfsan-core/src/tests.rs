use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use std::io::Write;
use tempfile::NamedTempFile;
use tokio_util::sync::CancellationToken;

use crate::{sanitize, verify_pdf, SanitizationSettings, SanitizeError};

fn no_op_settings() -> SanitizationSettings {
    SanitizationSettings {
        remove_metadata: false,
        remove_scripts: false,
        remove_embedded_files: false,
        compress_images: false,
        strip_external_links: false,
        font_subsetting: false,
        ..Default::default()
    }
}

fn all_on_settings() -> SanitizationSettings {
    SanitizationSettings {
        remove_metadata: true,
        remove_scripts: true,
        remove_embedded_files: true,
        compress_images: false,
        strip_external_links: true,
        font_subsetting: false,
        ..Default::default()
    }
}

/// Build a minimal valid PDF document in memory and save to a temp file
fn build_minimal_pdf() -> NamedTempFile {
    let mut doc = Document::with_version("1.4");

    let pages_id = doc.new_object_id();
    let page_id = doc.new_object_id();

    // Content stream
    let content = b"BT /F1 12 Tf 100 700 Td (Hello World) Tj ET";
    let content_stream = Stream::new(dictionary! {}, content.to_vec());
    let content_id = doc.add_object(Object::Stream(content_stream));

    let page = dictionary! {
        "Type" => Object::Name(b"Page".to_vec()),
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(612), Object::Integer(792),
        ]),
        "Contents" => Object::Reference(content_id),
    };
    doc.objects.insert(page_id, Object::Dictionary(page));

    let pages = dictionary! {
        "Type" => Object::Name(b"Pages".to_vec()),
        "Kids" => Object::Array(vec![Object::Reference(page_id)]),
        "Count" => Object::Integer(1),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => Object::Name(b"Catalog".to_vec()),
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut f = NamedTempFile::with_suffix(".pdf").unwrap();
    doc.save_to(&mut f).unwrap();
    f
}

#[test]
fn not_pdf_rejected() {
    let mut f = NamedTempFile::with_suffix(".pdf").unwrap();
    f.write_all(b"This is not a PDF file at all").unwrap();
    let token = CancellationToken::new();
    let result = sanitize(f.path(), &no_op_settings(), &token, &mut |_| {});
    assert!(matches!(result, Err(SanitizeError::NotAPdf)));
}

#[test]
fn disabled_features_are_noops() {
    let f = build_minimal_pdf();
    let token = CancellationToken::new();
    let settings = no_op_settings();
    let (temp, report) = sanitize(f.path(), &settings, &token, &mut |_| {}).unwrap();

    assert_eq!(report.pages, 1);
    assert!(!report.removed_metadata);
    assert_eq!(report.removed_javascript, 0);

    // Output must be loadable
    let out_doc = Document::load(&temp).unwrap();
    assert_eq!(out_doc.get_pages().len(), 1);

    std::fs::remove_file(temp).ok();
}

#[test]
fn xref_valid() {
    let f = build_minimal_pdf();
    let token = CancellationToken::new();
    let (temp, _) = sanitize(f.path(), &all_on_settings(), &token, &mut |_| {}).unwrap();

    let doc = Document::load(&temp).unwrap();
    // Every object reachable from trailer must resolve
    let catalog_id = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    doc.get_object(catalog_id).unwrap();

    std::fs::remove_file(temp).ok();
}

#[test]
fn metadata_removed() {
    let mut doc = Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let page_id = doc.new_object_id();

    let content_stream = Stream::new(dictionary! {}, b"BT ET".to_vec());
    let content_id = doc.add_object(Object::Stream(content_stream));

    // Add Info dict with metadata
    let info_id = doc.add_object(dictionary! {
        "Author" => Object::String(b"Test Author".to_vec(), lopdf::StringFormat::Literal),
        "Title" => Object::String(b"Test Title".to_vec(), lopdf::StringFormat::Literal),
        "CreationDate" => Object::String(b"D:20240101".to_vec(), lopdf::StringFormat::Literal),
    });
    doc.trailer.set("Info", Object::Reference(info_id));

    let page = dictionary! {
        "Type" => Object::Name(b"Page".to_vec()),
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(612), Object::Integer(792),
        ]),
        "Contents" => Object::Reference(content_id),
    };
    doc.objects.insert(page_id, Object::Dictionary(page));

    let pages = dictionary! {
        "Type" => Object::Name(b"Pages".to_vec()),
        "Kids" => Object::Array(vec![Object::Reference(page_id)]),
        "Count" => Object::Integer(1),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => Object::Name(b"Catalog".to_vec()),
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut f = NamedTempFile::with_suffix(".pdf").unwrap();
    doc.save_to(&mut f).unwrap();

    let token = CancellationToken::new();
    let settings = SanitizationSettings {
        remove_metadata: true,
        ..no_op_settings()
    };
    let (temp, report) = sanitize(f.path(), &settings, &token, &mut |_| {}).unwrap();

    assert!(report.removed_metadata);

    let out_doc = Document::load(&temp).unwrap();
    assert!(out_doc.trailer.get(b"Info").is_err());

    std::fs::remove_file(temp).ok();
}

#[test]
fn javascript_removed() {
    let mut doc = Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let page_id = doc.new_object_id();

    let content_stream = Stream::new(dictionary! {}, b"BT ET".to_vec());
    let content_id = doc.add_object(Object::Stream(content_stream));

    // Add OpenAction (JS)
    let js_action_id = doc.add_object(dictionary! {
        "Type" => Object::Name(b"Action".to_vec()),
        "S" => Object::Name(b"JavaScript".to_vec()),
        "JS" => Object::String(b"app.alert('hello')".to_vec(), lopdf::StringFormat::Literal),
    });

    let page = dictionary! {
        "Type" => Object::Name(b"Page".to_vec()),
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(612), Object::Integer(792),
        ]),
        "Contents" => Object::Reference(content_id),
    };
    doc.objects.insert(page_id, Object::Dictionary(page));

    let pages = dictionary! {
        "Type" => Object::Name(b"Pages".to_vec()),
        "Kids" => Object::Array(vec![Object::Reference(page_id)]),
        "Count" => Object::Integer(1),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => Object::Name(b"Catalog".to_vec()),
        "Pages" => Object::Reference(pages_id),
        "OpenAction" => Object::Reference(js_action_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut f = NamedTempFile::with_suffix(".pdf").unwrap();
    doc.save_to(&mut f).unwrap();

    let token = CancellationToken::new();
    let settings = SanitizationSettings {
        remove_scripts: true,
        ..no_op_settings()
    };
    let (temp, report) = sanitize(f.path(), &settings, &token, &mut |_| {}).unwrap();

    assert!(report.removed_actions > 0);

    let out_doc = Document::load(&temp).unwrap();
    let cat_id = out_doc
        .trailer
        .get(b"Root")
        .unwrap()
        .as_reference()
        .unwrap();
    let cat = match out_doc.objects.get(&cat_id) {
        Some(Object::Dictionary(d)) => d,
        _ => panic!("catalog not a dict"),
    };
    assert!(
        cat.get(b"OpenAction").is_err(),
        "OpenAction should be removed"
    );

    std::fs::remove_file(temp).ok();
}

#[test]
fn binary_streams_intact() {
    let f = build_minimal_pdf();
    let token = CancellationToken::new();
    let (temp, _) = sanitize(f.path(), &all_on_settings(), &token, &mut |_| {}).unwrap();

    // Just verify the output loads — binary streams in our minimal PDF are trivial
    let out_doc = Document::load(&temp).unwrap();
    assert_eq!(out_doc.get_pages().len(), 1);

    std::fs::remove_file(temp).ok();
}

#[test]
fn cancel_mid_way() {
    let f = build_minimal_pdf();
    let token = CancellationToken::new();
    token.cancel(); // cancel before starting

    let result = sanitize(f.path(), &all_on_settings(), &token, &mut |_| {});
    assert!(matches!(result, Err(SanitizeError::Cancelled)));

    // No temp files left for this input (other tests share the temp dir and
    // run in parallel, so only look at files derived from our own stem).
    let parent = f.path().parent().unwrap();
    let prefix = format!(
        ".{}.sanitizing-",
        f.path().file_stem().unwrap().to_string_lossy()
    );
    let leftover: Vec<_> = std::fs::read_dir(parent)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .collect();
    assert!(leftover.is_empty(), "temp files left behind after cancel");
}

#[test]
fn links_removed() {
    let mut doc = Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let page_id = doc.new_object_id();

    let content_stream = Stream::new(dictionary! {}, b"BT ET".to_vec());
    let content_id = doc.add_object(Object::Stream(content_stream));

    // URI link annotation
    let uri_action_id = doc.add_object(dictionary! {
        "S" => Object::Name(b"URI".to_vec()),
        "URI" => Object::String(b"https://example.com".to_vec(), lopdf::StringFormat::Literal),
    });
    let annot_id = doc.add_object(dictionary! {
        "Type" => Object::Name(b"Annot".to_vec()),
        "Subtype" => Object::Name(b"Link".to_vec()),
        "Rect" => Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(100), Object::Integer(20),
        ]),
        "A" => Object::Reference(uri_action_id),
    });

    let page = dictionary! {
        "Type" => Object::Name(b"Page".to_vec()),
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(612), Object::Integer(792),
        ]),
        "Contents" => Object::Reference(content_id),
        "Annots" => Object::Array(vec![Object::Reference(annot_id)]),
    };
    doc.objects.insert(page_id, Object::Dictionary(page));

    let pages = dictionary! {
        "Type" => Object::Name(b"Pages".to_vec()),
        "Kids" => Object::Array(vec![Object::Reference(page_id)]),
        "Count" => Object::Integer(1),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => Object::Name(b"Catalog".to_vec()),
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut f = NamedTempFile::with_suffix(".pdf").unwrap();
    doc.save_to(&mut f).unwrap();

    let token = CancellationToken::new();
    let settings = SanitizationSettings {
        strip_external_links: true,
        ..no_op_settings()
    };
    let (temp, report) = sanitize(f.path(), &settings, &token, &mut |_| {}).unwrap();
    assert!(report.removed_links > 0);

    let out_doc = Document::load(&temp).unwrap();
    let pages_out = out_doc.get_pages();
    let out_page_id = *pages_out.values().next().unwrap();
    if let Some(Object::Dictionary(p)) = out_doc.objects.get(&out_page_id) {
        if let Ok(Object::Array(annots)) = p.get(b"Annots") {
            assert!(annots.is_empty(), "link annotation should be removed");
        }
    }

    std::fs::remove_file(temp).ok();
}

#[test]
fn embedded_files_removed() {
    let f = build_minimal_pdf();
    let token = CancellationToken::new();
    let settings = SanitizationSettings {
        remove_embedded_files: true,
        ..no_op_settings()
    };
    let (temp, _report) = sanitize(f.path(), &settings, &token, &mut |_| {}).unwrap();

    let out_doc = Document::load(&temp).unwrap();
    // Catalog should have no EmbeddedFiles in Names
    let cat_id = out_doc
        .trailer
        .get(b"Root")
        .unwrap()
        .as_reference()
        .unwrap();
    if let Some(Object::Dictionary(cat)) = out_doc.objects.get(&cat_id) {
        if let Ok(names_ref) = cat.get(b"Names") {
            if let Ok(names_id) = names_ref.as_reference() {
                if let Some(Object::Dictionary(names)) = out_doc.objects.get(&names_id) {
                    assert!(names.get(b"EmbeddedFiles").is_err());
                }
            }
        }
    }

    std::fs::remove_file(temp).ok();
}

/// Wrap a page/catalog around the given extra catalog entries and page entries.
fn build_pdf_with(
    doc: &mut Document,
    catalog_extra: lopdf::Dictionary,
    page_extra: lopdf::Dictionary,
) -> NamedTempFile {
    let pages_id = doc.new_object_id();
    let page_id = doc.new_object_id();
    let content_id = doc.add_object(Object::Stream(Stream::new(
        dictionary! {},
        b"BT ET".to_vec(),
    )));

    let mut page = dictionary! {
        "Type" => Object::Name(b"Page".to_vec()),
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(612), Object::Integer(792),
        ]),
        "Contents" => Object::Reference(content_id),
    };
    for (k, v) in page_extra.iter() {
        page.set(k.clone(), v.clone());
    }
    doc.objects.insert(page_id, Object::Dictionary(page));
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => Object::Name(b"Pages".to_vec()),
            "Kids" => Object::Array(vec![Object::Reference(page_id)]),
            "Count" => Object::Integer(1),
        }),
    );
    let mut catalog = dictionary! {
        "Type" => Object::Name(b"Catalog".to_vec()),
        "Pages" => Object::Reference(pages_id),
    };
    for (k, v) in catalog_extra.iter() {
        catalog.set(k.clone(), v.clone());
    }
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut f = NamedTempFile::with_suffix(".pdf").unwrap();
    doc.save_to(&mut f).unwrap();
    f
}

fn js_action(doc: &mut Document) -> Object {
    Object::Reference(doc.add_object(dictionary! {
        "S" => Object::Name(b"JavaScript".to_vec()),
        "JS" => Object::String(b"app.alert(1)".to_vec(), lopdf::StringFormat::Literal),
    }))
}

#[test]
fn verify_accepts_uncompressed_content_streams() {
    // Tiny content streams stay unfiltered after doc.compress(); the verifier
    // used to treat them as undecodable and reject every simple PDF.
    let f = build_minimal_pdf();
    let token = CancellationToken::new();
    for settings in [no_op_settings(), all_on_settings()] {
        let (temp, report) = sanitize(f.path(), &settings, &token, &mut |_| {}).unwrap();
        let result = verify_pdf(&temp, report.pages, &settings);
        std::fs::remove_file(&temp).ok();
        result.unwrap();
    }
}

#[test]
fn javascript_removed_everywhere_and_verifies() {
    let mut doc = Document::with_version("1.4");
    let outline_js = js_action(&mut doc);
    let names_js = js_action(&mut doc);
    let chained_js = js_action(&mut doc);
    let annot_aa_js = js_action(&mut doc);

    // Outline item with a JS action: not covered by the old catalog/page walk.
    let outlines_id = doc.new_object_id();
    let item_id = doc.add_object(dictionary! {
        "Title" => Object::String(b"x".to_vec(), lopdf::StringFormat::Literal),
        "Parent" => Object::Reference(outlines_id),
        "A" => outline_js,
    });
    doc.objects.insert(
        outlines_id,
        Object::Dictionary(dictionary! {
            "Type" => Object::Name(b"Outlines".to_vec()),
            "First" => Object::Reference(item_id),
            "Last" => Object::Reference(item_id),
            "Count" => Object::Integer(1),
        }),
    );

    // Benign GoTo link whose /Next chain runs JS.
    let annot_id = doc.add_object(dictionary! {
        "Type" => Object::Name(b"Annot".to_vec()),
        "Subtype" => Object::Name(b"Link".to_vec()),
        "Rect" => Object::Array(vec![Object::Integer(0), Object::Integer(0), Object::Integer(10), Object::Integer(10)]),
        "A" => dictionary! {
            "S" => Object::Name(b"GoTo".to_vec()),
            "D" => Object::Array(vec![Object::Integer(0)]),
            "Next" => chained_js,
        },
        "AA" => dictionary! { "E" => annot_aa_js },
    });

    let catalog_extra = dictionary! {
        "Outlines" => Object::Reference(outlines_id),
        // Direct (inline) Names dictionary: the old pass only followed references.
        "Names" => dictionary! {
            "JavaScript" => dictionary! {
                "Names" => Object::Array(vec![
                    Object::String(b"init".to_vec(), lopdf::StringFormat::Literal),
                    names_js,
                ]),
            },
        },
    };
    let page_extra = dictionary! {
        "Annots" => Object::Array(vec![Object::Reference(annot_id)]),
    };
    let f = build_pdf_with(&mut doc, catalog_extra, page_extra);

    let settings = SanitizationSettings {
        remove_scripts: true,
        ..no_op_settings()
    };
    let token = CancellationToken::new();
    let (temp, report) = sanitize(f.path(), &settings, &token, &mut |_| {}).unwrap();
    let verified = verify_pdf(&temp, report.pages, &settings);
    let out = Document::load(&temp).unwrap();
    std::fs::remove_file(&temp).ok();

    verified.expect("sanitized output must pass verification");
    assert!(report.removed_javascript >= 1);
    assert!(report.removed_actions >= 3);
    for obj in out.objects.values() {
        let dict = match obj {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => continue,
        };
        if let Ok(Object::Name(s)) = dict.get(b"S") {
            assert_ne!(s.as_slice(), b"JavaScript", "JS action survived");
        }
    }
}

#[test]
fn images_are_recompressed() {
    let mut doc = Document::with_version("1.4");
    // 128x128 RGB noise-free gradient, Flate-compressed like real-world PDFs.
    let (w, h) = (128u32, 128u32);
    let mut raw = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            raw.extend_from_slice(&[(x * 2) as u8, (y * 2) as u8, ((x + y) % 256) as u8]);
        }
    }
    let mut img = Stream::new(
        dictionary! {
            "Type" => Object::Name(b"XObject".to_vec()),
            "Subtype" => Object::Name(b"Image".to_vec()),
            "Width" => Object::Integer(w as i64),
            "Height" => Object::Integer(h as i64),
            "ColorSpace" => Object::Name(b"DeviceRGB".to_vec()),
            "BitsPerComponent" => Object::Integer(8),
        },
        raw,
    );
    // Store uncompressed so the JPEG is clearly smaller.
    img.allows_compression = false;
    let img_id = doc.add_object(Object::Stream(img));
    let page_extra = dictionary! {
        "Resources" => dictionary! {
            "XObject" => dictionary! { "Im0" => Object::Reference(img_id) },
        },
    };
    let f = build_pdf_with(&mut doc, dictionary! {}, page_extra);

    let settings = SanitizationSettings {
        compress_images: true,
        ..no_op_settings()
    };
    let token = CancellationToken::new();
    let (temp, report) = sanitize(f.path(), &settings, &token, &mut |_| {}).unwrap();
    let verified = verify_pdf(&temp, report.pages, &settings);
    std::fs::remove_file(&temp).ok();

    verified.unwrap();
    assert_eq!(report.images_recompressed, 1);
}

// ---- Font subsetting ----

use crate::fonts::{test_font, TtFont};

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

/// Embed the synthetic test font and wrap it in a font dictionary.
/// `kind` is "Type0" (Identity-H over CIDFontType2) or "TrueType" (WinAnsi).
fn add_test_font(doc: &mut Document, kind: &str) -> ObjectId {
    let data = test_font::build();
    let len = data.len() as i64;
    let program = doc.add_object(Stream::new(dictionary! { "Length1" => len }, data));
    let descriptor = doc.add_object(dictionary! {
        "Type" => name("FontDescriptor"),
        "FontName" => name("TestSans"),
        "Flags" => Object::Integer(32),
        "FontFile2" => Object::Reference(program),
    });
    if kind == "Type0" {
        let cid_font = doc.add_object(dictionary! {
            "Type" => name("Font"),
            "Subtype" => name("CIDFontType2"),
            "BaseFont" => name("TestSans"),
            "FontDescriptor" => Object::Reference(descriptor),
            "CIDToGIDMap" => name("Identity"),
        });
        doc.add_object(dictionary! {
            "Type" => name("Font"),
            "Subtype" => name("Type0"),
            "BaseFont" => name("TestSans"),
            "Encoding" => name("Identity-H"),
            "DescendantFonts" => Object::Array(vec![Object::Reference(cid_font)]),
        })
    } else {
        doc.add_object(dictionary! {
            "Type" => name("Font"),
            "Subtype" => name("TrueType"),
            "BaseFont" => name("TestSans"),
            "Encoding" => name("WinAnsiEncoding"),
            "FontDescriptor" => Object::Reference(descriptor),
        })
    }
}

fn font_resources(fonts: &[(&str, ObjectId)]) -> lopdf::Dictionary {
    let mut d = lopdf::Dictionary::new();
    for (n, id) in fonts {
        d.set(n.as_bytes().to_vec(), Object::Reference(*id));
    }
    d
}

/// Sanitize with font subsetting, verify, and return (report, output document).
fn run_subset(f: &NamedTempFile) -> (crate::SanitizeReport, Document) {
    let settings = SanitizationSettings {
        font_subsetting: true,
        ..no_op_settings()
    };
    let token = CancellationToken::new();
    let (temp, report) = sanitize(f.path(), &settings, &token, &mut |_| {}).unwrap();
    let verified = verify_pdf(&temp, report.pages, &settings);
    let out = Document::load(&temp).unwrap();
    std::fs::remove_file(&temp).ok();
    verified.unwrap();
    (report, out)
}

fn page_font<'a>(doc: &'a Document, font_name: &str) -> &'a lopdf::Dictionary {
    let page_id = *doc.get_pages().values().next().unwrap();
    let page = doc.get_dictionary(page_id).unwrap();
    let res = page.get(b"Resources").unwrap().as_dict().unwrap();
    let fonts = res.get(b"Font").unwrap().as_dict().unwrap();
    doc.get_dictionary(
        fonts
            .get(font_name.as_bytes())
            .unwrap()
            .as_reference()
            .unwrap(),
    )
    .unwrap()
}

/// Decoded font program behind the page font resource `font_name`.
fn program_of(doc: &Document, font_name: &str) -> Vec<u8> {
    let mut font = page_font(doc, font_name);
    if let Ok(desc) = font.get(b"DescendantFonts") {
        let id = desc.as_array().unwrap()[0].as_reference().unwrap();
        font = doc.get_dictionary(id).unwrap();
    }
    let fd_id = font.get(b"FontDescriptor").unwrap().as_reference().unwrap();
    let fd = doc.get_dictionary(fd_id).unwrap();
    let stream_id = fd.get(b"FontFile2").unwrap().as_reference().unwrap();
    let stream = doc.get_object(stream_id).unwrap().as_stream().unwrap();
    if stream.dict.has(b"Filter") {
        stream.decompressed_content().unwrap()
    } else {
        stream.content.clone()
    }
}

/// Which of the test font's glyphs 1..=3 still have outlines.
fn kept_glyphs(program: &[u8]) -> Vec<u16> {
    let font = TtFont::parse(program).unwrap();
    (1..=3).filter(|&g| !font.glyph(g).is_empty()).collect()
}

fn page_pdf(doc: &mut Document, resources: lopdf::Dictionary, content: &[u8]) -> NamedTempFile {
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.to_vec()));
    let page_extra = dictionary! {
        "Resources" => resources,
        "Contents" => Object::Reference(content_id),
    };
    build_pdf_with(doc, dictionary! {}, page_extra)
}

#[test]
fn type0_font_subset_keeps_used_and_component_glyphs() {
    let mut doc = Document::with_version("1.7");
    let f1 = add_test_font(&mut doc, "Type0");
    let res = dictionary! { "Font" => font_resources(&[("F1", f1)]) };
    let f = page_pdf(&mut doc, res, b"BT /F1 12 Tf 10 10 Td <0003> Tj ET");

    let (report, out) = run_subset(&f);
    assert_eq!(report.fonts_subset, 1);
    // Glyph 3 is a composite of glyph 1; glyph 2 is unused.
    assert_eq!(kept_glyphs(&program_of(&out, "F1")), vec![1, 3]);

    let base = page_font(&out, "F1")
        .get(b"BaseFont")
        .unwrap()
        .as_name()
        .unwrap();
    assert!(
        base.ends_with(b"+TestSans") && base.len() == 15,
        "subset tag missing"
    );
}

#[test]
fn simple_truetype_font_subset_via_winansi() {
    let mut doc = Document::with_version("1.7");
    let f1 = add_test_font(&mut doc, "TrueType");
    let res = dictionary! { "Font" => font_resources(&[("F1", f1)]) };
    let f = page_pdf(&mut doc, res, b"BT /F1 12 Tf [(A) 120 (A)] TJ ET");

    let (report, out) = run_subset(&f);
    assert_eq!(report.fonts_subset, 1);
    assert_eq!(kept_glyphs(&program_of(&out, "F1")), vec![1]);
}

#[test]
fn font_state_follows_q_restore_and_inline_images() {
    let mut doc = Document::with_version("1.7");
    let f1 = add_test_font(&mut doc, "TrueType");
    let f2 = add_test_font(&mut doc, "TrueType");
    let res = dictionary! { "Font" => font_resources(&[("F1", f1), ("F2", f2)]) };
    // After Q the font reverts to F1, so 'B' belongs to F1, not F2. The inline
    // image holds bytes that look like tokens and must be skipped.
    let content: &[u8] = b"BT /F1 12 Tf ET q BT /F2 12 Tf (A) Tj ET Q \
        BI /W 2 /H 1 /CS /G /BPC 8 ID (Tf EI\n BT (B) Tj ET";
    let f = page_pdf(&mut doc, res, content);

    let (report, out) = run_subset(&f);
    assert_eq!(report.fonts_subset, 2);
    assert_eq!(kept_glyphs(&program_of(&out, "F1")), vec![2]);
    assert_eq!(kept_glyphs(&program_of(&out, "F2")), vec![1]);
}

#[test]
fn form_xobject_inherits_font_and_resources() {
    let mut doc = Document::with_version("1.7");
    let f1 = add_test_font(&mut doc, "TrueType");
    // Form without its own /Resources, showing text with the invoker's font.
    let form = doc.add_object(Stream::new(
        dictionary! {
            "Type" => name("XObject"),
            "Subtype" => name("Form"),
            "BBox" => Object::Array(vec![
                Object::Integer(0), Object::Integer(0),
                Object::Integer(100), Object::Integer(100),
            ]),
        },
        b"BT (C) Tj ET".to_vec(),
    ));
    let res = dictionary! {
        "Font" => font_resources(&[("F1", f1)]),
        "XObject" => dictionary! { "Fm0" => Object::Reference(form) },
    };
    let f = page_pdf(&mut doc, res, b"/F1 12 Tf /Fm0 Do");

    let (report, out) = run_subset(&f);
    assert_eq!(report.fonts_subset, 1);
    assert_eq!(kept_glyphs(&program_of(&out, "F1")), vec![1, 3]);
}

#[test]
fn acroform_fonts_are_left_alone() {
    // Fonts in AcroForm /DR can render user-typed text: keep every glyph.
    let mut doc = Document::with_version("1.7");
    let f1 = add_test_font(&mut doc, "Type0");
    let acroform = dictionary! {
        "Fields" => Object::Array(vec![]),
        "DR" => dictionary! { "Font" => font_resources(&[("F1", f1)]) },
    };
    let content = b"BT /F1 12 Tf <0001> Tj ET".to_vec();
    let content_id = doc.add_object(Stream::new(dictionary! {}, content));
    let page_extra = dictionary! {
        "Resources" => dictionary! { "Font" => font_resources(&[("F1", f1)]) },
        "Contents" => Object::Reference(content_id),
    };
    let f = build_pdf_with(&mut doc, dictionary! { "AcroForm" => acroform }, page_extra);

    let (report, out) = run_subset(&f);
    assert_eq!(report.fonts_subset, 0);
    assert_eq!(kept_glyphs(&program_of(&out, "F1")), vec![1, 2, 3]);
}

#[test]
fn unparseable_content_disables_subsetting() {
    let mut doc = Document::with_version("1.7");
    let f1 = add_test_font(&mut doc, "Type0");
    let res = dictionary! { "Font" => font_resources(&[("F1", f1)]) };
    let f = page_pdf(&mut doc, res, b"BT /F1 12 Tf <0001> Tj ET (unterminated");

    let (report, out) = run_subset(&f);
    assert_eq!(report.fonts_subset, 0);
    assert_eq!(kept_glyphs(&program_of(&out, "F1")), vec![1, 2, 3]);
}

/// Writes `original.pdf` and `subset.pdf` built with a real font so they can
/// be rendered and compared externally:
/// PDFSAN_TEST_FONT=font.ttf PDFSAN_TEST_OUT=dir cargo test real_font_pdf_roundtrip
#[test]
fn real_font_pdf_roundtrip() {
    let (font_path, out_dir) = match (
        std::env::var("PDFSAN_TEST_FONT"),
        std::env::var("PDFSAN_TEST_OUT"),
    ) {
        (Ok(f), Ok(o)) => (f, std::path::PathBuf::from(o)),
        _ => return,
    };
    let data = std::fs::read(&font_path).unwrap();
    let font = TtFont::parse(&data).unwrap();
    let cmap = font.unicode_cmap().unwrap();

    let mut doc = Document::with_version("1.7");
    let len = data.len() as i64;
    let program = doc.add_object(Stream::new(dictionary! { "Length1" => len }, data.clone()));
    let descriptor = doc.add_object(dictionary! {
        "Type" => name("FontDescriptor"),
        "FontName" => name("RealFont"),
        "Flags" => Object::Integer(32),
        "FontBBox" => Object::Array(vec![
            Object::Integer(-500), Object::Integer(-300),
            Object::Integer(1500), Object::Integer(1000),
        ]),
        "ItalicAngle" => Object::Integer(0),
        "Ascent" => Object::Integer(900),
        "Descent" => Object::Integer(-200),
        "CapHeight" => Object::Integer(700),
        "StemV" => Object::Integer(80),
        "FontFile2" => Object::Reference(program),
    });
    // The same program is shared by a Type0 and a simple font.
    let cid_font = doc.add_object(dictionary! {
        "Type" => name("Font"),
        "Subtype" => name("CIDFontType2"),
        "BaseFont" => name("RealFont"),
        "CIDSystemInfo" => dictionary! {
            "Registry" => Object::string_literal("Adobe"),
            "Ordering" => Object::string_literal("Identity"),
            "Supplement" => Object::Integer(0),
        },
        "DW" => Object::Integer(600),
        "FontDescriptor" => Object::Reference(descriptor),
        "CIDToGIDMap" => name("Identity"),
    });
    let f1 = doc.add_object(dictionary! {
        "Type" => name("Font"),
        "Subtype" => name("Type0"),
        "BaseFont" => name("RealFont"),
        "Encoding" => name("Identity-H"),
        "DescendantFonts" => Object::Array(vec![Object::Reference(cid_font)]),
    });
    let f2 = doc.add_object(dictionary! {
        "Type" => name("Font"),
        "Subtype" => name("TrueType"),
        "BaseFont" => name("RealFont"),
        "Encoding" => name("WinAnsiEncoding"),
        "FontDescriptor" => Object::Reference(descriptor),
    });

    let cid_text = "Identity-H: The quick brown fox jumps 0123 éüß 日本語";
    let hex: String = cid_text
        .chars()
        .filter_map(|c| cmap.lookup(c as u32))
        .map(|g| format!("{g:04X}"))
        .collect();
    // WinAnsi bytes: "WinAnsi: Wörld – “quotes” €"
    let mut ansi = b"WinAnsi: W\xF6rld \x96 \x93quotes\x94 \x80".to_vec();
    ansi.retain(|&b| b != b'(' && b != b')');

    let form = doc.add_object(Stream::new(
        dictionary! {
            "Type" => name("XObject"),
            "Subtype" => name("Form"),
            "BBox" => Object::Array(vec![
                Object::Integer(0), Object::Integer(0),
                Object::Integer(612), Object::Integer(792),
            ]),
        },
        b"BT 40 600 Td (Form XObject: xyz) Tj ET".to_vec(),
    ));
    let mut content =
        format!("BT /F1 16 Tf 40 700 Td <{hex}> Tj ET\nq BT /F2 16 Tf 40 650 Td (").into_bytes();
    content.extend(&ansi);
    content.extend(b") Tj ET Q\n/F2 16 Tf /Fm0 Do\n");
    let res = dictionary! {
        "Font" => font_resources(&[("F1", f1), ("F2", f2)]),
        "XObject" => dictionary! { "Fm0" => Object::Reference(form) },
    };
    let f = page_pdf(&mut doc, res, &content);
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::copy(f.path(), out_dir.join("original.pdf")).unwrap();

    let (report, out) = run_subset(&f);
    let mut out = out;
    out.save(out_dir.join("subset.pdf")).unwrap();
    let before = std::fs::metadata(out_dir.join("original.pdf"))
        .unwrap()
        .len();
    let after = std::fs::metadata(out_dir.join("subset.pdf")).unwrap().len();
    println!(
        "fonts_subset={} original={before} subset={after}",
        report.fonts_subset
    );
    assert_eq!(report.fonts_subset, 1);
    assert!(after < before);
}
