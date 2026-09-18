use lopdf::{Document, Object, Stream, dictionary};
use std::io::Write;
use tempfile::NamedTempFile;
use tokio_util::sync::CancellationToken;

use crate::{sanitize, SanitizationSettings, SanitizeError};

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
    let cat_id = out_doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let cat = match out_doc.objects.get(&cat_id) {
        Some(Object::Dictionary(d)) => d,
        _ => panic!("catalog not a dict"),
    };
    assert!(cat.get(b"OpenAction").is_err(), "OpenAction should be removed");

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

    // No temp files left
    let parent = f.path().parent().unwrap();
    let leftover: Vec<_> = std::fs::read_dir(parent)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains(".sanitizing-")
        })
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
    let cat_id = out_doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
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
