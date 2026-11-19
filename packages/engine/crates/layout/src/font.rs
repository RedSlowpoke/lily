//! LilyPond's font metrics (`font-metric.cc`, `font-metric-scheme.cc`,
//! `open-type-font.cc`, `open-type-font-scheme.cc`,
//! `modified-font-metric.cc`, `all-font-metrics.cc`,
//! `all-font-metrics-scheme.cc`) over W1's packs: an [`OpenTypeFont`] is
//! `Open_type_font`, read from the packs of `crates/fonts` ([`Font`])
//! where LilyPond reads the OTF through FreeType: the glyph names and
//! indices, the characters of the cmap, the PostScript name, `LILC`'s boxes
//! and attachments and `LILY`'s values, the design size among them. A
//! [`ModifiedFontMetric`] is `Modified_font_metric`, a font scaled by a
//! magnification, which font selection gives (`font_select.rs`).
//! `font-metric-scheme.cc`'s seven `ly:` functions and
//! `open-type-font-scheme.cc`'s nine are here under LilyPond's names and
//! arities, taking a scaled font as LilyPond's take it; the five that read
//! what the packs do not hold (a table's bytes, a font file's format,
//! glyph names or CFF offset, a collection's subfont) check their
//! arguments as LilyPond does and then throw by name (D67).
//!
//! [`AllFontMetrics`] is `All_font_metrics`, `(lily)`'s `%all-fonts`: the
//! packs a host gave `%put-font-pack` ([`put_pack`]), by name, the parts of
//! `emmentaler-brace` one after another, which stand for the OTFs of
//! LilyPond's search path; and the table of the fonts loaded from them,
//! which `ly:system-font-load` and font selection fill as
//! `All_font_metrics::find_otf_font` does, the same font each time, and
//! `ly:reset-all-fonts` forgets ([`reset_all_fonts`]), as LilyPond's makes
//! a new `All_font_metrics`. A font reads its pack's glyphs, so a part
//! given after the font was loaded adds to it. An image holds no packs: a
//! host gives them after it started. Since MS3's part 1 step 8 it keeps the
//! text packs too, by name (`C059-Roman`), and `pango_dict_`, the Pango
//! fonts loaded from them ([`find_pango_font`], `pango.rs`), which the font
//! functions take as LilyPond's take a `Pango_font`. Since MS3 step 50
//! (W5, D208) a font of the build's packs that no host gave is noted when
//! it is first asked for ([`take_wanted`]), so that the core asks the host
//! for its packs ([`pack_modules`]). Since MS7 step 5 (E14, D262) a text
//! pack's outlines are a pack of their own, `<face>.outlines`, which a
//! skyline that needs one notes on its face and [`take_wanted`] names with
//! the fonts; a host gives it as any pack ([`put_pack`]). LilyPond's
//! programming errors (a character asked of an index the cmap has not) and
//! the warning of a negative font index are not written.

use alloc::boxed::Box as Owned;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_fonts::{Font, Metrics, TextFont};
use lily_engine_scheme::{
    Bytevector, Cx, Exception, Foreign, Heap, ImageReader, ImageWriter, Thrown, Tracer, Value, Vm,
};

use crate::R;
use crate::geometry::{Box, Interval};
use crate::pango::{
    Description, FAMILIES, PangoFont, TextPack, face_files, needs_fallback, pango_font,
};
use crate::stencil::{Stencil, wrap};

/// A font of the packs a host gave: crates/fonts' [`Font`], its parts
/// added as they came, which every `Open_type_font` of its name reads.
pub struct FontPack {
    pub font: Font,
}

impl Foreign for FontPack {
    fn type_name(&self) -> &'static str {
        "Font_pack"
    }
}

/// `Open_type_font`: the font of a pack ([`FontPack`]) and its
/// `description_`, `(name . 1.0)`, as `find_otf_font` gives it.
pub struct OpenTypeFont {
    pack: Value,
    description: Value,
}

impl Foreign for OpenTypeFont {
    fn type_name(&self) -> &'static str {
        "Open_type_font"
    }

    /// `Font_metric::print_smob`: `#<Open_type_font ("emmentaler-20" . 1.0)>`.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Open_type_font ");
        heap.write(self.description, out);
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.pack);
        tracer.trace(self.description);
    }
}

/// `Modified_font_metric`: the font `orig` scaled by `magnification`, its
/// `description_` `(name . total magnification)`.
pub struct ModifiedFontMetric {
    orig: Value,
    magnification: f64,
    description: Value,
}

impl Foreign for ModifiedFontMetric {
    fn type_name(&self) -> &'static str {
        "Modified_font_metric"
    }

    /// `#<Modified_font_metric ("emmentaler-20" . 0.5690551181102361)>`.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Modified_font_metric ");
        heap.write(self.description, out);
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.orig);
        tracer.trace(self.description);
    }
}

/// `Modified_font_metric::make_scaled_font_metric`: `font`, an
/// `Open_type_font`, scaled by `magnification`.
pub fn make_scaled(heap: &mut Heap, font: Value, magnification: f64) -> Value {
    let description = heap
        .foreign::<OpenTypeFont>(font)
        .expect("a scaled font scales an Open_type_font")
        .description;
    let (name, mag) = (heap.car(description), heap.cdr(description));
    let (name, mag) = (
        name.unwrap_or(Value::FALSE),
        mag.and_then(|m| heap.to_f64(m)),
    );
    let total = heap.flonum(magnification * mag.unwrap_or(1.0));
    let description = heap.cons(name, total);
    heap.wrap(ModifiedFontMetric {
        orig: font,
        magnification,
        description,
    })
}

/// A font metric of either kind as its functions read it: the font of its
/// pack, the magnification its boxes take (1 for an `Open_type_font`) and
/// its own `description_`.
#[derive(Clone, Copy)]
pub struct Metric<'h> {
    pub font: &'h Font,
    pub magnification: f64,
    pub description: Value,
}

/// `v` as a [`Metric`], or `None` if it is no font metric.
pub fn metric(heap: &Heap, v: Value) -> Option<Metric<'_>> {
    let (otf, magnification, description) = match heap.foreign::<ModifiedFontMetric>(v) {
        Some(m) => (
            heap.foreign::<OpenTypeFont>(m.orig)?,
            m.magnification,
            m.description,
        ),
        None => {
            let otf = heap.foreign::<OpenTypeFont>(v)?;
            (otf, 1.0, otf.description)
        }
    };
    let font = &heap.foreign::<FontPack>(otf.pack)?.font;
    Some(Metric {
        font,
        magnification,
        description,
    })
}

/// The `Open_type_font` of `v`: itself, or the font a scaled font scales,
/// as `open-type-font-scheme.cc` takes either.
fn open_type(heap: &Heap, v: Value) -> Option<&Font> {
    let v = heap.foreign::<ModifiedFontMetric>(v).map_or(v, |m| m.orig);
    let otf = heap.foreign::<OpenTypeFont>(v)?;
    Some(&heap.foreign::<FontPack>(otf.pack)?.font)
}

/// `Font_metric::find_by_name` of the font `font`: the glyph `name`, a `-`
/// in it read as `M`, as a `named-glyph` stencil of its extents, scaled by
/// the font's magnification, or the empty stencil without an expression
/// when the font has no such glyph or `font` is no font.
pub fn find_by_name(heap: &mut Heap, font: Value, name: &str) -> Stencil {
    let name = name.replace('-', "M");
    let Some(b) = metric(heap, font).and_then(|m| {
        let index = m.font.name_to_index(&name)?;
        let b = m.font.char_dimensions(index);
        let mut b = Box::new(
            Interval::new(b.x.left, b.x.right),
            Interval::new(b.y.left, b.y.right),
        );
        if heap.foreign::<ModifiedFontMetric>(font).is_some() {
            b.scale(m.magnification);
        }
        Some(b)
    }) else {
        return Stencil::default();
    };
    let head = heap.intern("named-glyph");
    let name = heap.string(name);
    let expr = heap.list(&[head, font, name]);
    Stencil::new(b, expr)
}

// --- All_font_metrics ------------------------------------------------------------

/// `All_font_metrics`: the packs a host gave, standing for the OTFs of the
/// search path, and `otf_dict_`, the fonts loaded from them, each by name.
#[derive(Default)]
pub struct AllFontMetrics {
    packs: Vec<(String, Value)>,
    otf: Vec<(String, Value)>,
    /// The text packs ([`TextPack`]) by name, and Emmentaler's read for
    /// text, by its file's.
    text: Vec<(String, Value)>,
    /// `pango_dict_`: the Pango fonts by `pango_font_description_to_filename`.
    pango: Vec<(String, Value)>,
    /// The fonts of the build's packs asked for and not given (W5), once each.
    wanted: Vec<String>,
    /// Outlines packs given before their face (D262), by its name: read when it comes.
    early_outlines: Vec<(String, Vec<u8>)>,
}

impl Foreign for AllFontMetrics {
    fn type_name(&self) -> &'static str {
        "All_font_metrics"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for &(_, v) in self
            .packs
            .iter()
            .chain(&self.otf)
            .chain(&self.text)
            .chain(&self.pango)
        {
            tracer.trace(v);
        }
    }

    /// An image holds no packs, so only a table without any is written.
    fn save(&self, _: &mut ImageWriter) -> bool {
        self.packs.is_empty()
            && self.otf.is_empty()
            && self.text.is_empty()
            && self.pango.is_empty()
            && self.wanted.is_empty()
            && self.early_outlines.is_empty()
    }
}

/// What [`AllFontMetrics::save`] wrote: an empty table.
pub(crate) fn load_all(_: &mut ImageReader<'_>) -> Option<Owned<dyn Foreign>> {
    Some(Owned::new(AllFontMetrics::default()))
}

/// `(%make-all-font-metrics)`: the table `(lily)` keeps as `%all-fonts`.
pub(crate) fn make_all(cx: &mut Cx, _: &[Value]) -> R {
    Ok(cx.heap_mut().wrap(AllFontMetrics::default()))
}

/// `(lily)`'s `%all-fonts`.
fn all_fonts(cx: &mut Cx) -> Value {
    cx.global("lily", "%all-fonts")
        .filter(|&v| cx.heap().foreign::<AllFontMetrics>(v).is_some())
        .expect("layout's prelude defines %all-fonts")
}

/// `All_font_metrics::find_otf_font`: the font `name` loaded, the same
/// each time until `ly:reset-all-fonts`, from the pack a host gave of that
/// name; LilyPond's error when there is none, the search path left out.
pub fn find_otf_font(cx: &mut Cx, name: &str) -> Result<Value, Thrown> {
    let all = all_fonts(cx);
    let table = cx.heap().foreign::<AllFontMetrics>(all).unwrap();
    if let Some(&(_, font)) = table.otf.iter().find(|(n, _)| n == name) {
        return Ok(font);
    }
    let Some(&(_, pack)) = table.packs.iter().find(|(n, _)| n == name) else {
        return Err(missing(cx, name));
    };
    let heap = cx.heap_mut();
    let string = heap.string(name);
    let one = heap.flonum(1.0);
    let description = heap.cons(string, one);
    let font = heap.wrap(OpenTypeFont { pack, description });
    let table = heap.foreign_mut::<AllFontMetrics>(all).unwrap();
    table.otf.push((String::from(name), font));
    Ok(font)
}

/// LilyPond's error for the font `name`, which no host gave a pack of;
/// one of the build's packs is noted for the host to be asked for (W5).
#[cold]
fn missing(cx: &mut Cx, name: &str) -> Thrown {
    let all = all_fonts(cx);
    want(cx.heap_mut(), all, name);
    let name = cx.heap_mut().string(name);
    cx.error("misc-error", None, "cannot find font '~a'", &[name])
}

/// Notes the font `name` in `all`, `%all-fonts`, for the host to be asked
/// for its packs, once, if it is a font of the build's packs no host gave.
pub(crate) fn want(heap: &mut Heap, all: Value, name: &str) {
    let Some(table) = heap.foreign_mut::<AllFontMetrics>(all) else {
        return;
    };
    let known = |n: &String| n == name;
    if pack_modules(name).is_empty()
        || table.packs.iter().any(|(n, _)| known(n))
        || table.wanted.iter().any(known)
    {
        return;
    }
    table.wanted.push(String::from(name));
}

/// The fonts noted for the host since [`take_wanted`] last took them,
/// which it keeps, and the outlines a skyline noted.
pub fn wanted(host: &mut dyn lily_engine_music::Host) -> Vec<String> {
    let all = host.lily("%all-fonts").unwrap_or(Value::FALSE);
    let heap = host.heap();
    let table = heap.foreign::<AllFontMetrics>(all);
    table.map_or_else(Vec::new, |t| {
        let mut wanted = t.wanted.clone();
        wanted.extend(outlines_wanted(heap, t, false));
        wanted
    })
}

/// The module of the outlines of the text face `face` (D262).
pub fn outlines_module(face: &str) -> String {
    alloc::format!("{face}{OUTLINES}")
}

const OUTLINES: &str = ".outlines";

/// The outlines modules of the faces a skyline noted, each unnoted if
/// `take`.
fn outlines_wanted(heap: &Heap, table: &AllFontMetrics, take: bool) -> Vec<String> {
    let mut wanted = Vec::new();
    for &(ref name, face) in &table.text {
        let Some(face) = heap.foreign::<TextPack>(face) else {
            continue;
        };
        if face.wants_outlines.get() {
            if take {
                face.wants_outlines.set(false);
            }
            wanted.push(outlines_module(name));
        }
    }
    wanted
}

/// Whether a skyline noted outlines no host has given since the compile
/// began ([`take_wanted`]): the core stops it there for them (D262).
pub fn wants_outlines(vm: &mut Vm) -> bool {
    let Some(all) = vm
        .slot("lily", "%all-fonts")
        .and_then(|slot| vm.slot_value(slot))
    else {
        return false;
    };
    let heap = vm.heap();
    heap.foreign::<AllFontMetrics>(all).is_some_and(|table| {
        table.text.iter().any(|&(_, face)| {
            heap.foreign::<TextPack>(face)
                .is_some_and(|f| f.wants_outlines.get())
        })
    })
}

/// The modules of the build's packs that give the font `name`, in the
/// order a host puts them (`scripts/build.mjs`' manifest, D208): an
/// Emmentaler size's pack, the four parts of `emmentaler-brace`, or a text
/// face's or its outlines' (D262); none for a font the build has no pack
/// of.
pub fn pack_modules(name: &str) -> Vec<String> {
    const SIZES: [&str; 8] = ["11", "13", "14", "16", "18", "20", "23", "26"];
    let face = name.strip_suffix(OUTLINES);
    if face.is_some_and(|f| FAMILIES.iter().any(|(_, faces)| faces.contains(&f))) {
        return alloc::vec![String::from(name)];
    }
    if name == "emmentaler-brace" {
        return (0..4).map(|i| alloc::format!("{name}-{i}")).collect();
    }
    let size = name.strip_prefix("emmentaler-");
    let text = FAMILIES.iter().any(|(_, faces)| faces.contains(&name));
    match size.is_some_and(|s| SIZES.contains(&s)) || text {
        true => alloc::vec![String::from(name)],
        false => Vec::new(),
    }
}

/// The fonts of the build's packs that `vm`'s `(lily)` was asked for and
/// no host had given, since the last call, in the order first asked; then
/// the outlines modules of the text faces a skyline needed (D262).
pub fn take_wanted(vm: &mut Vm) -> Vec<String> {
    let all = vm
        .slot("lily", "%all-fonts")
        .and_then(|slot| vm.slot_value(slot))
        .expect("layout's prelude defines %all-fonts");
    let mut wanted = match vm.heap_mut().foreign_mut::<AllFontMetrics>(all) {
        Some(table) => core::mem::take(&mut table.wanted),
        None => return Vec::new(),
    };
    let heap = vm.heap();
    let table = heap.foreign::<AllFontMetrics>(all).unwrap();
    wanted.extend(outlines_wanted(heap, table, true));
    wanted
}

/// Gives a pack of `crates/fonts` to `vm`'s `(lily)`, through
/// `%put-font-pack`: a font of its own, or the next part of the font it
/// continues.
pub fn put_pack(vm: &mut Vm, pack: &[u8]) -> Result<(), Exception> {
    let put = vm
        .slot("lily", "%put-font-pack")
        .and_then(|slot| vm.slot_value(slot))
        .expect("layout's prelude defines %put-font-pack");
    let pack = vm.heap_mut().wrap(Bytevector(pack.to_vec()));
    vm.apply(put, &[pack]).map(|_| ())
}

/// Whether `data` is a pack of W1's, of a version this engine reads: a
/// music font's or a text font's.
pub fn is_pack(data: &[u8]) -> bool {
    Font::pack_name(data).is_ok()
        || TextFont::parse(data).is_ok()
        || TextFont::outlines_name(data).is_ok()
}

/// `ly:reset-all-fonts` in `vm`, as `lilypond-all` calls it after each
/// file.
pub fn reset_all_fonts(vm: &mut Vm) -> Result<(), Exception> {
    let reset = vm
        .slot("lily", "ly:reset-all-fonts")
        .and_then(|slot| vm.slot_value(slot))
        .expect("layout defines ly:reset-all-fonts");
    vm.apply(reset, &[]).map(|_| ())
}

/// A pack that is not one, or does not continue the font it names.
#[cold]
fn bad_pack(cx: &mut Cx, subr: &str, error: lily_engine_fonts::Error) -> Thrown {
    use lily_engine_fonts::Error;
    // Not the error's Debug, which would bring core::fmt's (H4).
    let what = match error {
        Error::NotAPack => "not a font pack of W1's",
        Error::Version(_) => "a font pack of a version this engine does not read",
        Error::Truncated => "a font pack cut short or malformed",
        Error::NotTheNextPart => "a font pack that does not continue its font",
    };
    cx.error("misc-error", Some(subr), what, &[])
}

/// `(%put-font-pack pack)`: a bytevector's pack, a font of its own or the
/// next part of the font of its name.
pub(crate) fn put_font_pack(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "%put-font-pack";
    let pack = match cx.heap().foreign::<Bytevector>(args[0]) {
        Some(bytes) => bytes.0.clone(),
        None => return Err(cx.wrong_type(SUBR, 1, args[0], "bytevector")),
    };
    if pack.starts_with(b"LILO") {
        let name = TextFont::outlines_name(&pack).map_err(|e| bad_pack(cx, SUBR, e))?;
        let all = all_fonts(cx);
        let table = cx.heap().foreign::<AllFontMetrics>(all).unwrap();
        let face = table.text.iter().find(|(n, _)| *n == name).map(|&(_, f)| f);
        let Some(face) = face else {
            // Its face is read first; until a host gives it, the outlines wait.
            let table = cx.heap_mut().foreign_mut::<AllFontMetrics>(all).unwrap();
            table.early_outlines.retain(|(n, _)| *n != name);
            table.early_outlines.push((name, pack));
            return Ok(Value::UNSPECIFIED);
        };
        let face = cx.heap_mut().foreign_mut::<TextPack>(face).unwrap();
        let added = face.font.add_outlines(&pack);
        added.map_err(|e| bad_pack(cx, SUBR, e))?;
        return Ok(Value::UNSPECIFIED);
    }
    if pack.starts_with(b"LILT") {
        let mut font = TextFont::parse(&pack).map_err(|e| bad_pack(cx, SUBR, e))?;
        let name = String::from(font.name());
        let all = all_fonts(cx);
        if cx
            .heap()
            .foreign::<AllFontMetrics>(all)
            .unwrap()
            .text
            .iter()
            .any(|(n, _)| *n == name)
        {
            return Ok(Value::UNSPECIFIED);
        }
        let table = cx.heap_mut().foreign_mut::<AllFontMetrics>(all).unwrap();
        if let Some(at) = table.early_outlines.iter().position(|(n, _)| *n == name) {
            let (_, outlines) = table.early_outlines.remove(at);
            font.add_outlines(&outlines)
                .map_err(|e| bad_pack(cx, SUBR, e))?;
        }
        let heap = cx.heap_mut();
        let file = name.clone();
        let given = heap.wrap(TextPack {
            font,
            file,
            music: None,
            wants_outlines: core::cell::Cell::new(false),
        });
        let table = heap.foreign_mut::<AllFontMetrics>(all).unwrap();
        table.text.push((name, given));
        return Ok(Value::UNSPECIFIED);
    }
    let name = Font::pack_name(&pack).map_err(|e| bad_pack(cx, SUBR, e))?;
    let all = all_fonts(cx);
    let table = cx.heap().foreign::<AllFontMetrics>(all).unwrap();
    if let Some(&(_, given)) = table.packs.iter().find(|(n, _)| *n == name) {
        let added = cx
            .heap_mut()
            .foreign_mut::<FontPack>(given)
            .unwrap()
            .font
            .add(&pack);
        added.map_err(|e| bad_pack(cx, SUBR, e))?;
        return Ok(Value::UNSPECIFIED);
    }
    let font = Font::parse(&pack).map_err(|e| bad_pack(cx, SUBR, e))?;
    let heap = cx.heap_mut();
    let given = heap.wrap(FontPack { font });
    let table = heap.foreign_mut::<AllFontMetrics>(all).unwrap();
    table.packs.push((name, given));
    Ok(Value::UNSPECIFIED)
}

// --- all-font-metrics-scheme.cc ---------------------------------------------------

/// `(ly:reset-all-fonts)`: the fonts loaded so far forgotten, Pango's
/// among them; the packs stay, as the OTFs of the search path do.
pub(crate) fn reset(cx: &mut Cx, _: &[Value]) -> R {
    let all = all_fonts(cx);
    let table = cx.heap_mut().foreign_mut::<AllFontMetrics>(all).unwrap();
    table.otf.clear();
    table.pango.clear();
    Ok(Value::UNSPECIFIED)
}

/// The face of `file` (`C059-Roman`, `emmentaler-20`): the text pack a host
/// gave of that name, or an Emmentaler pack read for text, once.
fn text_face(cx: &mut Cx, file: &str) -> Result<Value, Thrown> {
    let all = all_fonts(cx);
    let table = cx.heap().foreign::<AllFontMetrics>(all).unwrap();
    if let Some(&(_, face)) = table.text.iter().find(|(n, _)| n == file) {
        return Ok(face);
    }
    let Some(&(_, pack)) = table.packs.iter().find(|(n, _)| n == file) else {
        return Err(missing(cx, file));
    };
    let font = TextFont::from_music(&cx.heap().foreign::<FontPack>(pack).unwrap().font);
    let heap = cx.heap_mut();
    let face = heap.wrap(TextPack {
        font,
        file: String::from(file),
        music: Some(pack),
        wants_outlines: core::cell::Cell::new(false),
    });
    let table = heap.foreign_mut::<AllFontMetrics>(all).unwrap();
    table.text.push((String::from(file), face));
    Ok(face)
}

/// `All_font_metrics::find_pango_font`: the Pango font of `description`,
/// the same for each description until `ly:reset-all-fonts`, made at
/// `output_scale` the first time; its face is fontconfig's choice
/// ([`face_files`]) of the packs a host gave, and the faces after it those
/// it falls back through.
pub fn find_pango_font(
    cx: &mut Cx,
    description: Description,
    is_emmentaler: bool,
    output_scale: f64,
) -> Result<Value, Thrown> {
    let key = description.to_filename();
    let all = all_fonts(cx);
    let table = cx.heap().foreign::<AllFontMetrics>(all).unwrap();
    if let Some(&(_, font)) = table.pango.iter().find(|(n, _)| *n == key) {
        return Ok(font);
    }
    let mut files = face_files(&description, is_emmentaler);
    let face = text_face(cx, &files[0])?;
    files.remove(0);
    let heap = cx.heap_mut();
    let font = PangoFont::new(heap, description, face, files, output_scale);
    let font = heap.wrap(font);
    let table = heap.foreign_mut::<AllFontMetrics>(all).unwrap();
    table.pango.push((key, font));
    Ok(font)
}

/// The faces Pango shapes `text` in with the Pango font `font`: its face,
/// and those it falls back through if a character needs them and `music`
/// (a music string, which falls back to none) is not set, each loaded, a
/// pack the host has not given a stop for it.
pub(crate) fn text_faces(
    cx: &mut Cx,
    font: Value,
    text: &str,
    music: bool,
) -> Result<Vec<Value>, Thrown> {
    let p = pango_font(cx.heap(), font).expect("a Pango font");
    let mut faces = alloc::vec![p.face];
    if music || p.fallback.is_empty() || !needs_fallback(cx.heap(), p.face, text) {
        return Ok(faces);
    }
    for file in p.fallback.clone() {
        faces.push(text_face(cx, &file)?);
    }
    Ok(faces)
}

/// `(ly:system-font-load name)`: the OpenType font `name`.
pub(crate) fn system_font_load(cx: &mut Cx, args: &[Value]) -> R {
    let name = string_arg(cx, "ly:system-font-load", 1, args[0])?;
    find_otf_font(cx, &name)
}

// --- arguments ------------------------------------------------------------------

/// `LY_ASSERT_SMOB (Font_metric, v, position)`.
fn metric_arg<'h>(
    cx: &'h mut Cx,
    subr: &str,
    position: usize,
    v: Value,
) -> Result<Metric<'h>, Thrown> {
    if metric(cx.heap(), v).is_none() {
        return Err(cx.wrong_type(subr, position, v, "Font_metric"));
    }
    Ok(metric(cx.heap(), v).unwrap())
}

/// `open-type-font-scheme.cc`'s `SCM_ASSERT_TYPE (otf, font, SCM_ARG1,
/// __FUNCTION__, "OpenType font")`: named by the C function, `function`;
/// a scaled font's is the font it scales.
fn otf_arg<'h>(cx: &'h mut Cx, function: &str, v: Value) -> Result<&'h Font, Thrown> {
    if open_type(cx.heap(), v).is_none() {
        return Err(cx.wrong_type(function, 1, v, "OpenType font"));
    }
    Ok(open_type(cx.heap(), v).unwrap())
}

/// `LY_ASSERT_TYPE (scm_is_string, …)`, then `from_scm<std::string>`.
fn string_arg(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<String, Thrown> {
    match cx.heap().str(v) {
        Some(s) => Ok(String::from(s.as_str())),
        None => Err(cx.wrong_type(subr, position, v, "string")),
    }
}

/// `LY_ASSERT_TYPE (scm_is_integer, …)`, then `from_scm<int>`: an inexact
/// integer is `scm_to_int`'s type error and one beyond an `int` its range
/// error, both from no procedure.
fn int_arg(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<i32, Thrown> {
    if !cx.heap().is_integer(v) {
        return Err(cx.wrong_type(subr, position, v, "integer"));
    }
    if !cx.heap().is_exact_integer(v) {
        return Err(cx.wrong_type_vm(None, v, "exact integer"));
    }
    match cx.heap().to_i64(v).and_then(|n| i32::try_from(n).ok()) {
        Some(n) => Ok(n),
        None => Err(cx.out_of_bounds(i64::from(i32::MIN), i32::MAX as u64, v)),
    }
}

/// A primitive that reads what W1's packs do not hold: `misc-error` by
/// name once its arguments are checked (D67).
#[cold]
fn unported(cx: &mut Cx, subr: &str, reads: &str) -> Thrown {
    let message = alloc::format!("reads {reads}, which W1's packs do not hold");
    cx.error("misc-error", Some(subr), &message, &[])
}

// --- font-metric-scheme.cc --------------------------------------------------------

/// `(ly:font-get-glyph font name)`; of a Pango font, which has no glyph
/// boxes of its own, a `named-glyph` of empty extents, as
/// `Font_metric::find_by_name` gives it.
pub(crate) fn get_glyph(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:font-get-glyph";
    if pango_font(cx.heap(), args[0]).is_some() {
        let name = string_arg(cx, SUBR, 2, args[1])?.replace('-', "M");
        let stencil = match pango_index(cx.heap(), args[0], &name) {
            Some(_) => {
                let heap = cx.heap_mut();
                let head = heap.intern("named-glyph");
                let string = heap.string(name);
                let expr = heap.list(&[head, args[0], string]);
                Stencil::new(
                    Box::new(Interval::new(0.0, 0.0), Interval::new(0.0, 0.0)),
                    expr,
                )
            }
            None => Stencil::default(),
        };
        return wrap(cx, stencil);
    }
    metric_arg(cx, SUBR, 1, args[0])?;
    let name = string_arg(cx, SUBR, 2, args[1])?;
    let stencil = find_by_name(cx.heap_mut(), args[0], &name);
    wrap(cx, stencil)
}

/// `(ly:font-glyph-name-to-index font name)`: -1 for a glyph the font has
/// not, `.notdef` among them.
pub(crate) fn glyph_name_to_index(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:font-glyph-name-to-index";
    if pango_font(cx.heap(), args[0]).is_some() {
        let name = string_arg(cx, SUBR, 2, args[1])?;
        let index = pango_index(cx.heap(), args[0], &name);
        return Ok(cx.heap_mut().integer(index.map_or(-1, i64::from)));
    }
    metric_arg(cx, SUBR, 1, args[0])?;
    let name = string_arg(cx, SUBR, 2, args[1])?;
    let index = metric_arg(cx, SUBR, 1, args[0])?.font.name_to_index(&name);
    Ok(cx
        .heap_mut()
        .integer(index.map_or(-1, |index| index as i64)))
}

/// `(ly:font-index-to-charcode font index)`: 0 for an index the cmap does
/// not map, a negative one among them.
pub(crate) fn index_to_charcode(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:font-index-to-charcode";
    if pango_font(cx.heap(), args[0]).is_some() {
        // `Font_metric::index_to_charcode`: the index itself.
        let i = int_arg(cx, SUBR, 2, args[1])?;
        return Ok(cx.heap_mut().integer(i64::from(i)));
    }
    metric_arg(cx, SUBR, 1, args[0])?;
    let i = int_arg(cx, SUBR, 2, args[1])?;
    let font = &metric_arg(cx, SUBR, 1, args[0])?.font;
    let code = usize::try_from(i)
        .ok()
        .and_then(|i| font.charcode(i))
        .unwrap_or(0);
    Ok(cx.heap_mut().integer(i64::from(code)))
}

/// `(ly:font-file-name font)`: the description's name, `"emmentaler-20"`;
/// `#f` for a Pango font.
pub(crate) fn file_name(cx: &mut Cx, args: &[Value]) -> R {
    if pango_font(cx.heap(), args[0]).is_some() {
        return Ok(Value::FALSE);
    }
    let description = metric_arg(cx, "ly:font-file-name", 1, args[0])?.description;
    Ok(cx.heap().car(description).unwrap_or(Value::FALSE))
}

/// `(ly:font-name font)`: the PostScript name, `"Emmentaler-20"`;
/// `Font_metric::font_name`'s `"unknown"` for a Pango font.
pub(crate) fn name(cx: &mut Cx, args: &[Value]) -> R {
    if pango_font(cx.heap(), args[0]).is_some() {
        return Ok(cx.heap_mut().string("unknown"));
    }
    let name = String::from(
        metric_arg(cx, "ly:font-name", 1, args[0])?
            .font
            .postscript_name(),
    );
    Ok(cx.heap_mut().string(name))
}

/// `(ly:font-magnification font)`: the description's, 1.0 for a font of
/// its own.
pub(crate) fn magnification(cx: &mut Cx, args: &[Value]) -> R {
    if pango_font(cx.heap(), args[0]).is_some() {
        return Ok(cx.heap_mut().flonum(1.0));
    }
    let description = metric_arg(cx, "ly:font-magnification", 1, args[0])?.description;
    let m = cx.heap().cdr(description).and_then(|m| cx.heap().to_f64(m));
    Ok(cx.heap_mut().flonum(m.unwrap_or(1.0)))
}

/// `(ly:font-design-size font)`: `LILY`'s `design_size` in points, in
/// LilyPond's internal unit.
pub(crate) fn design_size(cx: &mut Cx, args: &[Value]) -> R {
    if pango_font(cx.heap(), args[0]).is_some() {
        // `Font_metric::design_size`: a point.
        return Ok(cx.heap_mut().flonum(lily_engine_fonts::POINT));
    }
    let size = metric_arg(cx, "ly:font-design-size", 1, args[0])?
        .font
        .design_size();
    Ok(cx.heap_mut().flonum(size))
}

/// `(ly:font-metric? x)`.
pub(crate) fn is_font_metric(cx: &mut Cx, args: &[Value]) -> R {
    let heap = cx.heap();
    Ok(Value::bool(
        metric(heap, args[0]).is_some() || pango_font(heap, args[0]).is_some(),
    ))
}

/// `Pango_font::name_to_index`: `FT_Get_Name_Index` of the face; 0, the
/// `.notdef`, is none.
fn pango_index(heap: &Heap, font: Value, name: &str) -> Option<u16> {
    let face = heap.foreign::<TextPack>(pango_font(heap, font)?.face)?;
    let count = face.font.glyphs().len();
    (1..count)
        .map(|i| i as u16)
        .find(|&i| face.font.glyph_name(i) == name)
}

// --- open-type-font-scheme.cc -----------------------------------------------------

/// A glyph's `LILC` entry as Guile reads it: `((bbox x0 y0 x1 y1)
/// (attachment x . y) (attachment-down x . y) (ledger-shortening-range l .
/// r))`, the fields it has, in that order.
fn char_entry(heap: &mut Heap, m: &Metrics) -> Value {
    let mut fields = Vec::new();
    let bbox = m.bbox.map(|x| heap.flonum(x));
    let key = heap.intern("bbox");
    let values = heap.list(&bbox);
    fields.push(heap.cons(key, values));
    for (name, pair) in [
        ("attachment", m.attachment),
        ("attachment-down", m.attachment_down),
        ("ledger-shortening-range", m.ledger_shortening_range),
    ] {
        if let Some([a, b]) = pair {
            let key = heap.intern(name);
            let (a, b) = (heap.flonum(a), heap.flonum(b));
            let pair = heap.cons(a, b);
            fields.push(heap.cons(key, pair));
        }
    }
    heap.list(&fields)
}

/// `(ly:otf-font-glyph-info font glyph)`: its `LILC` entry, or `()`.
pub(crate) fn glyph_info(cx: &mut Cx, args: &[Value]) -> R {
    otf_arg(cx, "ly_otf_font_glyph_info", args[0])?;
    let glyph = string_arg(cx, "ly:otf-font-glyph-info", 2, args[1])?;
    let font = otf_arg(cx, "ly_otf_font_glyph_info", args[0])?;
    Ok(match font.metrics(&glyph) {
        Some(m) => char_entry(cx.heap_mut(), &m),
        None => Value::NULL,
    })
}

/// `(ly:otf-font-table-data font tag)`: the tag checked, then a
/// placeholder.
pub(crate) fn table_data(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:otf-font-table-data";
    otf_arg(cx, "ly_otf_font_table_data", args[0])?;
    let tag = string_arg(cx, SUBR, 2, args[1])?;
    if tag.len() > 4 {
        return Err(cx.error(
            "misc-error",
            Some(SUBR),
            "tag string must have 4 characters at most: ~s",
            &[args[1]],
        ));
    }
    Err(unported(cx, SUBR, "the font's tables"))
}

/// `(ly:otf-font? x)`.
pub(crate) fn is_otf_font(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(open_type(cx.heap(), args[0]).is_some()))
}

/// `(ly:otf-glyph-count font)`: the glyphs that the cmap maps.
pub(crate) fn glyph_count(cx: &mut Cx, args: &[Value]) -> R {
    let count = otf_arg(cx, "ly_otf_glyph_count", args[0])?.count();
    Ok(cx.heap_mut().integer(count as i64))
}

/// `(ly:otf-glyph-list font)`: every glyph's name, `.notdef` first.
pub(crate) fn glyph_list(cx: &mut Cx, args: &[Value]) -> R {
    let names: Vec<String> = otf_arg(cx, "ly_otf_glyph_list", args[0])?
        .glyphs()
        .iter()
        .map(|g| g.name.clone())
        .collect();
    let heap = cx.heap_mut();
    let names: Vec<Value> = names.into_iter().map(|n| heap.string(n)).collect();
    Ok(heap.list(&names))
}

/// The optional font index of the file functions, checked as they check
/// it.
fn index_arg(cx: &mut Cx, subr: &str, args: &[Value]) -> Result<(), Thrown> {
    if let Some(&idx) = args.get(1) {
        int_arg(cx, subr, 2, idx)?;
    }
    Ok(())
}

/// `(ly:get-font-format file [idx])`: a placeholder once its arguments
/// are checked.
pub(crate) fn get_font_format(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:get-font-format";
    string_arg(cx, SUBR, 1, args[0])?;
    index_arg(cx, SUBR, args)?;
    Err(unported(cx, SUBR, "a font file's format"))
}

/// `(ly:has-glyph-names? file [idx])`: a placeholder.
pub(crate) fn has_glyph_names(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:has-glyph-names?";
    string_arg(cx, SUBR, 1, args[0])?;
    index_arg(cx, SUBR, args)?;
    Err(unported(cx, SUBR, "a font file's glyph names"))
}

/// `(ly:get-cff-offset file [idx])`: a placeholder.
pub(crate) fn get_cff_offset(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:get-cff-offset";
    string_arg(cx, SUBR, 1, args[0])?;
    index_arg(cx, SUBR, args)?;
    Err(unported(cx, SUBR, "a font file's CFF offset"))
}

/// `(ly:extract-subfont-from-collection collection idx subfont)`: a
/// placeholder.
pub(crate) fn extract_subfont(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:extract-subfont-from-collection";
    string_arg(cx, SUBR, 1, args[0])?;
    if !cx.heap().is_integer(args[1]) {
        return Err(cx.wrong_type(SUBR, 2, args[1], "integer"));
    }
    string_arg(cx, SUBR, 3, args[2])?;
    int_arg(cx, SUBR, 2, args[1])?;
    Err(unported(cx, SUBR, "a font collection's file"))
}
