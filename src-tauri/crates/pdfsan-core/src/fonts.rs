//! Font subsetting for embedded TrueType programs (`/FontFile2`).
//!
//! Glyph IDs are preserved: unused glyphs get empty outlines instead of being
//! renumbered, so content streams, `/W` width arrays and `/CIDToGIDMap` stay
//! valid without rewriting. Supported fonts are `Type0` with an Identity
//! encoding over `CIDFontType2`, and simple `TrueType` fonts with a standard
//! encoding. Anything whose glyph usage cannot be determined with certainty is
//! left untouched: unparseable content aborts subsetting for the whole
//! document, and an unsupported font blocks every font sharing its program.

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Read;
use std::rc::Rc;
use tokio_util::sync::CancellationToken;

/// Subset every eligible embedded TrueType program in `doc`.
/// Returns the number of font programs that were rewritten.
pub fn subset_fonts(doc: &mut Document, cancel: &CancellationToken) -> u32 {
    let plan = match plan_subsets(doc, cancel) {
        Some(p) => p,
        None => return 0,
    };

    let mut count = 0;
    for item in plan {
        if let Some(Object::Stream(s)) = doc.objects.get_mut(&item.program) {
            let len = item.data.len() as i64;
            s.set_content(item.data);
            s.dict.remove(b"Filter");
            s.dict.remove(b"DecodeParms");
            s.dict.set("Length1", Object::Integer(len));
            count += 1;
        }
        for (id, key) in item.renames {
            if let Some(Object::Dictionary(d)) = doc.objects.get_mut(&id) {
                let name = match d.get(key) {
                    Ok(Object::Name(n)) if !is_subset_tagged(n) => n.clone(),
                    _ => continue,
                };
                let mut tagged = item.tag.to_vec();
                tagged.push(b'+');
                tagged.extend(name);
                d.set(key, Object::Name(tagged));
            }
        }
    }
    count
}

struct SubsetPlan {
    program: ObjectId,
    data: Vec<u8>,
    tag: [u8; 6],
    renames: Vec<(ObjectId, &'static [u8])>,
}

fn plan_subsets(doc: &Document, cancel: &CancellationToken) -> Option<Vec<SubsetPlan>> {
    let mut scanner = Scanner::new(doc);
    scanner.classify_all();
    scanner.block_form_fonts();
    scanner.scan_document().ok()?;

    let mut plans = vec![];
    let programs: BTreeSet<ObjectId> = scanner.programs.keys().copied().collect();
    for program in programs {
        if cancel.is_cancelled() || scanner.blocked.contains(&program) {
            continue;
        }
        let stream = match doc.objects.get(&program) {
            Some(Object::Stream(s)) => s,
            _ => continue,
        };
        let bytes = match stream_bytes(stream) {
            Some(b) => b,
            None => continue,
        };
        let font = match TtFont::parse(&bytes) {
            Some(f) => f,
            None => continue,
        };

        let mut keep: BTreeSet<u16> = BTreeSet::new();
        keep.insert(0);
        if let Some(gids) = scanner.cid_gids.get(&program) {
            keep.extend(gids);
        }
        let mut resolvable = true;
        if let Some(by_enc) = scanner.simple_codes.get(&program) {
            'outer: for (enc, codes) in by_enc {
                for &code in codes {
                    match simple_code_to_gids(&font, *enc, code) {
                        Some(gids) => keep.extend(gids),
                        None => {
                            resolvable = false;
                            break 'outer;
                        }
                    }
                }
            }
        }
        if !resolvable {
            continue;
        }

        let data = match subset_truetype(&font, &keep) {
            // Compare decoded sizes; the stream is recompressed on save.
            Some(d) if d.len() < bytes.len() => d,
            _ => continue,
        };
        plans.push(SubsetPlan {
            program,
            data,
            tag: subset_tag(program, &keep),
            renames: scanner.programs[&program].clone(),
        });
    }
    Some(plans)
}

// ---------------------------------------------------------------------------
// Font classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SimpleEnc {
    /// No /Encoding: the font's built-in encoding (StandardEncoding for
    /// non-symbolic fonts).
    Builtin,
    WinAnsi,
    MacRoman,
}

#[derive(Debug, Clone)]
enum CidMap {
    Identity,
    Table(Rc<Vec<u16>>),
}

impl CidMap {
    fn gid(&self, cid: u16) -> u16 {
        match self {
            CidMap::Identity => cid,
            CidMap::Table(t) => t.get(cid as usize).copied().unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone)]
enum FontKind {
    /// Not an embedded TrueType program; nothing to subset.
    Other,
    Cid {
        program: ObjectId,
        map: CidMap,
    },
    Simple {
        program: ObjectId,
        enc: SimpleEnc,
    },
    /// Embeds a TrueType program whose glyph usage we cannot map.
    Unsupported(ObjectId),
}

fn deref<'a>(doc: &'a Document, mut obj: &'a Object) -> Option<&'a Object> {
    for _ in 0..16 {
        match obj {
            Object::Reference(id) => obj = doc.objects.get(id)?,
            other => return Some(other),
        }
    }
    None
}

fn deref_dict<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Dictionary> {
    match deref(doc, obj)? {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    }
}

fn name_of<'a>(dict: &'a Dictionary, key: &[u8]) -> Option<&'a [u8]> {
    match dict.get(key) {
        Ok(Object::Name(n)) => Some(n.as_slice()),
        _ => None,
    }
}

fn is_subset_tagged(name: &[u8]) -> bool {
    name.len() > 7 && name[6] == b'+' && name[..6].iter().all(|c| c.is_ascii_uppercase())
}

/// Returns the kind plus the (object id, key) pairs whose font name should be
/// tagged once the program is subset.
fn classify(
    doc: &Document,
    font_id: Option<ObjectId>,
    font: &Dictionary,
) -> (FontKind, Vec<(ObjectId, &'static [u8])>) {
    let mut renames = vec![];
    if let Some(id) = font_id {
        renames.push((id, b"BaseFont".as_slice()));
    }
    let tagged = name_of(font, b"BaseFont").is_some_and(is_subset_tagged);

    match name_of(font, b"Subtype") {
        Some(b"TrueType") => {
            let (program, desc_id) = match truetype_program(doc, font) {
                Some(p) => p,
                None => return (FontKind::Other, vec![]),
            };
            if let Some(d) = desc_id {
                renames.push((d, b"FontName".as_slice()));
            }
            if tagged {
                return (FontKind::Unsupported(program), vec![]);
            }
            let enc = match font.get(b"Encoding").ok().and_then(|o| deref(doc, o)) {
                None => Some(SimpleEnc::Builtin),
                Some(Object::Name(n)) => simple_enc_name(n),
                Some(Object::Dictionary(d)) => {
                    if d.has(b"Differences") {
                        None
                    } else {
                        match name_of(d, b"BaseEncoding") {
                            None => Some(SimpleEnc::Builtin),
                            Some(n) => simple_enc_name(n),
                        }
                    }
                }
                Some(_) => None,
            };
            match enc {
                Some(enc) => (FontKind::Simple { program, enc }, renames),
                None => (FontKind::Unsupported(program), vec![]),
            }
        }
        Some(b"Type0") => {
            let descendant_obj = match font
                .get(b"DescendantFonts")
                .ok()
                .and_then(|o| deref(doc, o))
            {
                Some(Object::Array(arr)) if !arr.is_empty() => &arr[0],
                _ => return (FontKind::Other, vec![]),
            };
            let cid_font = match deref_dict(doc, descendant_obj) {
                Some(d) => d,
                None => return (FontKind::Other, vec![]),
            };
            if name_of(cid_font, b"Subtype") != Some(b"CIDFontType2") {
                return (FontKind::Other, vec![]);
            }
            let (program, desc_id) = match truetype_program(doc, cid_font) {
                Some(p) => p,
                None => return (FontKind::Other, vec![]),
            };
            if let Object::Reference(id) = descendant_obj {
                renames.push((*id, b"BaseFont".as_slice()));
            }
            if let Some(d) = desc_id {
                renames.push((d, b"FontName".as_slice()));
            }
            let cid_tagged = name_of(cid_font, b"BaseFont").is_some_and(is_subset_tagged);
            let identity = matches!(
                name_of(font, b"Encoding"),
                Some(b"Identity-H") | Some(b"Identity-V")
            );
            if tagged || cid_tagged || !identity {
                return (FontKind::Unsupported(program), vec![]);
            }
            let map = match cid_font
                .get(b"CIDToGIDMap")
                .ok()
                .and_then(|o| deref(doc, o))
            {
                None => Some(CidMap::Identity),
                Some(Object::Name(n)) if n.as_slice() == b"Identity" => Some(CidMap::Identity),
                Some(Object::Stream(s)) => stream_bytes(s).map(|b| {
                    CidMap::Table(Rc::new(
                        b.as_chunks::<2>()
                            .0
                            .iter()
                            .map(|c| u16::from_be_bytes(*c))
                            .collect(),
                    ))
                }),
                Some(_) => None,
            };
            match map {
                Some(map) => (FontKind::Cid { program, map }, renames),
                None => (FontKind::Unsupported(program), vec![]),
            }
        }
        _ => (FontKind::Other, vec![]),
    }
}

fn simple_enc_name(n: &[u8]) -> Option<SimpleEnc> {
    match n {
        b"WinAnsiEncoding" => Some(SimpleEnc::WinAnsi),
        b"MacRomanEncoding" => Some(SimpleEnc::MacRoman),
        b"StandardEncoding" => Some(SimpleEnc::Builtin),
        _ => None,
    }
}

/// The `/FontFile2` stream id of a font dict's descriptor, plus the
/// descriptor's own id when it is indirect.
fn truetype_program(doc: &Document, font: &Dictionary) -> Option<(ObjectId, Option<ObjectId>)> {
    let desc_obj = font.get(b"FontDescriptor").ok()?;
    let desc_id = desc_obj.as_reference().ok();
    let desc = deref_dict(doc, desc_obj)?;
    let program = desc.get(b"FontFile2").ok()?.as_reference().ok()?;
    Some((program, desc_id))
}

// ---------------------------------------------------------------------------
// Usage scanning
// ---------------------------------------------------------------------------

struct Scanner<'a> {
    doc: &'a Document,
    kinds: HashMap<ObjectId, FontKind>,
    /// Every TrueType program seen, with the names to tag once subset.
    programs: HashMap<ObjectId, Vec<(ObjectId, &'static [u8])>>,
    blocked: HashSet<ObjectId>,
    cid_gids: HashMap<ObjectId, BTreeSet<u16>>,
    simple_codes: HashMap<ObjectId, BTreeMap<SimpleEnc, BTreeSet<u8>>>,
    /// (form id, inherited resources address, inherited font) already scanned.
    visited: HashSet<FormScanKey>,
    reached_forms: HashSet<ObjectId>,
}

type ScanResult = Result<(), ()>;

/// (form id, inherited resources address, inherited font program + variant).
type FormScanKey = (ObjectId, usize, Option<(ObjectId, usize)>);

const MAX_DEPTH: usize = 32;

impl<'a> Scanner<'a> {
    fn new(doc: &'a Document) -> Self {
        Scanner {
            doc,
            kinds: HashMap::new(),
            programs: HashMap::new(),
            blocked: HashSet::new(),
            cid_gids: HashMap::new(),
            simple_codes: HashMap::new(),
            visited: HashSet::new(),
            reached_forms: HashSet::new(),
        }
    }

    fn register(&mut self, kind: &FontKind, renames: Vec<(ObjectId, &'static [u8])>) {
        match kind {
            FontKind::Cid { program, .. } | FontKind::Simple { program, .. } => {
                self.programs.entry(*program).or_default().extend(renames);
            }
            FontKind::Unsupported(program) => {
                self.programs.entry(*program).or_default();
                self.blocked.insert(*program);
            }
            FontKind::Other => {}
        }
    }

    /// Classify every indirect font dictionary, used or not, so that a program
    /// shared with an unsupported font is blocked up front.
    fn classify_all(&mut self) {
        let doc = self.doc;
        for (&id, obj) in &doc.objects {
            if let Object::Dictionary(d) = obj {
                if name_of(d, b"Type") == Some(b"Font") {
                    let (kind, renames) = classify(doc, Some(id), d);
                    self.register(&kind, renames);
                    self.kinds.insert(id, kind);
                }
            }
        }
    }

    /// Form fields render user-typed text with fonts from the AcroForm /DR
    /// dictionary, so those fonts must keep every glyph.
    fn block_form_fonts(&mut self) {
        let doc = self.doc;
        let fonts = doc
            .trailer
            .get(b"Root")
            .ok()
            .and_then(|o| deref_dict(doc, o))
            .and_then(|c| c.get(b"AcroForm").ok())
            .and_then(|o| deref_dict(doc, o))
            .and_then(|f| f.get(b"DR").ok())
            .and_then(|o| deref_dict(doc, o))
            .and_then(|dr| dr.get(b"Font").ok())
            .and_then(|o| deref_dict(doc, o));
        if let Some(fonts) = fonts {
            for (_, f) in fonts.iter() {
                if let Some(d) = deref_dict(doc, f) {
                    let program = match name_of(d, b"Subtype") {
                        Some(b"Type0") => d
                            .get(b"DescendantFonts")
                            .ok()
                            .and_then(|o| deref(doc, o))
                            .and_then(|o| o.as_array().ok())
                            .and_then(|a| a.first())
                            .and_then(|o| deref_dict(doc, o))
                            .and_then(|cf| truetype_program(doc, cf)),
                        _ => truetype_program(doc, d),
                    };
                    if let Some((p, _)) = program {
                        self.programs.entry(p).or_default();
                        self.blocked.insert(p);
                    }
                }
            }
        }
    }

    fn scan_document(&mut self) -> ScanResult {
        let doc = self.doc;
        for (_, page_id) in doc.get_pages() {
            let page = match doc.objects.get(&page_id) {
                Some(Object::Dictionary(d)) => d,
                _ => return Err(()),
            };
            let resources = inherited_resources(doc, page);
            let mut data = Vec::new();
            match page.get(b"Contents").ok().and_then(|o| deref(doc, o)) {
                None => {}
                Some(Object::Stream(s)) => data = stream_bytes(s).ok_or(())?,
                Some(Object::Array(arr)) => {
                    for part in arr {
                        match deref(doc, part) {
                            Some(Object::Stream(s)) => {
                                data.extend(stream_bytes(s).ok_or(())?);
                                data.push(b'\n');
                            }
                            _ => return Err(()),
                        }
                    }
                }
                Some(_) => return Err(()),
            }
            self.scan(&data, resources, None, 0)?;
        }

        // Content not reachable from page contents: annotation appearances,
        // unreferenced forms, tiling patterns and Type3 glyph procedures.
        for (&id, obj) in &doc.objects {
            match obj {
                Object::Stream(s)
                    if (name_of(&s.dict, b"Subtype") == Some(b"Form")
                        && !self.reached_forms.contains(&id))
                        || matches!(s.dict.get(b"PatternType"), Ok(Object::Integer(1))) =>
                {
                    let res = s
                        .dict
                        .get(b"Resources")
                        .ok()
                        .and_then(|o| deref_dict(doc, o));
                    self.scan(&stream_bytes(s).ok_or(())?, res, None, 0)?;
                }
                Object::Dictionary(d) if name_of(d, b"Subtype") == Some(b"Type3") => {
                    let res = d.get(b"Resources").ok().and_then(|o| deref_dict(doc, o));
                    if let Some(procs) = d.get(b"CharProcs").ok().and_then(|o| deref_dict(doc, o)) {
                        for (_, p) in procs.iter() {
                            match deref(doc, p) {
                                Some(Object::Stream(s)) => {
                                    self.scan(&stream_bytes(s).ok_or(())?, res, None, 0)?
                                }
                                _ => return Err(()),
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// `font` is the text font in effect when the stream starts: a form
    /// XObject inherits the graphics state (including Tf) of its invoker.
    fn scan(
        &mut self,
        data: &[u8],
        res: Option<&'a Dictionary>,
        mut font: Option<FontKind>,
        depth: usize,
    ) -> ScanResult {
        if depth > MAX_DEPTH {
            return Err(());
        }
        let doc = self.doc;
        // q/Q save and restore the graphics state, which includes the font.
        let mut saved: Vec<Option<FontKind>> = Vec::new();
        for (op, operands) in parse_ops(data)? {
            match op.as_slice() {
                b"q" => saved.push(font.clone()),
                b"Q" => font = saved.pop().ok_or(())?,
                b"Tf" => {
                    let name = match operands.first() {
                        Some(Tok::Name(n)) => n,
                        _ => return Err(()),
                    };
                    font = Some(self.font_kind(res, name)?);
                }
                b"Tj" | b"'" | b"\"" => match (&font, operands.last()) {
                    (Some(f), Some(Tok::Str(s))) => self.show(f, s),
                    _ => return Err(()),
                },
                b"TJ" => match (&font, operands.last()) {
                    (Some(f), Some(Tok::Array(items))) => {
                        for item in items {
                            if let Tok::Str(s) = item {
                                self.show(f, s);
                            }
                        }
                    }
                    _ => return Err(()),
                },
                b"gs" => {
                    // An ExtGState can switch fonts without a Tf operator.
                    let name = match operands.first() {
                        Some(Tok::Name(n)) => n,
                        _ => return Err(()),
                    };
                    let gs = lookup_resource(doc, res, b"ExtGState", name).ok_or(())?;
                    if deref_dict(doc, gs).ok_or(())?.has(b"Font") {
                        return Err(());
                    }
                }
                b"Do" => {
                    let name = match operands.first() {
                        Some(Tok::Name(n)) => n,
                        _ => return Err(()),
                    };
                    let xobj = lookup_resource(doc, res, b"XObject", name).ok_or(())?;
                    let id = xobj.as_reference().map_err(|_| ())?;
                    let stream = match doc.objects.get(&id) {
                        Some(Object::Stream(s)) => s,
                        _ => return Err(()),
                    };
                    if name_of(&stream.dict, b"Subtype") != Some(b"Form") {
                        continue;
                    }
                    let own = stream
                        .dict
                        .get(b"Resources")
                        .ok()
                        .and_then(|o| deref_dict(doc, o));
                    let form_res = own.or(res);
                    let key = (id, form_res.map_or(0, |r| r as *const Dictionary as usize));
                    self.reached_forms.insert(id);
                    // The inherited font only matters for streams that show
                    // text without their own Tf; rescanning per font keeps
                    // that case exact.
                    let font_key = match &font {
                        Some(FontKind::Simple { program, enc }) => {
                            Some((*program, *enc as usize + 1))
                        }
                        Some(FontKind::Cid {
                            program,
                            map: CidMap::Identity,
                        }) => Some((*program, 0)),
                        Some(FontKind::Cid {
                            program,
                            map: CidMap::Table(t),
                        }) => Some((*program, Rc::as_ptr(t) as usize)),
                        _ => None,
                    };
                    if self.visited.insert((key.0, key.1, font_key)) {
                        self.scan(
                            &stream_bytes(stream).ok_or(())?,
                            form_res,
                            font.clone(),
                            depth + 1,
                        )?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn font_kind(&mut self, res: Option<&'a Dictionary>, name: &[u8]) -> Result<FontKind, ()> {
        let doc = self.doc;
        let obj = lookup_resource(doc, res, b"Font", name).ok_or(())?;
        if let Object::Reference(id) = obj {
            if let Some(k) = self.kinds.get(id) {
                return Ok(k.clone());
            }
            let dict = deref_dict(doc, obj).ok_or(())?;
            let (kind, renames) = classify(doc, Some(*id), dict);
            self.register(&kind, renames);
            self.kinds.insert(*id, kind.clone());
            return Ok(kind);
        }
        let dict = deref_dict(doc, obj).ok_or(())?;
        let (kind, renames) = classify(doc, None, dict);
        self.register(&kind, renames);
        Ok(kind)
    }

    fn show(&mut self, font: &FontKind, s: &[u8]) {
        match font {
            FontKind::Cid { program, map } => {
                let gids = self.cid_gids.entry(*program).or_default();
                for ch in s.chunks(2) {
                    let cid = if ch.len() == 2 {
                        u16::from_be_bytes([ch[0], ch[1]])
                    } else {
                        ch[0] as u16
                    };
                    gids.insert(map.gid(cid));
                }
            }
            FontKind::Simple { program, enc } => {
                self.simple_codes
                    .entry(*program)
                    .or_default()
                    .entry(*enc)
                    .or_default()
                    .extend(s.iter().copied());
            }
            FontKind::Other | FontKind::Unsupported(_) => {}
        }
    }
}

fn inherited_resources<'a>(doc: &'a Document, page: &'a Dictionary) -> Option<&'a Dictionary> {
    let mut node = page;
    for _ in 0..64 {
        if let Some(r) = node.get(b"Resources").ok().and_then(|o| deref_dict(doc, o)) {
            return Some(r);
        }
        node = node.get(b"Parent").ok().and_then(|o| deref_dict(doc, o))?;
    }
    None
}

fn lookup_resource<'a>(
    doc: &'a Document,
    res: Option<&'a Dictionary>,
    category: &[u8],
    name: &[u8],
) -> Option<&'a Object> {
    let cat = deref_dict(doc, res?.get(category).ok()?)?;
    cat.get(name).ok()
}

/// Decoded bytes of a stream, for the filters lopdf can decode.
fn stream_bytes(s: &Stream) -> Option<Vec<u8>> {
    if !s.dict.has(b"Filter") {
        return Some(s.content.clone());
    }
    let filters = s.filters().ok()?;
    if filters == [b"FlateDecode".as_slice()] && !s.dict.has(b"DecodeParms") {
        // Decode directly: lopdf refuses some stream subtypes.
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(s.content.as_slice())
            .read_to_end(&mut out)
            .ok()?;
        return Some(out);
    }
    if filters
        .iter()
        .all(|f| matches!(*f, b"FlateDecode" | b"LZWDecode" | b"ASCII85Decode"))
    {
        return s.decompressed_content().ok();
    }
    None
}

// ---------------------------------------------------------------------------
// Content stream tokenizer
// ---------------------------------------------------------------------------

/// Operand values. Only the shapes the scanner inspects are kept.
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Other,
    Name(Vec<u8>),
    Str(Vec<u8>),
    Array(Vec<Tok>),
}

fn is_ws(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

enum Lex {
    Operand(Tok),
    ArrayStart,
    ArrayEnd,
    DictStart,
    DictEnd,
    Keyword(Vec<u8>),
}

struct Lexer<'d> {
    d: &'d [u8],
    p: usize,
}

impl Lexer<'_> {
    fn skip_ws(&mut self) {
        while self.p < self.d.len() {
            let b = self.d[self.p];
            if is_ws(b) {
                self.p += 1;
            } else if b == b'%' {
                while self.p < self.d.len() && !matches!(self.d[self.p], b'\r' | b'\n') {
                    self.p += 1;
                }
            } else {
                break;
            }
        }
    }

    fn next(&mut self) -> Result<Option<Lex>, ()> {
        self.skip_ws();
        let d = self.d;
        if self.p >= d.len() {
            return Ok(None);
        }
        let b = d[self.p];
        let lex = match b {
            b'(' => Lex::Operand(Tok::Str(self.literal_string()?)),
            b'<' if d.get(self.p + 1) == Some(&b'<') => {
                self.p += 2;
                Lex::DictStart
            }
            b'<' => Lex::Operand(Tok::Str(self.hex_string()?)),
            b'>' if d.get(self.p + 1) == Some(&b'>') => {
                self.p += 2;
                Lex::DictEnd
            }
            b'[' => {
                self.p += 1;
                Lex::ArrayStart
            }
            b']' => {
                self.p += 1;
                Lex::ArrayEnd
            }
            b'/' => {
                self.p += 1;
                Lex::Operand(Tok::Name(self.name()))
            }
            b')' | b'>' | b'{' | b'}' => return Err(()),
            _ => {
                let start = self.p;
                while self.p < d.len() && !is_ws(d[self.p]) && !is_delim(d[self.p]) {
                    self.p += 1;
                }
                let word = &d[start..self.p];
                match word {
                    b"true" | b"false" | b"null" => Lex::Operand(Tok::Other),
                    _ if matches!(word[0], b'0'..=b'9' | b'+' | b'-' | b'.') => {
                        if !word
                            .iter()
                            .all(|c| matches!(c, b'0'..=b'9' | b'+' | b'-' | b'.'))
                        {
                            return Err(());
                        }
                        Lex::Operand(Tok::Other)
                    }
                    _ => Lex::Keyword(word.to_vec()),
                }
            }
        };
        Ok(Some(lex))
    }

    fn name(&mut self) -> Vec<u8> {
        let d = self.d;
        let mut out = Vec::new();
        while self.p < d.len() && !is_ws(d[self.p]) && !is_delim(d[self.p]) {
            if d[self.p] == b'#' && self.p + 2 < d.len() {
                if let Ok(v) =
                    u8::from_str_radix(&String::from_utf8_lossy(&d[self.p + 1..self.p + 3]), 16)
                {
                    out.push(v);
                    self.p += 3;
                    continue;
                }
            }
            out.push(d[self.p]);
            self.p += 1;
        }
        out
    }

    fn literal_string(&mut self) -> Result<Vec<u8>, ()> {
        let d = self.d;
        self.p += 1;
        let mut depth = 1;
        let mut out = Vec::new();
        while self.p < d.len() {
            let b = d[self.p];
            self.p += 1;
            match b {
                b'(' => {
                    depth += 1;
                    out.push(b);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(out);
                    }
                    out.push(b);
                }
                b'\\' => {
                    let e = *d.get(self.p).ok_or(())?;
                    self.p += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'0'..=b'7' => {
                            let mut v = (e - b'0') as u32;
                            for _ in 0..2 {
                                match d.get(self.p) {
                                    Some(c @ b'0'..=b'7') => {
                                        v = v * 8 + (c - b'0') as u32;
                                        self.p += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push(v as u8);
                        }
                        b'\r' => {
                            if d.get(self.p) == Some(&b'\n') {
                                self.p += 1;
                            }
                        }
                        b'\n' => {}
                        other => out.push(other),
                    }
                }
                _ => out.push(b),
            }
        }
        Err(())
    }

    fn hex_string(&mut self) -> Result<Vec<u8>, ()> {
        let d = self.d;
        self.p += 1;
        let mut digits = Vec::new();
        while self.p < d.len() {
            let b = d[self.p];
            self.p += 1;
            match b {
                b'>' => {
                    if digits.len() % 2 == 1 {
                        digits.push(0);
                    }
                    return Ok(digits.chunks(2).map(|c| c[0] << 4 | c[1]).collect());
                }
                _ if is_ws(b) => {}
                _ => digits.push((b as char).to_digit(16).ok_or(())? as u8),
            }
        }
        Err(())
    }

    /// Skip inline image data after `ID`, up to and including `EI`.
    fn skip_inline_image(&mut self) -> Result<(), ()> {
        let d = self.d;
        // Exactly one whitespace byte separates ID from the data.
        self.p += 1;
        let mut i = self.p;
        while i + 1 < d.len() {
            if &d[i..i + 2] == b"EI"
                && i > 0
                && is_ws(d[i - 1])
                && (i + 2 == d.len() || is_ws(d[i + 2]) || is_delim(d[i + 2]))
            {
                self.p = i + 2;
                return Ok(());
            }
            i += 1;
        }
        Err(())
    }
}

/// An operator with its operands.
type Op = (Vec<u8>, Vec<Tok>);

/// Split a content stream into (operator, operands). Fails on anything that
/// does not tokenize cleanly, rather than silently dropping the remainder.
fn parse_ops(data: &[u8]) -> Result<Vec<Op>, ()> {
    let mut lx = Lexer { d: data, p: 0 };
    let mut ops = Vec::new();
    let mut operands: Vec<Tok> = Vec::new();
    // Open arrays/dicts: (is_dict, items)
    let mut open: Vec<(bool, Vec<Tok>)> = Vec::new();

    while let Some(lex) = lx.next()? {
        let tok = match lex {
            Lex::Operand(t) => t,
            Lex::ArrayStart => {
                open.push((false, vec![]));
                continue;
            }
            Lex::DictStart => {
                open.push((true, vec![]));
                continue;
            }
            Lex::ArrayEnd => match open.pop() {
                Some((false, items)) => Tok::Array(items),
                _ => return Err(()),
            },
            Lex::DictEnd => match open.pop() {
                Some((true, _)) => Tok::Other,
                _ => return Err(()),
            },
            Lex::Keyword(k) => {
                if !open.is_empty() {
                    return Err(());
                }
                if k == b"BI" {
                    // Inline image dictionary, then binary data.
                    loop {
                        match lx.next()? {
                            Some(Lex::Keyword(k)) if k == b"ID" => break,
                            Some(Lex::Keyword(_)) | None => return Err(()),
                            Some(_) => {}
                        }
                    }
                    lx.skip_inline_image()?;
                    operands.clear();
                } else {
                    ops.push((k, std::mem::take(&mut operands)));
                }
                continue;
            }
        };
        match open.last_mut() {
            Some((_, items)) => items.push(tok),
            None => operands.push(tok),
        }
    }
    if !open.is_empty() {
        return Err(());
    }
    Ok(ops)
}

// ---------------------------------------------------------------------------
// Simple-font code → glyph mapping
// ---------------------------------------------------------------------------

/// Windows-1252 code points for 0x80..=0x9F (undefined slots map to themselves).
const CP1252_HIGH: [u16; 32] = [
    0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
];

/// Every glyph a viewer might pick for `code`, following the lookup order in
/// PDF 32000 §9.6.6.4 across all cmap subtables present. Over-inclusion is
/// harmless; `None` means the code cannot be mapped confidently.
fn simple_code_to_gids(font: &TtFont, enc: SimpleEnc, code: u8) -> Option<Vec<u16>> {
    let c = code as u32;
    let mut out = Vec::new();
    let mut any_cmap = false;

    if let Some(t) = font.cmap(3, 0) {
        any_cmap = true;
        for cp in [c, 0xF000 + c, 0xF100 + c, 0xF200 + c] {
            out.extend(t.lookup(cp));
        }
    }
    if let Some(t) = font.cmap(1, 0) {
        any_cmap = true;
        out.extend(t.lookup(c));
    }
    if let Some(t) = font.unicode_cmap() {
        any_cmap = true;
        let unicode: Vec<u32> = match (enc, code) {
            (SimpleEnc::Builtin, 0x27) => vec![0x27, 0x2019],
            (SimpleEnc::Builtin, 0x60) => vec![0x60, 0x2018],
            (_, 0..=0x7F) => vec![c],
            (SimpleEnc::WinAnsi, 0x80..=0x9F) => vec![CP1252_HIGH[(code - 0x80) as usize] as u32],
            (SimpleEnc::WinAnsi, _) => vec![c],
            _ => vec![],
        };
        if unicode.is_empty() && out.is_empty() {
            return None;
        }
        for u in unicode {
            out.extend(t.lookup(u));
        }
    }
    any_cmap.then_some(out)
}

// ---------------------------------------------------------------------------
// TrueType parsing and glyf/loca subsetting
// ---------------------------------------------------------------------------

fn be_u16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn be_u32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

struct Table {
    tag: [u8; 4],
    offset: usize,
    len: usize,
}

pub(crate) struct TtFont<'d> {
    data: &'d [u8],
    tables: Vec<Table>,
    num_glyphs: u16,
    /// glyf-relative offsets, num_glyphs + 1 entries.
    loca: Vec<usize>,
}

pub(crate) struct CmapSub<'d> {
    d: &'d [u8],
}

impl<'d> TtFont<'d> {
    pub(crate) fn parse(data: &'d [u8]) -> Option<Self> {
        // Plain TrueType outlines only; 'OTTO' (CFF) and 'ttcf' collections are skipped.
        match be_u32(data, 0)? {
            0x0001_0000 | 0x7472_7565 => {}
            _ => return None,
        }
        let n = be_u16(data, 4)? as usize;
        let mut tables = Vec::with_capacity(n);
        for i in 0..n {
            let r = 12 + 16 * i;
            let tag: [u8; 4] = data.get(r..r + 4)?.try_into().ok()?;
            let offset = be_u32(data, r + 8)? as usize;
            let len = be_u32(data, r + 12)? as usize;
            if offset.checked_add(len)? > data.len() {
                return None;
            }
            tables.push(Table { tag, offset, len });
        }
        let mut font = TtFont {
            data,
            tables,
            num_glyphs: 0,
            loca: vec![],
        };

        let head = font.table(b"head")?;
        let maxp = font.table(b"maxp")?;
        let loca = font.table(b"loca")?;
        let glyf_len = font.table(b"glyf")?.len();
        font.num_glyphs = be_u16(maxp, 4)?;
        let long = match be_u16(head, 50)? {
            0 => false,
            1 => true,
            _ => return None,
        };
        let mut offsets = Vec::with_capacity(font.num_glyphs as usize + 1);
        for i in 0..=font.num_glyphs as usize {
            let off = if long {
                be_u32(loca, i * 4)? as usize
            } else {
                be_u16(loca, i * 2)? as usize * 2
            };
            if off > glyf_len || offsets.last().is_some_and(|&prev| off < prev) {
                return None;
            }
            offsets.push(off);
        }
        font.loca = offsets;
        Some(font)
    }

    fn table(&self, tag: &[u8; 4]) -> Option<&'d [u8]> {
        let t = self.tables.iter().find(|t| &t.tag == tag)?;
        Some(&self.data[t.offset..t.offset + t.len])
    }

    pub(crate) fn glyph(&self, gid: u16) -> &'d [u8] {
        let glyf = self.table(b"glyf").unwrap_or(&[]);
        let i = gid as usize;
        if i + 1 >= self.loca.len() {
            return &[];
        }
        &glyf[self.loca[i]..self.loca[i + 1]]
    }

    pub(crate) fn cmap(&self, platform: u16, encoding: u16) -> Option<CmapSub<'d>> {
        let cmap = self.table(b"cmap")?;
        let n = be_u16(cmap, 2)? as usize;
        for i in 0..n {
            let r = 4 + 8 * i;
            if be_u16(cmap, r)? == platform && be_u16(cmap, r + 2)? == encoding {
                let off = be_u32(cmap, r + 4)? as usize;
                return Some(CmapSub {
                    d: cmap.get(off..)?,
                });
            }
        }
        None
    }

    pub(crate) fn unicode_cmap(&self) -> Option<CmapSub<'d>> {
        self.cmap(3, 10)
            .or_else(|| self.cmap(3, 1))
            .or_else(|| (0..=6).find_map(|e| self.cmap(0, e)))
    }
}

impl CmapSub<'_> {
    pub(crate) fn lookup(&self, cp: u32) -> Option<u16> {
        let d = self.d;
        let gid = match be_u16(d, 0)? {
            0 => *d.get(6 + usize::try_from(cp).ok().filter(|&c| c < 256)?)? as u16,
            4 => {
                let cp = u16::try_from(cp).ok()?;
                let seg_x2 = be_u16(d, 6)? as usize;
                let (ends, starts, deltas, ranges) =
                    (14, 16 + seg_x2, 16 + 2 * seg_x2, 16 + 3 * seg_x2);
                let mut found = None;
                for i in 0..seg_x2 / 2 {
                    if be_u16(d, ends + 2 * i)? >= cp {
                        found = Some(i);
                        break;
                    }
                }
                let i = found?;
                let start = be_u16(d, starts + 2 * i)?;
                if start > cp {
                    return None;
                }
                let delta = be_u16(d, deltas + 2 * i)?;
                let ro = be_u16(d, ranges + 2 * i)? as usize;
                if ro == 0 {
                    cp.wrapping_add(delta)
                } else {
                    let addr = ranges + 2 * i + ro + 2 * (cp - start) as usize;
                    match be_u16(d, addr)? {
                        0 => 0,
                        g => g.wrapping_add(delta),
                    }
                }
            }
            6 => {
                let first = be_u16(d, 6)? as u32;
                let count = be_u16(d, 8)? as u32;
                if cp < first || cp >= first + count {
                    return None;
                }
                be_u16(d, 10 + 2 * (cp - first) as usize)?
            }
            12 => {
                let groups = be_u32(d, 12)? as usize;
                let mut gid = None;
                for i in 0..groups {
                    let g = 16 + 12 * i;
                    let (start, end) = (be_u32(d, g)?, be_u32(d, g + 4)?);
                    if (start..=end).contains(&cp) {
                        gid = u16::try_from(be_u32(d, g + 8)? + (cp - start)).ok();
                        break;
                    }
                }
                gid?
            }
            _ => return None,
        };
        (gid != 0).then_some(gid)
    }
}

/// Component glyph ids of a composite glyph (empty for simple glyphs).
fn components(glyph: &[u8]) -> Option<Vec<u16>> {
    if glyph.len() < 10 || (be_u16(glyph, 0)? as i16) >= 0 {
        return Some(vec![]);
    }
    let mut out = vec![];
    let mut p = 10;
    loop {
        let flags = be_u16(glyph, p)?;
        out.push(be_u16(glyph, p + 2)?);
        p += 4;
        p += if flags & 0x0001 != 0 { 4 } else { 2 };
        if flags & 0x0008 != 0 {
            p += 2;
        } else if flags & 0x0040 != 0 {
            p += 4;
        } else if flags & 0x0080 != 0 {
            p += 8;
        }
        if flags & 0x0020 == 0 {
            return Some(out);
        }
    }
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, c| {
        let mut w = [0u8; 4];
        w[..c.len()].copy_from_slice(c);
        sum.wrapping_add(u32::from_be_bytes(w))
    })
}

/// Assemble an sfnt from tables (written in the given order, which should be
/// sorted by tag). Fixes up head.checkSumAdjustment.
pub(crate) fn assemble(sfnt_version: u32, tables: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let n = tables.len() as u16;
    let mut pow = 1u16;
    let mut log = 0u16;
    while pow * 2 <= n {
        pow *= 2;
        log += 1;
    }
    let mut out = Vec::new();
    out.extend(sfnt_version.to_be_bytes());
    out.extend(n.to_be_bytes());
    out.extend((pow * 16).to_be_bytes());
    out.extend(log.to_be_bytes());
    out.extend((n * 16 - pow * 16).to_be_bytes());

    let dir = out.len();
    out.resize(dir + 16 * tables.len(), 0);
    let mut head_at = None;
    for (i, (tag, data)) in tables.iter().enumerate() {
        let offset = out.len();
        let mut data = data.clone();
        if tag == b"head" && data.len() >= 12 {
            data[8..12].copy_from_slice(&[0; 4]);
            head_at = Some(offset);
        }
        let r = dir + 16 * i;
        out[r..r + 4].copy_from_slice(tag);
        out[r + 4..r + 8].copy_from_slice(&checksum(&data).to_be_bytes());
        out[r + 8..r + 12].copy_from_slice(&(offset as u32).to_be_bytes());
        out[r + 12..r + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend(&data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    if let Some(h) = head_at {
        let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        out[h + 8..h + 12].copy_from_slice(&adj.to_be_bytes());
    }
    out
}

/// Keep the outlines of `keep` (plus composite dependencies); every other
/// glyph becomes empty. Glyph count and ids are unchanged.
pub(crate) fn subset_truetype(font: &TtFont, keep: &BTreeSet<u16>) -> Option<Vec<u8>> {
    let mut kept: BTreeSet<u16> = BTreeSet::new();
    let mut work: Vec<u16> = keep
        .iter()
        .copied()
        .filter(|&g| g < font.num_glyphs)
        .collect();
    while let Some(g) = work.pop() {
        if kept.insert(g) {
            for c in components(font.glyph(g))? {
                if c < font.num_glyphs && !kept.contains(&c) {
                    work.push(c);
                }
            }
        }
    }

    let mut glyf = Vec::new();
    let mut loca = Vec::with_capacity((font.num_glyphs as usize + 1) * 4);
    for g in 0..font.num_glyphs {
        loca.extend((glyf.len() as u32).to_be_bytes());
        if kept.contains(&g) {
            glyf.extend_from_slice(font.glyph(g));
            while glyf.len() % 4 != 0 {
                glyf.push(0);
            }
        }
    }
    loca.extend((glyf.len() as u32).to_be_bytes());

    let mut head = font.table(b"head")?.to_vec();
    head.get_mut(50..52)?.copy_from_slice(&1u16.to_be_bytes()); // long loca

    let tables: Vec<([u8; 4], Vec<u8>)> = font
        .tables
        .iter()
        .filter(|t| &t.tag != b"DSIG") // signature no longer matches
        .map(|t| {
            let data = match &t.tag {
                b"glyf" => glyf.clone(),
                b"loca" => loca.clone(),
                b"head" => head.clone(),
                _ => font.data[t.offset..t.offset + t.len].to_vec(),
            };
            (t.tag, data)
        })
        .collect();
    let out = assemble(be_u32(font.data, 0)?, &tables);

    // Self-check: the result must parse and keep every retained outline intact.
    // (Kept glyphs may gain up to 3 bytes of alignment padding.)
    let check = TtFont::parse(&out)?;
    let intact = |g: u16| check.glyph(g).starts_with(font.glyph(g));
    if check.num_glyphs != font.num_glyphs || !kept.iter().all(|&g| intact(g)) {
        return None;
    }
    Some(out)
}

fn subset_tag(program: ObjectId, keep: &BTreeSet<u16>) -> [u8; 6] {
    // FNV-1a over the program id and kept glyphs; deterministic per output.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    program.0.to_be_bytes().into_iter().for_each(&mut feed);
    program.1.to_be_bytes().into_iter().for_each(&mut feed);
    keep.iter()
        .flat_map(|g| g.to_be_bytes())
        .for_each(&mut feed);
    let mut tag = [0u8; 6];
    for (i, t) in tag.iter_mut().enumerate() {
        *t = b'A' + ((h >> (i * 8)) % 26) as u8;
    }
    tag
}

#[cfg(test)]
pub(crate) mod test_font {
    //! A tiny synthetic TrueType font: glyph 0 (.notdef, empty), glyphs 1 and
    //! 2 simple outlines, glyph 3 a composite of glyph 1. cmap (3,1) maps
    //! 'A'→1, 'B'→2, 'C'→3.

    use super::assemble;

    pub fn simple_glyph(seed: u8) -> Vec<u8> {
        let mut g = vec![0, 1, 0, 0, 0, 0, 0, 10, 0, 10, 0, 0, 0, 0, 0x01, seed];
        g.resize(20, seed);
        g
    }

    pub fn composite_glyph() -> Vec<u8> {
        // numberOfContours = -1, bbox, flags=0 (byte args, last component), glyph 1, args.
        vec![
            0xFF, 0xFF, 0, 0, 0, 0, 0, 10, 0, 10, 0x00, 0x00, 0x00, 0x01, 0, 0,
        ]
    }

    pub fn build() -> Vec<u8> {
        let glyphs = [
            vec![],
            simple_glyph(0x11),
            simple_glyph(0x22),
            composite_glyph(),
        ];
        let mut glyf = Vec::new();
        let mut loca = Vec::new();
        for g in &glyphs {
            loca.extend(((glyf.len() / 2) as u16).to_be_bytes());
            glyf.extend(g);
        }
        loca.extend(((glyf.len() / 2) as u16).to_be_bytes());

        let mut head = vec![0u8; 54];
        head[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
        head[18..20].copy_from_slice(&1000u16.to_be_bytes());
        // indexToLocFormat = 0 (short)

        let mut maxp = vec![0u8; 6];
        maxp[0..4].copy_from_slice(&0x0000_5000u32.to_be_bytes());
        maxp[4..6].copy_from_slice(&(glyphs.len() as u16).to_be_bytes());

        // cmap with a single (3,1) format 4 subtable: one segment 0x41..0x43 → 1..3, plus 0xFFFF.
        let mut sub = Vec::new();
        let seg_x2 = 4u16;
        sub.extend(4u16.to_be_bytes()); // format
        sub.extend(32u16.to_be_bytes()); // length
        sub.extend(0u16.to_be_bytes()); // language
        sub.extend(seg_x2.to_be_bytes());
        sub.extend([0, 4, 0, 1, 0, 0]); // searchRange, entrySelector, rangeShift
        sub.extend([0x00, 0x43, 0xFF, 0xFF]); // endCodes
        sub.extend([0, 0]); // reservedPad
        sub.extend([0x00, 0x41, 0xFF, 0xFF]); // startCodes
        sub.extend(((1i32 - 0x41) as i16 as u16).to_be_bytes()); // idDelta seg 0
        sub.extend(1u16.to_be_bytes()); // idDelta seg 1
        sub.extend([0, 0, 0, 0]); // idRangeOffsets
        let mut cmap = Vec::new();
        cmap.extend(0u16.to_be_bytes());
        cmap.extend(1u16.to_be_bytes());
        cmap.extend(3u16.to_be_bytes());
        cmap.extend(1u16.to_be_bytes());
        cmap.extend(12u32.to_be_bytes());
        cmap.extend(sub);

        assemble(
            0x0001_0000,
            &[
                (*b"cmap", cmap),
                (*b"glyf", glyf),
                (*b"head", head),
                (*b"loca", loca),
                (*b"maxp", maxp),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_handles_strings_arrays_and_inline_images() {
        let data = b"BT /F1 12 Tf (a\\)b\\101) Tj [<4142> -20 (C)] TJ ET \
                     BI /W 1 /H 1 /CS /G /BPC 8 ID \xffEI\x00 EI Q % comment\n/F2 9 Tf";
        let ops = parse_ops(data).unwrap();
        let names: Vec<&[u8]> = ops.iter().map(|(o, _)| o.as_slice()).collect();
        assert_eq!(
            names,
            vec![&b"BT"[..], b"Tf", b"Tj", b"TJ", b"ET", b"Q", b"Tf"]
        );
        assert_eq!(ops[2].1, vec![Tok::Str(b"a)bA".to_vec())]);
        assert_eq!(
            ops[3].1,
            vec![Tok::Array(vec![
                Tok::Str(b"AB".to_vec()),
                Tok::Other,
                Tok::Str(b"C".to_vec())
            ])]
        );
    }

    #[test]
    fn tokenizer_rejects_malformed_content() {
        assert!(parse_ops(b"BT (unterminated Tj").is_err());
        assert!(parse_ops(b"[ (a) Tj").is_err());
        assert!(parse_ops(b"BI /W 1 ID nodelimiter").is_err());
    }

    #[test]
    fn cmap_format4_lookup() {
        let data = test_font::build();
        let font = TtFont::parse(&data).unwrap();
        let cmap = font.cmap(3, 1).unwrap();
        assert_eq!(cmap.lookup(0x41), Some(1));
        assert_eq!(cmap.lookup(0x43), Some(3));
        assert_eq!(cmap.lookup(0x44), None);
    }

    #[test]
    fn subset_keeps_ids_and_composite_components() {
        let data = test_font::build();
        let font = TtFont::parse(&data).unwrap();
        let out = subset_truetype(&font, &BTreeSet::from([0, 3])).unwrap();
        let sub = TtFont::parse(&out).unwrap();

        assert_eq!(sub.num_glyphs, 4);
        assert_eq!(sub.glyph(3), font.glyph(3));
        assert_eq!(
            sub.glyph(1),
            font.glyph(1),
            "component of glyph 3 must survive"
        );
        assert!(sub.glyph(2).is_empty(), "unused glyph must be emptied");
        assert_eq!(checksum(&out), 0xB1B0_AFBA, "head.checkSumAdjustment");
    }

    /// Run against a real font: PDFSAN_TEST_FONT=/path/to/font.ttf cargo test
    #[test]
    fn subset_real_font_from_env() {
        let path = match std::env::var("PDFSAN_TEST_FONT") {
            Ok(p) => p,
            Err(_) => return,
        };
        let data = std::fs::read(path).unwrap();
        let font = TtFont::parse(&data).unwrap();
        let cmap = font.unicode_cmap().unwrap();
        let keep: BTreeSet<u16> = "Hello, wörld! ﬁ"
            .chars()
            .filter_map(|c| cmap.lookup(c as u32))
            .collect();
        let out = subset_truetype(&font, &keep).unwrap();
        let sub = TtFont::parse(&out).unwrap();
        assert!(
            out.len() < data.len() / 2,
            "{} vs {}",
            out.len(),
            data.len()
        );
        for g in 0..font.num_glyphs {
            if keep.contains(&g) {
                assert!(sub.glyph(g).starts_with(font.glyph(g)), "glyph {g} changed");
            }
        }
        assert_eq!(checksum(&out), 0xB1B0_AFBA);
    }
}
