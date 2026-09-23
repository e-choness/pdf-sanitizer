use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

use crate::{SanitizationSettings, SanitizeError};

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct SanitizeReport {
    pub removed_metadata: bool,
    pub removed_xmp: bool,
    pub removed_javascript: u32,
    pub removed_actions: u32,
    pub removed_embedded_files: u32,
    pub removed_links: u32,
    pub images_recompressed: u32,
    pub fonts_subset: u32,
    pub pages: u32,
    pub input_size: u64,
    pub output_size: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "stage", rename_all = "camelCase")]
pub enum Stage {
    Loading,
    Rewriting { pct: u8 },
    Optimizing { pct: u8 },
    Saving,
    Verifying,
    Replacing,
}

pub fn sanitize(
    input: &Path,
    settings: &SanitizationSettings,
    cancel: &CancellationToken,
    progress: &mut dyn FnMut(Stage),
) -> Result<(PathBuf, SanitizeReport), SanitizeError> {
    progress(Stage::Loading);

    if !input.exists() {
        return Err(SanitizeError::NotFound);
    }

    // Check PDF header before loading
    {
        let mut header = [0u8; 5];
        let mut f = std::fs::File::open(input)?;
        let n = f.read(&mut header).unwrap_or(0);
        if n < 5 || !header.starts_with(b"%PDF-") {
            return Err(SanitizeError::NotAPdf);
        }
    }

    let input_size = std::fs::metadata(input)?.len();

    let mut doc = Document::load(input).map_err(|e| SanitizeError::Parse(e.to_string()))?;

    // Document::load already decrypts files that open with an empty user
    // password (owner-password-only / permission-restricted PDFs), so an
    // Encrypt dictionary that survives loading means a real password is needed.
    if doc.is_encrypted() {
        return Err(SanitizeError::Encrypted);
    }

    let page_count = doc.get_pages().len() as u32;
    let mut report = SanitizeReport {
        input_size,
        pages: page_count,
        ..Default::default()
    };

    if cancel.is_cancelled() {
        return Err(SanitizeError::Cancelled);
    }

    // --- Rewriting passes ---
    let pass_count = [
        settings.remove_metadata,
        settings.remove_scripts,
        settings.remove_embedded_files,
        settings.strip_external_links,
    ]
    .iter()
    .filter(|&&v| v)
    .count()
    .max(1);

    let mut done_passes = 0usize;

    if settings.remove_metadata {
        remove_metadata_pass(&mut doc, &mut report);
        done_passes += 1;
        progress(Stage::Rewriting {
            pct: ((done_passes * 80) / pass_count) as u8,
        });
        if cancel.is_cancelled() {
            return Err(SanitizeError::Cancelled);
        }
    }

    if settings.remove_scripts {
        remove_scripts_pass(&mut doc, &mut report);
        done_passes += 1;
        progress(Stage::Rewriting {
            pct: ((done_passes * 80) / pass_count) as u8,
        });
        if cancel.is_cancelled() {
            return Err(SanitizeError::Cancelled);
        }
    }

    if settings.remove_embedded_files {
        remove_embedded_files_pass(&mut doc, &mut report);
        done_passes += 1;
        progress(Stage::Rewriting {
            pct: ((done_passes * 80) / pass_count) as u8,
        });
        if cancel.is_cancelled() {
            return Err(SanitizeError::Cancelled);
        }
    }

    if settings.strip_external_links {
        strip_external_links_pass(&mut doc, &mut report);
        done_passes += 1;
        progress(Stage::Rewriting {
            pct: ((done_passes * 80) / pass_count) as u8,
        });
        if cancel.is_cancelled() {
            return Err(SanitizeError::Cancelled);
        }
    }

    // --- Optimizing passes ---
    if settings.font_subsetting {
        progress(Stage::Optimizing { pct: 0 });
        report.fonts_subset = crate::fonts::subset_fonts(&mut doc, cancel);
        if cancel.is_cancelled() {
            return Err(SanitizeError::Cancelled);
        }
    }

    if settings.compress_images {
        let image_ids = collect_image_ids(&doc);
        let total = image_ids.len().max(1);
        for (i, img_id) in image_ids.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(SanitizeError::Cancelled);
            }
            compress_image(
                &mut doc,
                *img_id,
                settings.image_quality.jpeg_quality(),
                &mut report,
            );
            progress(Stage::Optimizing {
                pct: ((i + 1) * 100 / total) as u8,
            });
        }
    } else {
        progress(Stage::Optimizing { pct: 100 });
    }

    if cancel.is_cancelled() {
        return Err(SanitizeError::Cancelled);
    }

    // --- Cleanup ---
    doc.prune_objects();
    doc.renumber_objects();
    doc.compress();

    progress(Stage::Saving);

    // Build temp path (same volume as input for atomic rename)
    let temp_path = {
        let parent = input.parent().unwrap_or(Path::new("."));
        let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("pdf");
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        parent.join(format!(".{}.sanitizing-{}.tmp", stem, ts))
    };

    doc.save(&temp_path).map_err(SanitizeError::Io)?;
    report.output_size = std::fs::metadata(&temp_path)?.len();

    Ok((temp_path, report))
}

// ---- Metadata pass ----

fn remove_metadata_pass(doc: &mut Document, report: &mut SanitizeReport) {
    let mut changed = false;

    if doc.trailer.remove(b"Info").is_some() {
        changed = true;
    }

    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for id in ids {
        match doc.objects.get_mut(&id) {
            Some(Object::Dictionary(dict)) => {
                if dict.remove(b"Metadata").is_some() {
                    changed = true;
                }
                dict.remove(b"PieceInfo");
            }
            Some(Object::Stream(stream)) => {
                if stream.dict.remove(b"Metadata").is_some() {
                    changed = true;
                }
                stream.dict.remove(b"PieceInfo");
            }
            _ => {}
        }
    }

    if changed {
        report.removed_metadata = true;
        report.removed_xmp = true;
    }
}

// ---- Scripts pass ----

/// Actions can hang off almost any dictionary (catalog, pages, annotations,
/// form fields, outline items, action chains via /Next), either as indirect
/// references or inline dictionaries. Rather than chase each known location,
/// walk every object in the file, including nested direct dictionaries.
fn remove_scripts_pass(doc: &mut Document, report: &mut SanitizeReport) {
    let dangerous: HashSet<ObjectId> = doc
        .objects
        .iter()
        .filter_map(|(&id, obj)| match obj {
            Object::Dictionary(d) if is_dangerous_action(d) => Some(id),
            _ => None,
        })
        .collect();

    for obj in doc.objects.values_mut() {
        strip_actions(obj, &dangerous, report);
    }

    // Detached action objects are now unreachable; drop them so that only
    // script bodies still referenced from somewhere unexpected remain.
    doc.prune_objects();

    for obj in doc.objects.values_mut() {
        strip_leftover_js(obj, report);
    }
}

fn is_dangerous_action(dict: &Dictionary) -> bool {
    matches!(
        dict.get(b"S"),
        Ok(Object::Name(n)) if DANGEROUS_ACTION_SUBTYPES.contains(&n.as_slice())
    )
}

fn is_dangerous_value(value: &Object, dangerous: &HashSet<ObjectId>) -> bool {
    match value {
        Object::Reference(id) => dangerous.contains(id),
        Object::Dictionary(d) => is_dangerous_action(d),
        Object::Array(arr) => arr.iter().any(|v| is_dangerous_value(v, dangerous)),
        _ => false,
    }
}

fn strip_actions(obj: &mut Object, dangerous: &HashSet<ObjectId>, report: &mut SanitizeReport) {
    let dict = match obj {
        Object::Dictionary(d) => d,
        Object::Stream(s) => &mut s.dict,
        Object::Array(arr) => {
            for v in arr.iter_mut() {
                strip_actions(v, dangerous, report);
            }
            return;
        }
        _ => return,
    };

    // Trigger dictionaries: always removed.
    for key in [b"AA".as_slice(), b"OpenAction"] {
        if dict.remove(key).is_some() {
            report.removed_actions += 1;
        }
    }
    // Document-level JavaScript name tree and XFA forms (which carry scripts).
    for key in [b"JavaScript".as_slice(), b"XFA"] {
        if dict.remove(key).is_some() {
            report.removed_javascript += 1;
        }
    }
    // Single actions and action chains: removed only when dangerous, so that
    // ordinary internal links (GoTo) keep working.
    for key in [b"A".as_slice(), b"Next"] {
        let remove = dict
            .get(key)
            .map(|v| is_dangerous_value(v, dangerous))
            .unwrap_or(false);
        if remove {
            dict.remove(key);
            report.removed_actions += 1;
        }
    }

    for (_, v) in dict.iter_mut() {
        strip_actions(v, dangerous, report);
    }
}

fn strip_leftover_js(obj: &mut Object, report: &mut SanitizeReport) {
    let dict = match obj {
        Object::Dictionary(d) => d,
        Object::Stream(s) => &mut s.dict,
        Object::Array(arr) => {
            for v in arr.iter_mut() {
                strip_leftover_js(v, report);
            }
            return;
        }
        _ => return,
    };
    if dict.remove(b"JS").is_some() {
        report.removed_javascript += 1;
    }
    for (_, v) in dict.iter_mut() {
        strip_leftover_js(v, report);
    }
}

/// True if `obj` or any dictionary nested inside it has one of `keys`.
fn has_key_deep(obj: &Object, keys: &[&[u8]]) -> bool {
    match obj {
        Object::Dictionary(d) => {
            keys.iter().any(|k| d.has(k)) || d.iter().any(|(_, v)| has_key_deep(v, keys))
        }
        Object::Stream(s) => {
            keys.iter().any(|k| s.dict.has(k)) || s.dict.iter().any(|(_, v)| has_key_deep(v, keys))
        }
        Object::Array(arr) => arr.iter().any(|v| has_key_deep(v, keys)),
        _ => false,
    }
}

fn collect_annot_ids(doc: &Document, page_id: ObjectId) -> Vec<ObjectId> {
    let annots_obj = match doc.objects.get(&page_id) {
        Some(Object::Dictionary(d)) => d.get(b"Annots").ok().cloned(),
        _ => None,
    };
    match annots_obj {
        Some(Object::Array(arr)) => arr.iter().filter_map(|o| o.as_reference().ok()).collect(),
        Some(Object::Reference(id)) => match doc.objects.get(&id) {
            Some(Object::Array(arr)) => arr.iter().filter_map(|o| o.as_reference().ok()).collect(),
            _ => vec![],
        },
        _ => vec![],
    }
}

const DANGEROUS_ACTION_SUBTYPES: &[&[u8]] = &[
    b"JavaScript",
    b"Launch",
    b"ImportData",
    b"SubmitForm",
    b"ResetForm",
    b"GoToE",
    b"GoToR",
    b"Rendition",
    b"Movie",
    b"Sound",
];

// ---- Embedded files pass ----

fn remove_embedded_files_pass(doc: &mut Document, report: &mut SanitizeReport) {
    // Remove EmbeddedFiles from Names tree
    let catalog_id = match doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|o| o.as_reference().ok())
    {
        Some(id) => id,
        None => return,
    };

    let names_id = match doc.objects.get(&catalog_id) {
        Some(Object::Dictionary(d)) => d.get(b"Names").ok().and_then(|o| o.as_reference().ok()),
        _ => None,
    };

    if let Some(nid) = names_id {
        if let Some(Object::Dictionary(names_dict)) = doc.objects.get_mut(&nid) {
            if names_dict.remove(b"EmbeddedFiles").is_some() {
                report.removed_embedded_files += 1;
            }
        }
    }

    // Remove /Collection from catalog (portfolio PDFs)
    if let Some(Object::Dictionary(catalog)) = doc.objects.get_mut(&catalog_id) {
        catalog.remove(b"Collection");
    }

    // Walk pages for FileAttachment annotations
    let page_ids: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    for page_id in page_ids {
        let annot_ids = collect_annot_ids(doc, page_id);
        let mut annots_to_remove = vec![];

        for annot_id in &annot_ids {
            let is_file_attach = match doc.objects.get(annot_id) {
                Some(Object::Dictionary(d)) => d
                    .get(b"Subtype")
                    .ok()
                    .and_then(|o| match o {
                        Object::Name(n) => Some(n.as_slice() == b"FileAttachment"),
                        _ => None,
                    })
                    .unwrap_or(false),
                _ => false,
            };
            if is_file_attach {
                annots_to_remove.push(*annot_id);
                report.removed_embedded_files += 1;
            }
        }

        if !annots_to_remove.is_empty() {
            // Remove from page's Annots array
            if let Some(Object::Dictionary(page)) = doc.objects.get_mut(&page_id) {
                if let Ok(Object::Array(annots)) = page.get_mut(b"Annots") {
                    annots.retain(|o| {
                        o.as_reference()
                            .map(|id| !annots_to_remove.contains(&id))
                            .unwrap_or(true)
                    });
                }
            }
        }
    }
}

// ---- External links pass ----

fn strip_external_links_pass(doc: &mut Document, report: &mut SanitizeReport) {
    // Remove /URI from catalog
    let catalog_id = match doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|o| o.as_reference().ok())
    {
        Some(id) => id,
        None => return,
    };

    if let Some(Object::Dictionary(catalog)) = doc.objects.get_mut(&catalog_id) {
        catalog.remove(b"URI");
    }

    // Walk pages for URI Link annotations
    let page_ids: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    for page_id in page_ids {
        let annot_ids = collect_annot_ids(doc, page_id);
        let mut annots_to_remove = vec![];

        for annot_id in &annot_ids {
            let is_external = match doc.objects.get(annot_id) {
                Some(Object::Dictionary(d)) => {
                    let is_link = d
                        .get(b"Subtype")
                        .ok()
                        .and_then(|o| match o {
                            Object::Name(n) => Some(n.as_slice() == b"Link"),
                            _ => None,
                        })
                        .unwrap_or(false);

                    if is_link {
                        let action_subtype = d.get(b"A").ok().and_then(|a| {
                            let a_id = a.as_reference().ok()?;
                            let adict = doc.objects.get(&a_id)?;
                            if let Object::Dictionary(ad) = adict {
                                if let Ok(Object::Name(s)) = ad.get(b"S") {
                                    return Some(s.clone());
                                }
                            }
                            None
                        });

                        matches!(action_subtype.as_deref(), Some(b"URI") | Some(b"GoToR"))
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if is_external {
                annots_to_remove.push(*annot_id);
                report.removed_links += 1;
            }
        }

        if !annots_to_remove.is_empty() {
            if let Some(Object::Dictionary(page)) = doc.objects.get_mut(&page_id) {
                if let Ok(Object::Array(annots)) = page.get_mut(b"Annots") {
                    annots.retain(|o| {
                        o.as_reference()
                            .map(|id| !annots_to_remove.contains(&id))
                            .unwrap_or(true)
                    });
                }
            }
        }
    }
}

// ---- Image compression pass ----

fn collect_image_ids(doc: &Document) -> Vec<ObjectId> {
    doc.objects
        .iter()
        .filter_map(|(&id, obj)| {
            let stream = match obj {
                Object::Stream(s) => s,
                _ => return None,
            };
            let subtype = stream.dict.get(b"Subtype").ok()?;
            if !matches!(subtype, Object::Name(n) if n.as_slice() == b"Image") {
                return None;
            }
            // Must be 8-bit RGB or Gray
            let bpc = stream.dict.get(b"BitsPerComponent").ok()?;
            if !matches!(bpc, Object::Integer(8)) {
                return None;
            }
            let cs = stream.dict.get(b"ColorSpace").ok()?;
            let cs_ok = match cs {
                Object::Name(n) => n.as_slice() == b"DeviceRGB" || n.as_slice() == b"DeviceGray",
                _ => false,
            };
            if !cs_ok {
                return None;
            }
            // Skip if has SMask or Mask
            if stream.dict.get(b"SMask").is_ok() || stream.dict.get(b"Mask").is_ok() {
                return None;
            }
            // Width x Height >= 64x64
            let w = match stream.dict.get(b"Width").ok()? {
                Object::Integer(n) => *n,
                _ => return None,
            };
            let h = match stream.dict.get(b"Height").ok()? {
                Object::Integer(n) => *n,
                _ => return None,
            };
            if w < 64 || h < 64 {
                return None;
            }
            Some(id)
        })
        .collect()
}

fn compress_image(doc: &mut Document, id: ObjectId, quality: u8, report: &mut SanitizeReport) {
    let stream = match doc.objects.get(&id) {
        Some(Object::Stream(s)) => s.clone(),
        _ => return,
    };

    let w = match stream.dict.get(b"Width").ok() {
        Some(Object::Integer(n)) => *n as u32,
        _ => return,
    };
    let h = match stream.dict.get(b"Height").ok() {
        Some(Object::Integer(n)) => *n as u32,
        _ => return,
    };
    let cs = match stream.dict.get(b"ColorSpace").ok() {
        Some(Object::Name(n)) => n.clone(),
        _ => return,
    };

    let channels: usize = if cs.as_slice() == b"DeviceRGB" { 3 } else { 1 };
    let expected_len = w as usize * h as usize * channels;
    let raw = match raw_image_samples(&stream) {
        Some(mut bytes) if bytes.len() >= expected_len => {
            bytes.truncate(expected_len);
            bytes
        }
        _ => return, // skip undecodable or malformed streams
    };

    // Re-encode as JPEG
    use image::codecs::jpeg::JpegEncoder;
    use image::{ColorType, ImageEncoder};

    let color_type = if cs.as_slice() == b"DeviceRGB" {
        ColorType::Rgb8
    } else {
        ColorType::L8
    };

    let mut jpeg_bytes = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, quality);
    if encoder.write_image(&raw, w, h, color_type.into()).is_err() {
        return;
    }

    // Only replace if meaningfully smaller than what is stored today
    // (< 95% of the encoded stream, not of the raw samples).
    let original_len = stream.content.len();
    if jpeg_bytes.len() >= (original_len * 95 / 100) {
        return;
    }

    // Update stream
    if let Some(Object::Stream(s)) = doc.objects.get_mut(&id) {
        s.set_content(jpeg_bytes);
        s.dict.set(b"Filter", Object::Name(b"DCTDecode".to_vec()));
        s.dict.remove(b"DecodeParms");
        report.images_recompressed += 1;
    }
}

/// Raw 8-bit samples of an image XObject. lopdf's `decompressed_content`
/// refuses image streams outright, so decode the supported cases here:
/// unfiltered, or a single FlateDecode without a predictor.
fn raw_image_samples(stream: &Stream) -> Option<Vec<u8>> {
    if !stream.dict.has(b"Filter") {
        return Some(stream.content.clone());
    }
    let filters = stream.filters().ok()?;
    if filters.len() != 1 || filters[0] != b"FlateDecode" || stream.dict.has(b"DecodeParms") {
        return None;
    }
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(stream.content.as_slice())
        .read_to_end(&mut out)
        .ok()?;
    Some(out)
}

/// Check that a content stream can be decoded. Streams without a filter are
/// plain bytes (lopdf leaves small streams uncompressed), and filters lopdf
/// cannot decode are not evidence of corruption, so both pass.
fn check_stream_decodes(stream: &Stream) -> Result<(), String> {
    if !stream.dict.has(b"Filter") {
        return Ok(());
    }
    let filters = stream.filters().map_err(|e| format!("bad /Filter: {e}"))?;
    let supported = filters
        .iter()
        .all(|f| matches!(*f, b"FlateDecode" | b"LZWDecode" | b"ASCII85Decode"));
    if !supported {
        return Ok(());
    }
    stream
        .decompressed_content()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Verify that a saved PDF is valid (used in the pipeline after saving temp)
pub fn verify_pdf(
    temp_path: &Path,
    input_page_count: u32,
    settings: &SanitizationSettings,
) -> Result<(), String> {
    let doc = Document::load(temp_path).map_err(|e| format!("failed to load: {e}"))?;

    // Must have /Root
    doc.trailer
        .get(b"Root")
        .map_err(|_| "missing /Root in trailer".to_string())?;

    // Must have /Pages
    let catalog_id = doc
        .trailer
        .get(b"Root")
        .and_then(|o| o.as_reference())
        .map_err(|_| "bad /Root ref".to_string())?;

    let catalog = doc
        .objects
        .get(&catalog_id)
        .and_then(|o| {
            if let Object::Dictionary(d) = o {
                Some(d)
            } else {
                None
            }
        })
        .ok_or("catalog not a dict")?;

    catalog
        .get(b"Pages")
        .map_err(|_| "missing /Pages in catalog".to_string())?;

    // Page count must match
    let out_pages = doc.get_pages().len() as u32;
    if out_pages != input_page_count {
        return Err(format!(
            "page count mismatch: input {input_page_count} vs output {out_pages}"
        ));
    }

    // Verify all page content streams decompress
    for (_, page_id) in doc.get_pages() {
        let page = match doc.objects.get(&page_id) {
            Some(Object::Dictionary(d)) => d,
            _ => continue,
        };
        let contents = match page.get(b"Contents") {
            Ok(Object::Reference(id)) => vec![*id],
            Ok(Object::Array(arr)) => arr.iter().filter_map(|o| o.as_reference().ok()).collect(),
            _ => vec![],
        };
        for cid in contents {
            if let Some(Object::Stream(s)) = doc.objects.get(&cid) {
                check_stream_decodes(s)
                    .map_err(|e| format!("content stream {cid:?} failed to decompress: {e}"))?;
            }
        }
    }

    // If remove_scripts was on, verify no JS/AA/OpenAction remain
    if settings.remove_scripts {
        let script_keys: &[&[u8]] = &[b"JavaScript", b"JS", b"AA", b"OpenAction"];
        if doc
            .objects
            .values()
            .any(|obj| has_key_deep(obj, script_keys))
        {
            return Err("script or trigger action still present after sanitization".to_string());
        }
    }

    Ok(())
}
