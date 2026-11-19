//! Font selection (`font-select.cc`, `font-interface.cc`, `paper-def.cc`'s
//! `find_scaled_font`, `output-def-scheme.cc`'s `ly:paper-get-font`): the
//! font a grob or a markup gets from its property chain, as
//! [`select_font`] reads it. `fonts` names each family's font
//! (`paper-defaults-init.ly`'s `property-defaults.fonts`, `music`
//! `"emmentaler"`); `font-encoding` says whether it is a music font
//! (`fetaMusic`, `fetaBraces`) or text (`fetaText`, `latin1`, or any
//! `font-name`); and `font-size`, in steps of 2^(1/6), with the output
//! definition's `staff-height`, gives the size asked for. A music font is
//! Emmentaler's optical size nearest to it (`feta-design-size-mapping`)
//! loaded from its pack (`font::find_otf_font`), then scaled by what is
//! left over and divided by `output-scale` as a `Modified_font_metric`,
//! which each output definition keeps in its `scaled-fonts`, one per font
//! and magnification ([`find_scaled_font`]).
//!
//! A grob's chain is `Font_interface::music_font_alist_chain`'s: its
//! properties, then [`MUSIC_DEFAULTS`] ([`music_font_alist_chain`]), and
//! `Font_interface::get_default_font` (MS3 step 14, [`get_default_font`],
//! `ly:grob-default-font`) the font it selects in the grob's `layout`,
//! kept in its `font` property. A text font
//! (`latin1`, `fetaText`, or a `font-name`) is a Pango font (`pango.rs`,
//! MS3 step 8): a description of the family `fonts` names, tweaked by the
//! chain's `font-variant`, `font-shape`, `font-series` and `font-stretch`
//! (`pango-select.cc`), at the size asked for in Pango units, from
//! `All_font_metrics` ([`find_pango_font`]), and kept in the definition's
//! `pango-fonts`. An output definition has no parent in the engine
//! yet (`lily_engine_music::OutputDef`), so its own scope is the one
//! `lookup_variable` and `find_scaled_font` read, as a `\paper`'s is in
//! LilyPond.
//!
//! Since MS3 step 50 (W5, D208) [`want_fonts`] names the music fonts a
//! translation's grobs would select, before its layout asks for them, so
//! that the core asks the host for their packs first.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{Host, OutputDef, host_lookup_variable, lookup_variable, root};
use lily_engine_scheme::{Cx, Exception, Foreign, Heap, Tag, Thrown, Tracer, Value};

use crate::R;
use crate::font::{find_otf_font, find_pango_font, make_scaled, want};
use crate::pango::{Description, Style};

/// `point_constant`: a point in LilyPond's internal unit, the millimetre,
/// as `dimensions.hh` computes it.
pub(crate) const POINT: f64 = 1.0 / (72.270 / 2.54 / 10.0);

/// What `Font_interface::music_font_alist_chain` puts after a grob's
/// properties: `((font-family . music) (font-encoding . fetaMusic))`.
pub const MUSIC_DEFAULTS: [(&str, &str); 2] =
    [("font-family", "music"), ("font-encoding", "fetaMusic")];

/// An output definition's `scaled-fonts`: for each font, the fonts scaled
/// from it by magnification, newest first. LilyPond keeps a hash table of
/// alists; a clone of the definition shares it, as it shares LilyPond's.
#[derive(Default)]
pub struct ScaledFonts {
    fonts: Vec<(Value, Vec<(f64, Value)>)>,
}

impl Foreign for ScaledFonts {
    fn type_name(&self) -> &'static str {
        "scaled-fonts"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for (font, sizes) in &self.fonts {
            tracer.trace(*font);
            for &(_, scaled) in sizes {
                tracer.trace(scaled);
            }
        }
    }
}

/// `font-encoding`, as `select_font` takes it apart.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Music,
    Braces,
    Text,
    Latin1,
}

/// `v` is the symbol `name`.
fn is_symbol(heap: &Heap, v: Value, name: &str) -> bool {
    v.tag() == Tag::Symbol && heap.name(v) == Some(name)
}

/// `ly_chain_assoc_get (key, chain, …)`: the value of `key` in the first
/// alist of `chain` that has it.
pub(crate) fn chain_get(heap: &mut Heap, key: &str, chain: Value) -> Option<Value> {
    let key = heap.intern(key);
    let mut at = chain;
    while let (Some(alist), Some(next)) = (heap.car(at), heap.cdr(at)) {
        let mut entries = alist;
        while let (Some(entry), Some(rest)) = (heap.car(entries), heap.cdr(entries)) {
            if heap.car(entry) == Some(key) {
                return heap.cdr(entry);
            }
            entries = rest;
        }
        at = next;
    }
    None
}

/// [`chain_get`] of each of `keys` (symbols), the chain walked once.
fn chain_get_all<const N: usize>(
    heap: &Heap,
    keys: &[Value; N],
    chain: Value,
) -> [Option<Value>; N] {
    let mut found = [None; N];
    let mut at = chain;
    while let (Some(alist), Some(next)) = (heap.car(at), heap.cdr(at)) {
        let mut entries = alist;
        while let (Some(entry), Some(rest)) = (heap.car(entries), heap.cdr(entries)) {
            if let Some(key) = heap.car(entry)
                && let Some(i) = keys.iter().position(|&k| k == key)
                && found[i].is_none()
            {
                found[i] = heap.cdr(entry);
            }
            entries = rest;
        }
        at = next;
    }
    found
}

/// `ly_assoc_get (key, alist, SCM_BOOL_F)`: by `equal?`.
fn assoc_get(heap: &Heap, key: Value, mut alist: Value) -> Value {
    while let (Some(entry), Some(rest)) = (heap.car(alist), heap.cdr(alist)) {
        if let Some(k) = heap.car(entry)
            && heap.equal(k, key)
        {
            return heap.cdr(entry).unwrap_or(Value::FALSE);
        }
        alist = rest;
    }
    Value::FALSE
}

/// `Output_def::lookup_variable (name)` of the definition `def`, through
/// its parents, then `from_scm<double>`: Guile's type error for a value
/// that is no real, unbound among them.
fn dimension(cx: &mut Cx, def: Value, name: &str) -> Result<f64, Thrown> {
    let symbol = cx.heap_mut().intern(name);
    let value = lookup_variable(cx, def, symbol).unwrap_or(Value::UNDEFINED);
    crate::to_double(cx, value)
}

/// `heap.write (v)`, as `ly_scm_write_string` gives it.
fn written(heap: &Heap, v: Value) -> String {
    let mut out = String::new();
    heap.write(v, &mut out);
    out
}

/// `best_rounded_design_size`: of `feta-design-size-mapping`'s optical
/// sizes, the one whose design size is nearest to `requested` by ratio,
/// and that design size.
fn best_rounded_design_size(heap: &Heap, mut mapping: Value, requested: f64) -> (i64, f64) {
    let (mut min_ratio, mut best) = (f64::INFINITY, (0, 0.0));
    while let (Some(pair), Some(rest)) = (heap.car(mapping), heap.cdr(mapping)) {
        let rounded = heap.car(pair).and_then(|r| heap.to_i64(r));
        let actual = heap.cdr(pair).and_then(|a| heap.to_f64(a));
        if let (Some(rounded), Some(actual)) = (rounded, actual) {
            let ratio = if requested > actual {
                requested / actual
            } else {
                actual / requested
            };
            if ratio < min_ratio {
                min_ratio = ratio;
                best = (rounded, actual);
            }
        }
        mapping = rest;
    }
    best
}

/// `select_font (layout, chain)`: the font the property chain `chain`
/// selects in the output definition `layout`, and the warnings LilyPond
/// gives on the way (an encoding it does not know, a family `fonts` does
/// not name, a font property's value it does not know). A music font is a
/// `Modified_font_metric` ([`find_scaled_font`]), a text font a Pango font.
pub fn select_font(
    cx: &mut Cx,
    layout: Value,
    chain: Value,
) -> Result<(Value, Vec<String>), Thrown> {
    assert!(
        cx.heap().foreign::<OutputDef>(layout).is_some(),
        "select_font is given an output definition"
    );
    let mut warnings = Vec::new();
    let heap = cx.heap_mut();
    let fonts = chain_get(heap, "fonts", chain).unwrap_or(Value::NULL);
    let description = chain_get(heap, "font-name", chain).unwrap_or(Value::FALSE);
    let encoding = chain_get(heap, "font-encoding", chain).unwrap_or(Value::FALSE);
    let encoding = if heap.str(description).is_some() {
        Encoding::Latin1
    } else if is_symbol(heap, encoding, "fetaMusic") {
        Encoding::Music
    } else if is_symbol(heap, encoding, "fetaBraces") {
        Encoding::Braces
    } else if is_symbol(heap, encoding, "fetaText") {
        Encoding::Text
    } else if is_symbol(heap, encoding, "latin1") {
        Encoding::Latin1
    } else {
        warnings.push(format!(
            "font-encoding is invalid, should be 'fetaMusic, 'fetaBraces, 'fetaText or 'latin1: {}",
            written(heap, encoding)
        ));
        warnings.push(String::from("falling back to latin1"));
        Encoding::Latin1
    };
    let feta = encoding != Encoding::Latin1;

    let base_size = if feta {
        dimension(cx, layout, "staff-height")? / POINT
    } else {
        dimension(cx, layout, "text-font-size")? * POINT
    };
    let heap = cx.heap_mut();
    let step = chain_get(heap, "font-size", chain)
        .filter(|&s| heap.is_real(s))
        .and_then(|s| heap.to_f64(s))
        .unwrap_or(0.0);
    // `pow (2.0, step / 6.0)`: libm's `exp2`, which gives what LilyPond's
    // `pow` on macOS gives at every tenth, quarter and half step from -12
    // to 12, where libm's `pow` is a unit in the last place off at a
    // quarter of them (MS3 step 5).
    let requested = base_size * libm::exp2(step / 6.0);

    let family = if feta {
        let family = chain_get(heap, "font-family", chain).unwrap_or(Value::FALSE);
        // As LilyPond: `\markup \sans { più \dynamic p }` asks so.
        if family == Value::FALSE
            || ["serif", "sans", "typewriter"]
                .iter()
                .any(|name| is_symbol(heap, family, name))
        {
            heap.intern("music")
        } else {
            family
        }
    } else {
        match chain_get(heap, "font-family", chain) {
            Some(family) => family,
            None => heap.intern("serif"),
        }
    };
    let name = assoc_get(heap, family, fonts);
    let mut name = if name == Value::FALSE {
        warnings.push(format!(
            "no entry for font family {} in fonts alist",
            written(heap, family)
        ));
        String::from("LilyPond Serif")
    } else if let Some(name) = heap.str(name) {
        String::from(name.as_str())
    } else {
        warnings.push(format!(
            "expected string for value in fonts alist, found: {}",
            written(heap, name)
        ));
        String::from("LilyPond Serif")
    };

    let base_name = name.clone();
    let mut actual = 0.0;
    if feta {
        let mapping = cx
            .global("lily", "feta-design-size-mapping")
            .unwrap_or(Value::NULL);
        let (rounded, size) = best_rounded_design_size(cx.heap(), mapping, requested);
        actual = size;
        name.push('-');
        if encoding == Encoding::Braces {
            name.push_str("brace");
        } else {
            name.push_str(&format!("{rounded}"));
        }
    }

    match encoding {
        Encoding::Music | Encoding::Braces => {
            let font = find_otf_font(cx, &name)?;
            let scaled = find_scaled_font(cx, layout, font, requested / actual)?;
            Ok((scaled, warnings))
        }
        Encoding::Text | Encoding::Latin1 => {
            let heap = cx.heap_mut();
            let (mut desc, is_emmentaler) = match heap.str(description) {
                Some(text) => {
                    let text = String::from(text.as_str());
                    let is_emmentaler = text.to_lowercase() == "emmentaler";
                    (Description::from_string(&text), is_emmentaler)
                }
                None => {
                    let mut desc = Description::new(&name);
                    if encoding == Encoding::Latin1 {
                        tweak_description(heap, &mut desc, chain, &mut warnings);
                    }
                    (desc, base_name.to_lowercase() == "emmentaler")
                }
            };
            let requested = if encoding == Encoding::Text {
                requested * POINT
            } else {
                requested
            };
            desc.size = Some(libm::round(requested * 1024.0) as i32);
            let font = find_pango_font_in(cx, layout, desc, is_emmentaler)?;
            Ok((font, warnings))
        }
    }
}

/// The interfaces of the grobs whose print draws a glyph of the font their
/// own chain selects (`Font_interface::get_default_font`), which
/// [`want_fonts`] reads: not a paper column's, slur's or tie's, whose
/// `font-size` is for debugging, nor a text's, whose font is its markup's
/// (a grob of `text-interface` among them, as a `TabNoteHead`, since MS4
/// step 7a).
const GLYPHS: [&str; 12] = [
    "note-head-interface",
    "rest-interface",
    "accidental-interface",
    "dots-interface",
    "clef-interface",
    "flag-interface",
    "script-interface",
    "key-signature-interface",
    "time-signature-interface",
    "custos-interface",
    "arpeggio-interface",
    "system-start-delimiter-interface",
];

/// The music fonts the grobs of the Paper_score `pscore` would select, as
/// [`select_font`] names them from each grob's chain at its layout's
/// `staff-height` (its staff size, `\layout`'s, its `font-size`, a grace
/// note's), each noted for the host to be asked for unless a host gave it
/// (W5, D208): of the grobs of [`GLYPHS`]; no font is loaded and no
/// callback called, so a `font-size` a callback gives is 0 here, and a
/// font another grob or a markup selects is noted when it is loaded.
pub fn want_fonts(host: &mut dyn Host, pscore: Value) {
    let (Some(system), Some(layout)) = (
        crate::paper_score::root_system(host.heap(), pscore),
        crate::paper_score::layout(host.heap(), pscore),
    ) else {
        return;
    };
    let symbol = host.heap().intern("staff-height");
    let staff = host_lookup_variable(host, layout, symbol);
    let (Some(staff), Some(mapping), Some(all)) = (
        staff.and_then(|s| host.heap().to_f64(s)),
        host.lily("feta-design-size-mapping"),
        host.lily("%all-fonts"),
    ) else {
        return;
    };
    let heap = host.heap();
    let elements = heap.intern("all-elements");
    let elements = crate::grob::object_data(heap, system, elements);
    let glyphs = GLYPHS.map(|i| heap.intern(i));
    let text = [heap.intern("text-interface")];
    let keys = [
        "font-encoding",
        "font-name",
        "font-size",
        "font-family",
        "fonts",
    ]
    .map(|k| heap.intern(k));
    for grob in crate::grob_array::grobs_of(heap, elements) {
        if !crate::grob::has_any_interface(heap, grob, &glyphs)
            || crate::grob::has_any_interface(heap, grob, &text)
        {
            continue;
        }
        let chain = music_font_alist_chain(heap, grob);
        let [encoding, name, size, family, fonts] = chain_get_all(heap, &keys, chain);
        let encoding = encoding.unwrap_or(Value::FALSE);
        let braces = is_symbol(heap, encoding, "fetaBraces");
        let named = name.is_some_and(|n| heap.str(n).is_some());
        if named || !(braces || is_symbol(heap, encoding, "fetaMusic")) {
            continue;
        }
        let step = size
            .filter(|&s| heap.is_real(s))
            .and_then(|s| heap.to_f64(s))
            .unwrap_or(0.0);
        let family = match family {
            Some(f)
                if f != Value::FALSE
                    && !["serif", "sans", "typewriter"]
                        .iter()
                        .any(|name| is_symbol(heap, f, name)) =>
            {
                f
            }
            _ => heap.intern("music"),
        };
        let fonts = fonts.unwrap_or(Value::NULL);
        let name = assoc_get(heap, family, fonts);
        let Some(name) = heap.str(name).map(|n| String::from(n.as_str())) else {
            continue;
        };
        let name = match braces {
            true => format!("{name}-brace"),
            false => {
                let requested = staff / POINT * libm::exp2(step / 6.0);
                format!(
                    "{name}-{}",
                    best_rounded_design_size(heap, mapping, requested).0
                )
            }
        };
        want(heap, all, &name);
    }
}

/// `find_pango_font (layout, description, is_emmentaler)` of `paper-def.cc`:
/// the font from `All_font_metrics` at the `output-scale` of the root of
/// the definition's parents, added to the root's `pango-fonts`.
fn find_pango_font_in(cx: &mut Cx, layout: Value, desc: Description, is_emmentaler: bool) -> R {
    let layout = root(cx.heap(), layout);
    let scope = cx
        .heap()
        .foreign::<OutputDef>(layout)
        .map(|def| def.scope)
        .expect("find_pango_font is given an output definition");
    let output_scale = dimension(cx, layout, "output-scale")?;
    let font = find_pango_font(cx, desc, is_emmentaler, output_scale)?;
    let key = cx.heap_mut().intern("pango-fonts");
    let fonts = cx.module_ref(scope, key).unwrap_or(Value::NULL);
    let fonts = cx.heap_mut().cons(font, fonts);
    cx.module_define(scope, key, fonts);
    Ok(font)
}

/// `tweak_pango_description`: the chain's `font-variant`, `font-shape`,
/// `font-series` and `font-stretch` in `desc`, each symbol it does not know
/// a warning and its default.
fn tweak_description(
    heap: &mut Heap,
    desc: &mut Description,
    chain: Value,
    warnings: &mut Vec<String>,
) {
    let property = |heap: &mut Heap, key: &str| {
        chain_get(heap, key, chain)
            .filter(|v| v.tag() == Tag::Symbol)
            .and_then(|v| heap.name(v).map(String::from))
    };
    let variant = property(heap, "font-variant");
    desc.small_caps = match variant.as_deref() {
        None | Some("normal") => false,
        Some("small-caps") => true,
        Some(v) => {
            warnings.push(format!(
                "unknown font-variant value {v}; allowed values are normal and small-caps"
            ));
            false
        }
    };
    let shape = property(heap, "font-shape");
    desc.style = match shape.as_deref() {
        None | Some("upright") => Style::Normal,
        Some("italic") => Style::Italic,
        Some("oblique" | "slanted") => Style::Oblique,
        Some(v) => {
            warnings.push(format!(
                "unknown font-shape value {v}; allowed values are italic, oblique and upright"
            ));
            Style::Normal
        }
    };
    let series = property(heap, "font-series");
    desc.weight = match series.as_deref() {
        None | Some("normal" | "regular") => 400,
        Some("thin") => 100,
        Some("ultralight" | "extralight") => 200,
        Some("light") => 300,
        Some("semilight" | "demilight") => 350,
        Some("book") => 380,
        Some("medium") => 500,
        Some("semibold" | "demibold") => 600,
        Some("bold") => 700,
        Some("ultrabold" | "extrabold") => 800,
        Some("heavy" | "black") => 900,
        Some("ultraheavy" | "ultrablack" | "extrablack") => 1000,
        Some(v) => {
            warnings.push(format!(
                "unknown font-series value {v}; allowed values are thin, ultralight (or extralight), light, semilight (or demilight), book, normal (or regular), medium, semibold (or demibold), bold, ultrabold (or extrabold), heavy (or black), and ultraheavy (or ultrablack or extrablack)"
            ));
            400
        }
    };
    const STRETCHES: [&str; 9] = [
        "ultra-condensed",
        "extra-condensed",
        "condensed",
        "semi-condensed",
        "normal",
        "semi-expanded",
        "expanded",
        "extra-expanded",
        "ultra-expanded",
    ];
    let stretch = property(heap, "font-stretch");
    desc.stretch = match stretch.as_deref() {
        None => 4,
        Some(v) => match STRETCHES.iter().position(|s| *s == v) {
            Some(i) => i as u8,
            None => {
                warnings.push(format!(
                    "unknown font-stretch value {v}; allowed values are ultra-condensed, extra-condensed, condensed, semi-condensed, normal, semi-expanded, extra-expanded and ultra-expanded"
                ));
                4
            }
        },
    };
}

/// `find_scaled_font (layout, font, m)`: `font` scaled by `m`, divided by
/// the `output-scale` of the root of the definition's parents, the same
/// scaled font each time the root is asked for it.
pub fn find_scaled_font(cx: &mut Cx, layout: Value, font: Value, m: f64) -> R {
    let layout = root(cx.heap(), layout);
    let scope = cx
        .heap()
        .foreign::<OutputDef>(layout)
        .map(|def| def.scope)
        .expect("find_scaled_font is given an output definition");
    let lookup_mag = m / dimension(cx, layout, "output-scale")?;
    let key = cx.heap_mut().intern("scaled-fonts");
    let table = match cx
        .module_ref(scope, key)
        .filter(|&t| cx.heap().foreign::<ScaledFonts>(t).is_some())
    {
        Some(table) => table,
        None => {
            let table = cx.heap_mut().wrap(ScaledFonts::default());
            cx.module_define(scope, key, table);
            table
        }
    };
    let fonts = &cx.heap().foreign::<ScaledFonts>(table).unwrap().fonts;
    // `ly_assoc` of the magnification: `equal?` of two reals.
    let found = fonts
        .iter()
        .find(|(f, _)| *f == font)
        .and_then(|(_, sizes)| {
            sizes
                .iter()
                .find(|(m, _)| m.to_bits() == lookup_mag.to_bits())
        });
    if let Some(&(_, scaled)) = found {
        return Ok(scaled);
    }
    let heap = cx.heap_mut();
    let scaled = make_scaled(heap, font, lookup_mag);
    let fonts = &mut heap.foreign_mut::<ScaledFonts>(table).unwrap().fonts;
    match fonts.iter_mut().find(|(f, _)| *f == font) {
        Some((_, sizes)) => sizes.insert(0, (lookup_mag, scaled)),
        None => fonts.push((font, alloc::vec![(lookup_mag, scaled)])),
    }
    Ok(scaled)
}

/// `(ly:paper-get-font def chain)`: the font the alist chain `chain`
/// selects in `def`, after LilyPond's warnings.
pub(crate) fn paper_get_font(cx: &mut Cx, args: &[Value]) -> R {
    if cx.heap().foreign::<OutputDef>(args[0]).is_none() {
        return Err(cx.wrong_type("ly:paper-get-font", 1, args[0], "Output_def"));
    }
    let (font, warnings) = select_font(cx, args[0], args[1])?;
    crate::warned(cx, font, &warnings)
}

/// `Font_interface::music_font_alist_chain`: the grob's alist chain
/// (`grob.rs`' `alist_chain`) with [`MUSIC_DEFAULTS`] as its global
/// default.
pub fn music_font_alist_chain(heap: &mut Heap, grob: Value) -> Value {
    let defaults: Vec<Value> = MUSIC_DEFAULTS
        .iter()
        .map(|(key, value)| {
            let (key, value) = (heap.intern(key), heap.intern(value));
            heap.cons(key, value)
        })
        .collect();
    let defaults = heap.list(&defaults);
    crate::grob::alist_chain(heap, grob, defaults)
}

/// `(%music-font-alist-chain grob)`: [`music_font_alist_chain`].
pub(crate) fn music_font_alist_chain_prim(cx: &mut Cx, args: &[Value]) -> R {
    crate::grob::grob_arg(cx, "%music-font-alist-chain", 1, args[0])?;
    Ok(music_font_alist_chain(cx.heap_mut(), args[0]))
}

/// `Font_interface::get_default_font`: the grob's `font`, if it is a font,
/// else the font its [`music_font_alist_chain`] selects in its `layout`,
/// then kept as its `font`, as [`PRELUDE`]'s `ly:grob-default-font`
/// gives it.
pub fn get_default_font(host: &mut dyn Host, grob: Value) -> Result<Value, Exception> {
    let f = crate::lily_procedure(host, "ly:grob-default-font")?;
    host.call(f, &[grob])
}

/// `ly:grob-default-font` (`grob-scheme.cc:397–406`) over
/// `get_default_font`, which reads `font` through its callback.
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:grob-default-font grob)
  (%check-type "ly:grob-default-font" 1 ly:grob? "Grob" grob)
  (let ((font (ly:grob-property grob 'font)))
    (if (ly:font-metric? font)
        font
        (let ((font (ly:paper-get-font (ly:grob-layout grob) (%music-font-alist-chain grob))))
          (%grob-set-property-data! grob 'font font)
          font))))
"#;
