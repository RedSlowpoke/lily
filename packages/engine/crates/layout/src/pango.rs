//! LilyPond's text fonts (`pango-font.cc`, `pango-font-scheme.cc`, and of
//! `pango-select.cc` and `all-font-metrics.cc` what selects them) over W1's
//! text packs (06 §5.3), as LilyPond 2.26.0's Pango 1.58.2, HarfBuzz 14.4.0
//! and fontconfig measure them (MS3 step 8):
//!
//! - [`Description`]: a `PangoFontDescription`, its family, weight, style,
//!   variant, stretch and size in Pango units, read and written as
//!   `pango_font_description_from_string` and `_to_string` do, the size
//!   with the fewest decimals that read back to it.
//! - [`PangoFont`]: `Pango_font`, a description, the face the packs give
//!   for it and `scale_`, `INCH_TO_BP / (PANGO_SCALE * PANGO_RESOLUTION *
//!   output_scale)`. The face is fontconfig's choice made by name: the
//!   families LilyPond names (`LilyPond Serif`, `LilyPond Sans Serif`,
//!   `LilyPond Monospace`), the URW families and the generic `serif`, `sans`
//!   and `monospace` are C059, Nimbus Sans and Nimbus Mono PS, and "New
//!   Century Schoolbook" LilyPond's TeX Gyre Schola (fontconfig's metric
//!   alias, E23), their bold from weight 600 and their italic for an italic
//!   or oblique style; any other family is C059, as fontconfig would give
//!   some serif; an
//!   Emmentaler family is that Emmentaler's pack, read for text
//!   ([`TextFont::from_music`]) with its `kern` and GSUB features (W6,
//!   D209).
//! - [`itemize`]: Pango's runs (MS4 step 19, W6): split where the script
//!   changes (`pango-script.c`'s iterator over [`unicode::script`], its
//!   paired characters taking their opener's) and where the face changes,
//!   each character in the first of the font's faces that has it (the
//!   description's families, [`face_files`]), a space or another character
//!   that selects no font in its neighbours', and one no face has in the
//!   first; a music string in its own face alone.
//! - [`text_stencil`]: `Pango_font::text_stencil`, each run shaped
//!   ([`TextFont::shape`]) into a `glyph-string` after the one before, in a
//!   `utf-8-string`, its numbers as Pango gives them: the font loaded at `Q`, the size's pixels
//!   at 1200 dpi in Pango units rounded; the size it describes `Q` back to
//!   points, truncated; each advance and kerning HarfBuzz's `em_mult` of
//!   `Q`, their sum rounded to whole pixels; ink boxes HarfBuzz's extents at
//!   `Q / units-per-em` in `f32`, rounded outward, which in Pango's y is
//!   inward. A probe of Pango over 609 runs and 45,066 glyphs of C059 and
//!   every size from 1,500 to 9,000 Pango units found no other rounding. A
//!   space the face has not is HarfBuzz's fallback: the face's space at the
//!   width [`TextFont::fallback_advance`] gives.
//!
//! Not as Pango: the faces are the packs' only, so a character LilyPond's
//! fontconfig takes from a font of the machine (DejaVu, Noto, the system's
//! serif) or of LilyPond's TeX Gyre is no glyph and LilyPond's warning,
//! with its Unicode name where [`unicode::formal_name`] has it, as when no
//! font has it; runs are not reordered for a right-to-left script, and a
//! newline does not start a line; a font feature a pack does not hold does
//! nothing (a text pack holds `liga` and `kern`, an Emmentaler pack `kern`,
//! `cv47`, `dlig`, `ss01` and `tnum`).

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{format, vec};

use lily_engine_fonts::unicode::{self, Script};
use lily_engine_fonts::{Features, TextFont};
use lily_engine_scheme::{Cx, Foreign, Heap, Tracer, Value};

use crate::R;
use crate::geometry::{Box, Interval, X};
use crate::stencil::Stencil;

/// `PANGO_SCALE`.
const PANGO_SCALE: i64 = 1024;
/// `pango-font.hh`'s `PANGO_RESOLUTION`, dots per inch.
const PANGO_RESOLUTION: i64 = 1200;
/// `dimensions.hh`'s `INCH_TO_BP`.
const INCH_TO_BP: f64 = 72.0;
/// How a glyph string names a font's file: below LilyPond's data
/// directory, which the engine names as `lily_engine_parser::source::DATADIR`.
const FONT_DIR: &str = "<datadir>/fonts/otf/";

// --- PangoFontDescription -------------------------------------------------------

/// `PangoStyle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Normal,
    Oblique,
    Italic,
}

/// A `PangoFontDescription` as LilyPond makes them: a family, a size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Description {
    pub family: String,
    /// `PangoWeight`: 400 normal, 700 bold.
    pub weight: u32,
    pub style: Style,
    pub small_caps: bool,
    /// `PangoStretch`: 0 ultra-condensed to 8 ultra-expanded, 4 normal.
    pub stretch: u8,
    /// In Pango units, a 1024th of a point; `None` for none.
    pub size: Option<i32>,
}

const WEIGHTS: [(u32, &str); 11] = [
    (100, "Thin"),
    (200, "Ultra-Light"),
    (300, "Light"),
    (350, "Semi-Light"),
    (380, "Book"),
    (500, "Medium"),
    (600, "Semi-Bold"),
    (700, "Bold"),
    (800, "Ultra-Bold"),
    (900, "Heavy"),
    (1000, "Ultra-Heavy"),
];

const STRETCHES: [&str; 9] = [
    "Ultra-Condensed",
    "Extra-Condensed",
    "Condensed",
    "Semi-Condensed",
    "",
    "Semi-Expanded",
    "Expanded",
    "Extra-Expanded",
    "Ultra-Expanded",
];

/// The style words `pango_font_description_from_string` reads after the
/// family, and what each sets.
fn style_word(desc: &mut Description, word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    match lower.as_str() {
        "normal" | "roman" => {}
        "italic" => desc.style = Style::Italic,
        "oblique" => desc.style = Style::Oblique,
        "small-caps" => desc.small_caps = true,
        _ => {
            if let Some(&(w, _)) = WEIGHTS.iter().find(|(_, n)| n.eq_ignore_ascii_case(word)) {
                desc.weight = w;
            } else if let Some(s) = STRETCHES
                .iter()
                .position(|n| !n.is_empty() && n.eq_ignore_ascii_case(word))
            {
                desc.stretch = s as u8;
            } else {
                return false;
            }
        }
    }
    true
}

impl Description {
    /// `pango_font_description_new` with the family `family`.
    pub fn new(family: &str) -> Description {
        Description {
            family: String::from(family),
            weight: 400,
            style: Style::Normal,
            small_caps: false,
            stretch: 4,
            size: None,
        }
    }

    /// `pango_font_description_from_string`: a size last (`pango_units_from_double`
    /// of it), the style words before it, the family the rest, without a
    /// trailing comma.
    pub fn from_string(text: &str) -> Description {
        let mut desc = Description::new("");
        let mut words: Vec<&str> = text.split_whitespace().collect();
        if let Some(size) = words.last().and_then(|w| w.parse::<f64>().ok())
            && size >= 0.0
        {
            desc.size = Some(libm::floor(size * PANGO_SCALE as f64 + 0.5) as i32);
            words.pop();
        }
        while let Some(&word) = words.last() {
            if !style_word(&mut desc, word) {
                break;
            }
            words.pop();
        }
        let family = words.join(" ");
        desc.family = String::from(family.trim_end_matches(','));
        desc
    }

    /// `pango_font_description_to_filename`: the string, lower case, with
    /// `_` for each space and comma; `All_font_metrics`' key.
    pub fn to_filename(&self) -> String {
        self.to_string()
            .chars()
            .map(|c| match c {
                ' ' | ',' => '_',
                c => c.to_ascii_lowercase(),
            })
            .collect()
    }
}

impl core::fmt::Display for Description {
    /// `pango_font_description_to_string`.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut out = if self.family.is_empty() {
            String::new()
        } else {
            self.family.clone()
        };
        let mut field = |word: &str| {
            if !word.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(word);
            }
        };
        if let Some(&(_, w)) = WEIGHTS.iter().find(|(w, _)| *w == self.weight) {
            field(w);
        }
        field(match self.style {
            Style::Normal => "",
            Style::Oblique => "Oblique",
            Style::Italic => "Italic",
        });
        field(STRETCHES[usize::from(self.stretch.min(8))]);
        field(if self.small_caps { "Small-Caps" } else { "" });
        if out.is_empty() {
            out.push_str("Normal");
        }
        if let Some(size) = self.size {
            out.push(' ');
            out.push_str(&size_string(size));
        }
        f.write_str(&out)
    }
}

/// The size in points with the fewest decimals that `pango_units_from_double`
/// reads back to `size`, as Pango writes it.
fn size_string(size: i32) -> String {
    let points = size as f64 / PANGO_SCALE as f64;
    for decimals in 0..10 {
        let text = format!("{points:.decimals$}");
        let back = text.parse::<f64>().unwrap_or(f64::NAN);
        if libm::floor(back * PANGO_SCALE as f64 + 0.5) as i64 == i64::from(size) {
            return text;
        }
    }
    format!("{points}")
}

// --- the faces ------------------------------------------------------------------

/// A text pack a host gave, or an Emmentaler pack read for text: the face
/// and the OTF it stands for; for Emmentaler, its [`FontPack`](crate::font::FontPack), whose
/// outlines the skylines read. A text pack's outlines come in a pack of
/// their own (D262): until a host gives it, a skyline that needs one notes
/// it in `wants_outlines` (`skylines.rs`), for the core to ask for
/// ([`crate::font::take_wanted`]).
pub struct TextPack {
    pub font: TextFont,
    pub file: String,
    pub music: Option<Value>,
    pub wants_outlines: core::cell::Cell<bool>,
}

impl Foreign for TextPack {
    fn type_name(&self) -> &'static str {
        "Text_pack"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        if let Some(pack) = self.music {
            tracer.trace(pack);
        }
    }
}

/// LilyPond's own text families as fontconfig names them, and the faces
/// of each: regular, bold, italic, bold italic, by their packs' names (the
/// PostScript names). TeX Gyre Schola is LilyPond's too, which fontconfig's
/// metric alias gives "New Century Schoolbook" (OPEN E23, MS8 step 6, D271).
pub(crate) const FAMILIES: [(&[&str], [&str; 4]); 4] = [
    (
        &["lilypond serif", "c059", "serif"],
        ["C059-Roman", "C059-Bold", "C059-Italic", "C059-BdIta"],
    ),
    (
        &["lilypond sans serif", "nimbus sans", "sans", "sans-serif"],
        [
            "NimbusSans-Regular",
            "NimbusSans-Bold",
            "NimbusSans-Italic",
            "NimbusSans-BoldItalic",
        ],
    ),
    (
        &["lilypond monospace", "nimbus mono ps", "monospace", "mono"],
        [
            "NimbusMonoPS-Regular",
            "NimbusMonoPS-Bold",
            "NimbusMonoPS-Italic",
            "NimbusMonoPS-BoldItalic",
        ],
    ),
    (
        &["tex gyre schola", "texgyreschola", "new century schoolbook"],
        [
            "TeXGyreSchola-Regular",
            "TeXGyreSchola-Bold",
            "TeXGyreSchola-Italic",
            "TeXGyreSchola-BoldItalic",
        ],
    ),
];

/// The OTF that fontconfig gives a description of LilyPond's own fonts:
/// its pack's name, its PostScript name, which is also its file's name
/// without `.otf` (TeX Gyre's in lower case). Under the
/// engine's `svg` backend, `paper-defaults-init.ly:170–181` names the text
/// families `serif`, `sans` and `monospace`, which LilyPond's fontconfig
/// gives the machine's fonts; here they are LilyPond's own, C059 and the
/// URW fonts, in which D106's references measure their text.
pub fn face_file(desc: &Description, is_emmentaler: bool) -> String {
    face_files(desc, is_emmentaler).swap_remove(0)
}

/// The faces of `desc` in the order fontconfig falls back through them,
/// of those the packs hold: each of its comma-separated families that is
/// one of [`FAMILIES`], in its style, or C059's if none is. LilyPond's
/// `00-lilypond-fonts.conf` gives each of its families a URW family first
/// and then TeX Gyre's, DejaVu's and Noto's, which no pack holds as a
/// fallback, and the URW faces cover the same characters.
pub fn face_files(desc: &Description, is_emmentaler: bool) -> Vec<String> {
    if is_emmentaler {
        return vec![desc.family.to_ascii_lowercase()];
    }
    let bold = usize::from(desc.weight >= 600);
    let italic = usize::from(desc.style != Style::Normal);
    let mut files: Vec<String> = Vec::new();
    for family in desc.family.split(',') {
        let family = family.trim().to_ascii_lowercase();
        if let Some((_, faces)) = FAMILIES
            .iter()
            .find(|(names, _)| names.contains(&family.as_str()))
            && !files.iter().any(|f| f == faces[bold + 2 * italic])
        {
            files.push(String::from(faces[bold + 2 * italic]));
        }
    }
    if files.is_empty() {
        files.push(String::from(FAMILIES[0].1[bold + 2 * italic]));
    }
    files
}

// --- Pango_font -----------------------------------------------------------------

/// `Pango_font`: a description, its face ([`TextPack`]) and `scale_`, with
/// `physical_font_tab_`, the PostScript names its glyph strings used.
pub struct PangoFont {
    pub description: Description,
    pub face: Value,
    /// The files of the faces after `face` it falls back through
    /// ([`face_files`]), loaded when a string needs them.
    pub fallback: Vec<String>,
    pub scale: f64,
    /// `description_`, `(#f . 1.0)`, as `find_pango_font` sets it.
    pub font_description: Value,
    physical: Vec<(String, String)>,
}

impl Foreign for PangoFont {
    fn type_name(&self) -> &'static str {
        "Pango_font"
    }

    /// `#<Pango_font (#f . 1.0)>`.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Pango_font ");
        heap.write(self.font_description, out);
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.face);
        tracer.trace(self.font_description);
    }
}

impl PangoFont {
    /// `Pango_font (fontmap, description, output_scale)`.
    pub fn new(
        heap: &mut Heap,
        description: Description,
        face: Value,
        fallback: Vec<String>,
        output_scale: f64,
    ) -> PangoFont {
        let scale = INCH_TO_BP / (PANGO_SCALE as f64 * PANGO_RESOLUTION as f64 * output_scale);
        let one = heap.flonum(1.0);
        PangoFont {
            description,
            face,
            fallback,
            scale,
            font_description: heap.cons(Value::FALSE, one),
            physical: Vec::new(),
        }
    }
}

/// `v` as a [`PangoFont`].
pub fn pango_font(heap: &Heap, v: Value) -> Option<&PangoFont> {
    heap.foreign::<PangoFont>(v)
}

/// Pango's rectangle, in Pango units.
#[derive(Clone, Copy, Default)]
struct Rect {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

impl Rect {
    /// `pango_glyph_string_extents_range`'s union of a glyph's ink: an empty
    /// one is left out.
    fn unite(&mut self, r: Rect) {
        if r.width == 0 || r.height == 0 {
            return;
        }
        if self.width == 0 || self.height == 0 {
            *self = r;
            return;
        }
        let (x0, y0) = (self.x.min(r.x), self.y.min(r.y));
        let x1 = (self.x + self.width).max(r.x + r.width);
        let y1 = (self.y + self.height).max(r.y + r.height);
        *self = Rect {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        };
    }

    /// `(-PANGO_DESCENT, PANGO_ASCENT)` times `scale`.
    fn y_extent(self, scale: f64) -> Interval {
        Interval::new(
            (-(self.y + self.height)) as f64 * scale,
            (-self.y) as f64 * scale,
        )
    }
}

/// HarfBuzz's font at Pango's size: `x_mult` for advances and kerning and
/// `x_multf` for extents.
struct Scaled {
    mult: i64,
    multf: f32,
}

impl Scaled {
    fn new(q: i64, units_per_em: u32) -> Scaled {
        let upem = i64::from(units_per_em.max(1));
        Scaled {
            mult: (q << 16) / upem,
            multf: q as f32 / upem as f32,
        }
    }

    /// `em_mult`.
    fn em(&self, v: i64) -> i64 {
        (v * self.mult + 32768) >> 16
    }

    /// `hb_font_get_glyph_extents` as `pango_fc_font_get_glyph_extents`
    /// turns it into an ink rectangle.
    fn ink(&self, [x0, y0, x1, y1]: [i32; 4]) -> Rect {
        if [x0, y0, x1, y1] == [0; 4] {
            return Rect::default();
        }
        let s = |v: i32| v as f32 * self.multf;
        let x_bearing = libm::floorf(s(x0)) as i64;
        let y_bearing = libm::floorf(s(y1)) as i64;
        let width = libm::ceilf(s(x1)) as i64 - x_bearing;
        let height = libm::ceilf(s(y0)) as i64 - y_bearing;
        Rect {
            x: x_bearing,
            y: -y_bearing,
            width,
            height: -height,
        }
    }
}

/// The size's pixels at `PANGO_RESOLUTION`, in Pango units, as fontconfig
/// keeps them: the size HarfBuzz's font is made at.
fn pixels(p: &PangoFont) -> i64 {
    let size = i64::from(p.description.size.unwrap_or(0));
    libm::round((size * PANGO_RESOLUTION) as f64 / 72.0) as i64
}

/// `Pango_font::get_scaled_indexed_char_dimensions`: the ink of glyph
/// `index` of `face`, the font's face, as Pango gives it at the font's
/// size, times `scale_`; a glyph the face has not has none.
pub fn scaled_glyph_box(p: &PangoFont, face: &TextFont, index: usize) -> Box {
    let hb = Scaled::new(pixels(p), face.units_per_em());
    let ink = face
        .glyphs()
        .get(index)
        .map_or(Rect::default(), |g| hb.ink(g.ink));
    Box::new(
        Interval::new(ink.x as f64 * p.scale, (ink.x + ink.width) as f64 * p.scale),
        ink.y_extent(p.scale),
    )
}

/// `PANGO_UNITS_ROUND`: to whole pixels.
fn units_round(d: i64) -> i64 {
    (d + (PANGO_SCALE >> 1)) & !(PANGO_SCALE - 1)
}

/// The features of a `font-features` string, as HarfBuzz reads them
/// (`hb_feature_from_string`): `liga` and `kern` on unless turned off
/// (`-kern`, `kern=0`), any other off unless turned on (`cv47`, `+ss01`).
fn features(text: &str) -> Features {
    let mut f = Features::default();
    for item in text.split(',') {
        let item = item.trim();
        let (on, name) = match item.strip_prefix('-') {
            Some(name) => (false, name),
            None => (true, item.strip_prefix('+').unwrap_or(item)),
        };
        let (name, on) = match name.split_once('=') {
            Some((name, value)) => (name.trim(), on && value.trim() != "0"),
            None => (name, on),
        };
        let name = name.split('[').next().unwrap_or("").trim();
        if !name.is_empty() {
            f.set(name, on);
        }
    }
    f
}

/// Whether `text` needs a face after the first of `faces`: a character of
/// it that selects a font and the first face has not.
pub fn needs_fallback(heap: &Heap, face: Value, text: &str) -> bool {
    let face = &heap.foreign::<TextPack>(face).expect("a face").font;
    text.chars()
        .any(|c| !unicode::keeps_font(c) && face.glyph(c).is_none())
}

/// A run of [`itemize`]'s: its bytes and the position of its face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub start: usize,
    pub end: usize,
    pub face: usize,
}

/// Pango's script runs of `text` (`pango_script_iter_next`): a weak
/// character (Common, Inherited) joins the run it is in, which takes the
/// script of its first strong one; a paired character closing one opened
/// before takes the script its opener was pushed with, which an opener
/// pushed before its run had a script takes when it gets one. The stack of
/// openers lasts from run to run.
fn script_runs(text: &str) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    // The openers: their pair and script, the last on top.
    let mut stack: Vec<(usize, Script)> = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut run = Script::Common;
        // How many of `stack` have their script: those below this run's.
        let mut set = stack.len();
        let mut end = text.len();
        for (i, c) in text[start..].char_indices() {
            let mut script = unicode::script(c);
            let pair = if script == Script::Common {
                unicode::paired(c)
            } else {
                None
            };
            match pair {
                Some((p, true)) => stack.push((p, run)),
                Some((p, false)) if !stack.is_empty() => {
                    while stack.last().is_some_and(|&(q, _)| q != p) {
                        stack.pop();
                    }
                    set = set.min(stack.len());
                    if let Some(&(_, s)) = stack.last() {
                        script = s;
                    }
                }
                _ => {}
            }
            if !(run.is_weak() || script.is_weak() || run == script) {
                end = start + i;
                break;
            }
            if run.is_weak() && !script.is_weak() {
                run = script;
                for open in &mut stack[set..] {
                    open.1 = script;
                }
                set = stack.len();
            }
            if let Some((_, false)) = pair
                && stack.pop().is_some()
            {
                set = set.min(stack.len());
            }
        }
        runs.push((start, end));
        start = end;
    }
    runs
}

/// Pango's items of `text` over the faces `faces` (`itemize_state_process_run`
/// and `_add_character`): a new run at each script run and where the face
/// changes; a character that selects no font ([`unicode::keeps_font`]) in
/// the run's face, or the next character's at its start; else the first
/// face that has it, or the first if none has or `fallback` is off (a
/// music string). Where the face changes to an earlier one, the spaces that
/// end the run before move to the new one, unless they are all it holds.
pub fn itemize(heap: &Heap, faces: &[Value], text: &str, fallback: bool) -> Vec<Run> {
    let fonts: Vec<&TextFont> = faces
        .iter()
        .map(|&f| &heap.foreign::<TextPack>(f).expect("a face").font)
        .collect();
    let mut runs: Vec<Run> = Vec::new();
    for (start, end) in script_runs(text) {
        // The run being made: its start and face, if one is chosen yet.
        let mut at = start;
        let mut face: Option<usize> = None;
        for (i, c) in text[start..end].char_indices() {
            let i = start + i;
            let chosen = if unicode::keeps_font(c) {
                None
            } else if !fallback {
                Some(0)
            } else {
                Some(fonts.iter().position(|f| f.glyph(c).is_some()).unwrap_or(0))
            };
            match (face, chosen) {
                (_, None) => {}
                (None, Some(f)) => face = Some(f),
                (Some(f), Some(g)) if f == g => {}
                (Some(f), Some(g)) => {
                    let mut cut = i;
                    if g < f {
                        let spaces = text[at..i]
                            .char_indices()
                            .rev()
                            .take_while(|&(_, c)| unicode::space_fallback(c).is_some())
                            .last();
                        if let Some((s, _)) = spaces
                            && at + s > at
                        {
                            cut = at + s;
                        }
                    }
                    runs.push(Run {
                        start: at,
                        end: cut,
                        face: f,
                    });
                    at = cut;
                    face = Some(g);
                }
            }
        }
        runs.push(Run {
            start: at,
            end,
            face: face.unwrap_or(0),
        });
    }
    runs
}

/// `Pango_font::text_stencil`: `text` in the font `font`, a `utf-8-string`
/// around its runs' glyph strings, each after the one before, unless it is
/// a music string drawn as paths (`music-strings-to-paths`), and the
/// warnings on the way. `faces` are the font's face and those it falls back
/// through, as far as the string needs them ([`needs_fallback`]).
#[allow(clippy::too_many_arguments)]
pub fn text_stencil(
    heap: &mut Heap,
    font: Value,
    faces: &[Value],
    text: &str,
    music: bool,
    music_to_paths: bool,
    features_str: &str,
    warnings: &mut Vec<String>,
) -> Stencil {
    let features = features(features_str);
    let mut dest = Stencil::default();
    let mut last_x = 0.0;
    for run in itemize(heap, faces, text, !music) {
        let piece = &text[run.start..run.end];
        let item = item_stencil(heap, font, faces[run.face], piece, &features, warnings);
        if let Some(mut item) = item {
            item.translate_axis(heap, last_x, X);
            last_x = item.dim.x().right;
            dest.add_stencil(heap, &item);
        }
    }
    if dest.expr != Value::NULL && (!music || !music_to_paths) {
        let p = pango_font(heap, font).expect("a Pango font");
        let description = p.description.to_string();
        let head = heap.intern("utf-8-string");
        let description = heap.string(description);
        let string = heap.string(text);
        let expr = heap.list(&[head, description, string, dest.expr]);
        dest = Stencil::new(dest.dim, expr);
    }
    dest
}

/// `pango_item_string_stencil` of a run, `text`, in the face `face`:
/// `None` when Pango makes no run (an empty string).
fn item_stencil(
    heap: &mut Heap,
    font: Value,
    face: Value,
    text: &str,
    features: &Features,
    warnings: &mut Vec<String>,
) -> Option<Stencil> {
    if text.is_empty() {
        return None;
    }
    let p = pango_font(heap, font).expect("a Pango font");
    let scale = p.scale;
    let face = heap.foreign::<TextPack>(face).expect("a Pango font's face");
    let shaped = face.font.shape(text, features);
    let ps_name = String::from(face.font.name());
    // LilyPond's fonts/otf names TeX Gyre's files in lower case (E23, D271).
    let file_name = match face.file.starts_with("TeXGyre") {
        true => format!("{FONT_DIR}{}.otf", face.file.to_ascii_lowercase()),
        false => format!("{FONT_DIR}{}.otf", face.file),
    };

    // The size the font describes from its pixels.
    let q = pixels(p);
    let px = q as f64 / PANGO_SCALE as f64;
    let described = ((px * 72.0 / PANGO_RESOLUTION as f64) * PANGO_SCALE as f64) as i64;
    let hb = Scaled::new(q, face.font.units_per_em());

    struct Glyph {
        width: i64,
        ink: Rect,
        index: u16,
        name: String,
        cluster: usize,
    }
    let mut glyphs = Vec::with_capacity(shaped.len());
    let mut lost = false;
    for s in &shaped {
        let Some(index) = s.glyph else {
            let name = unicode::formal_name(s.character).map_or(String::new(), |n| format!(" {n}"));
            warnings.push(format!(
                "no glyph for character '{}' (U+{:04X}{name}) in font `{file_name}'",
                s.character, s.character as u32
            ));
            lost = true;
            continue;
        };
        let mut advance = hb.em(i64::from(s.advance));
        if let Some(space) = s.space {
            advance = face
                .font
                .fallback_advance(space, q, |a| hb.em(i64::from(a)), advance);
        }
        let width = units_round(advance + hb.em(i64::from(s.kerning)));
        let ink = face
            .font
            .glyphs()
            .get(usize::from(index))
            .map_or(Rect::default(), |g| hb.ink(g.ink));
        glyphs.push(Glyph {
            width,
            ink,
            index,
            name: String::from(face.font.glyph_name(index)),
            cluster: s.cluster,
        });
    }

    let mut ink = Rect::default();
    let mut x = 0;
    for g in &glyphs {
        ink.unite(Rect {
            x: x + g.ink.x,
            ..g.ink
        });
        x += g.width;
    }
    let extent = Box::new(Interval::new(0.0, x as f64 * scale), ink.y_extent(scale));
    if glyphs.is_empty() {
        return Some(Stencil::new(extent, Value::NULL));
    }

    let clusters = if lost {
        Value::FALSE
    } else {
        let mut cells = Vec::new();
        let mut at = 0;
        while at < glyphs.len() {
            let start = glyphs[at].cluster;
            let n = glyphs[at..]
                .iter()
                .take_while(|g| g.cluster == start)
                .count();
            let end = glyphs.get(at + n).map_or(text.len(), |g| g.cluster);
            let (bytes, count) = (heap.integer((end - start) as i64), heap.integer(n as i64));
            cells.push(heap.cons(bytes, count));
            at += n;
        }
        heap.list(&cells)
    };
    let exprs: Vec<Value> = glyphs
        .iter()
        .map(|g| {
            let width = heap.flonum(g.width as f64 * scale);
            let y = g.ink.y_extent(scale);
            let (down, up) = (heap.flonum(y.left), heap.flonum(y.right));
            let y = heap.cons(down, up);
            let zero = heap.flonum(0.0);
            let index = heap.integer(i64::from(g.index));
            let name = heap.string(g.name.as_str());
            heap.list(&[width, y, zero, zero, index, name])
        })
        .collect();

    let p = heap.foreign_mut::<PangoFont>(font).expect("a Pango font");
    if !p.physical.iter().any(|(n, _)| *n == ps_name) {
        p.physical.push((ps_name.clone(), file_name.clone()));
    }
    let head = heap.intern("glyph-string");
    let ps = heap.string(ps_name);
    let size = heap.flonum(described as f64 / PANGO_SCALE as f64);
    let glyphs = heap.list(&exprs);
    let file = heap.string(file_name);
    let face_index = heap.integer(0);
    let substr = heap.string(text);
    let expr = heap.list(&[
        head,
        font,
        ps,
        size,
        Value::FALSE,
        glyphs,
        file,
        face_index,
        substr,
        clusters,
    ]);
    Some(Stencil::new(extent, expr))
}

// --- pango-font-scheme.cc ---------------------------------------------------------

/// `(ly:pango-font? f)`.
pub(crate) fn is_pango_font(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(pango_font(cx.heap(), args[0]).is_some()))
}

/// `(ly:pango-font-physical-fonts f)`: `((ps-name file-name font-index) …)`,
/// `()` for anything but a Pango font.
pub(crate) fn physical_fonts(cx: &mut Cx, args: &[Value]) -> R {
    let Some(p) = pango_font(cx.heap(), args[0]) else {
        return Ok(Value::NULL);
    };
    let physical = p.physical.clone();
    let heap = cx.heap_mut();
    let entries: Vec<Value> = physical
        .into_iter()
        .map(|(name, file)| {
            let items = vec![heap.string(name), heap.string(file), heap.integer(0)];
            heap.list(&items)
        })
        .collect();
    Ok(heap.list(&entries))
}
