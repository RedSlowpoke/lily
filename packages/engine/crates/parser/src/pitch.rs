//! Pitch names (02 §2.3, §5.2): the tables of `define-note-names.scm`, never
//! copied into the repository. An interpreter that loaded the file, as the
//! image's has, gives them with [`NoteNames::of_vm`]; else the caller reads
//! the installed LilyPond's file and hands its text to [`NoteNames::read`].
//!
//! The file is Scheme, and only its two forms that build the tables are read:
//! `language-pitch-names`, a quasi-quoted alist of languages, each an alist of
//! names and `(ly:make-pitch -1 step ALTERATION)`; and the `for-each` that
//! appends the aliases (`català` as `catalan`, …). A small reader of data
//! does that without the interpreter, which the parser has no use for until
//! P3.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lily_engine_scheme::{Value, Vm};

/// `declarations-init.ly`'s `default-language`, the table a file starts with.
pub const DEFAULT_LANGUAGE: &str = "nederlands";

/// A pitch as `ly:make-pitch` takes it: an octave (0 is the one from middle
/// C up), a step of the scale (0 is C) and an alteration in quarter tones,
/// as LilyPond's constants (`SHARP` is 1/2, two quarter tones) give them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pitch {
    pub octave: i8,
    pub step: u8,
    pub alteration: i8,
}

impl Pitch {
    /// The pitch `octaves` octaves up (`Pitch::transposed` by octaves).
    pub fn up(self, octaves: i8) -> Pitch {
        Pitch {
            octave: self.octave.saturating_add(octaves),
            ..self
        }
    }

    /// The pitch in a token's value.
    pub(crate) fn pack(self) -> u32 {
        u32::from(self.octave as u8)
            | u32::from(self.step) << 8
            | u32::from(self.alteration as u8) << 16
    }

    pub(crate) fn unpack(value: u32) -> Pitch {
        Pitch {
            octave: value as u8 as i8,
            step: (value >> 8) as u8,
            alteration: (value >> 16) as u8 as i8,
        }
    }
}

/// One language's names, sorted by their bytes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PitchNames {
    /// Each name, its pitch and its place in the file's table.
    names: Vec<(Box<[u8]>, Pitch, u32)>,
}

impl PitchNames {
    /// The pitch named `name`, as `scan_word` looks it up.
    pub fn get(&self, name: &[u8]) -> Option<Pitch> {
        let i = self
            .names
            .binary_search_by(|(n, ..)| (**n).cmp(name))
            .ok()?;
        Some(self.names[i].1)
    }

    /// The first name in the file's table of a pitch with `step` and
    /// `alteration`, in any octave: `note-name->lily-string`'s `rassoc`.
    pub fn name_of(&self, step: u8, alteration: i8) -> Option<&[u8]> {
        self.names
            .iter()
            .filter(|(_, p, _)| p.step == step && p.alteration == alteration)
            .min_by_key(|(.., place)| *place)
            .map(|(name, ..)| &**name)
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    fn insert(&mut self, name: &[u8], pitch: Pitch) {
        // A name given twice keeps its first pitch, as an alist lookup does;
        // no table of the file repeats a name.
        if let Err(i) = self.names.binary_search_by(|(n, ..)| (**n).cmp(name)) {
            let place = self.names.len() as u32;
            self.names.insert(i, (name.into(), pitch, place));
        }
    }
}

/// Every language of `define-note-names.scm`, aliases included, in the
/// file's order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoteNames {
    /// Shared by its clones: a template's parser takes its parser's
    /// (`embedded.rs`).
    languages: Arc<Vec<(String, PitchNames)>>,
}

impl NoteNames {
    /// The tables of `define-note-names.scm`'s `text`.
    pub fn read(text: &[u8]) -> Result<NoteNames, String> {
        let mut reader = Reader { text, pos: 0 };
        let mut names = NoteNames::default();
        while let Some(form) = reader.datum()? {
            let Datum::List(items) = form else { continue };
            match items.as_slice() {
                [
                    Datum::Symbol(b"define-session-public"),
                    Datum::Symbol(b"language-pitch-names"),
                    Datum::Prefix(b'`', alist),
                ] => names.languages(alist)?,
                [Datum::Symbol(b"for-each"), .., Datum::Prefix(b'\'', pairs)] => {
                    names.aliases(pairs)?
                }
                _ => {}
            }
        }
        if names.languages.is_empty() {
            return Err("no language-pitch-names".into());
        }
        Ok(names)
    }

    /// The tables of `language-pitch-names` in `vm`'s `(lily)`, which
    /// `define-note-names.scm` made, its aliases appended. The variable is
    /// read from its slot, as `(@@ (lily) language-pitch-names)` would find
    /// it, with no Scheme evaluated: the engine's start runs neither the
    /// reader nor the compiler (MS2 step 31, D90).
    pub fn of_vm(vm: &mut Vm) -> Result<NoteNames, String> {
        let alist = vm
            .slot("lily", "language-pitch-names")
            .and_then(|slot| vm.slot_value(slot))
            .ok_or("Unbound variable: language-pitch-names")?;
        let heap = vm.heap();
        let items = |list: Value| {
            let mut at = list;
            core::iter::from_fn(move || {
                let item = heap.car(at)?;
                at = heap.cdr(at)?;
                Some(item)
            })
        };
        let mut names = NoteNames::default();
        for language in items(alist) {
            let (Some(name), Some(pitches)) = (heap.car(language), heap.cdr(language)) else {
                return Err("a language is no pair".into());
            };
            let mut table = PitchNames::default();
            for entry in items(pitches) {
                let (Some(pitch_name), Some(pitch)) = (heap.car(entry), heap.cdr(entry)) else {
                    return Err("a pitch name is no pair".into());
                };
                let (Some(pitch_name), Some(pitch)) = (
                    heap.name(pitch_name),
                    heap.foreign::<lily_engine_music::Pitch>(pitch),
                ) else {
                    return Err("a pitch name is no (symbol . pitch)".into());
                };
                let quarter_tones = pitch
                    .alteration
                    .checked_mul(lily_engine_music::Rational::new(4, 1))
                    .filter(|q| q.denominator() == 1)
                    .and_then(|q| i8::try_from(q.numerator()).ok())
                    .ok_or("an alteration is no number of quarter tones")?;
                let pitch = Pitch {
                    octave: i8::try_from(pitch.octave).map_err(|_| "an octave out of range")?,
                    step: u8::try_from(pitch.notename)
                        .ok()
                        .filter(|s| *s < 7)
                        .ok_or("a step out of range")?,
                    alteration: quarter_tones,
                };
                table.insert(pitch_name.as_bytes(), pitch);
            }
            let name = heap.name(name).ok_or("a language's name is no symbol")?;
            Arc::make_mut(&mut names.languages).push((name.into(), table));
        }
        if names.languages.is_empty() {
            return Err("no language-pitch-names".into());
        }
        Ok(names)
    }

    /// The table of `language`, which `assoc` finds first.
    pub fn language(&self, language: &str) -> Option<&PitchNames> {
        self.languages
            .iter()
            .find(|(name, _)| name == language)
            .map(|(_, table)| table)
    }

    /// The name of the language whose table `table` is, which
    /// [`NoteNames::language`] gave.
    pub fn language_of(&self, table: &PitchNames) -> Option<&str> {
        self.languages
            .iter()
            .find(|(_, t)| core::ptr::eq(t, table))
            .map(|(name, _)| name.as_str())
    }

    /// The languages' names, in the file's order.
    pub fn languages_names(&self) -> impl Iterator<Item = &str> {
        self.languages.iter().map(|(name, _)| name.as_str())
    }

    fn languages(&mut self, alist: &Datum) -> Result<(), String> {
        for language in list(alist)? {
            let (name, pitches) = pair(language)?;
            let mut table = PitchNames::default();
            for entry in list(pitches)? {
                let (pitch_name, value) = pair(entry)?;
                table.insert(symbol(pitch_name)?, make_pitch(value)?);
            }
            let name = String::from_utf8_lossy(symbol(name)?).into_owned();
            Arc::make_mut(&mut self.languages).push((name, table));
        }
        Ok(())
    }

    /// `(for-each (lambda (pair) …append (cadr pair) with (car pair)'s table…) '((a b) …))`.
    fn aliases(&mut self, pairs: &Datum) -> Result<(), String> {
        for pair in list(pairs)? {
            let [from, to] = list(pair)? else {
                return Err("an alias is no (name alias)".into());
            };
            let from = String::from_utf8_lossy(symbol(from)?).into_owned();
            let table = self
                .language(&from)
                .ok_or_else(|| alloc::format!("no language {from} to alias"))?
                .clone();
            let to = String::from_utf8_lossy(symbol(to)?).into_owned();
            Arc::make_mut(&mut self.languages).push((to, table));
        }
        Ok(())
    }
}

/// `,(ly:make-pitch octave step alteration)`, the alteration one of
/// `lily-library.scm`'s constants or a number of quarter tones.
fn make_pitch(value: &Datum) -> Result<Pitch, String> {
    let Datum::Prefix(b',', call) = value else {
        return Err("a pitch is no ,(ly:make-pitch …)".into());
    };
    let [Datum::Symbol(b"ly:make-pitch"), octave, step, alteration] = list(call)? else {
        return Err("a pitch is no (ly:make-pitch octave step alteration)".into());
    };
    let alteration = match symbol(alteration)? {
        b"DOUBLE-FLAT" => -4,
        b"THREE-Q-FLAT" => -3,
        b"FLAT" => -2,
        b"SEMI-FLAT" => -1,
        b"NATURAL" => 0,
        b"SEMI-SHARP" => 1,
        b"SHARP" => 2,
        b"THREE-Q-SHARP" => 3,
        b"DOUBLE-SHARP" => 4,
        number => quarter_tones(number).ok_or("an alteration is no number of quarter tones")?,
    };
    let octave = integer(symbol(octave)?).ok_or("an octave is no integer")?;
    let step = integer(symbol(step)?)
        .filter(|s| (0..7).contains(s))
        .ok_or("a step is no integer from 0 to 6")?;
    Ok(Pitch {
        octave,
        step: step as u8,
        alteration,
    })
}

fn integer(text: &[u8]) -> Option<i8> {
    core::str::from_utf8(text).ok()?.parse().ok()
}

/// `n`, `n/2` or `n/4`, in quarter tones.
fn quarter_tones(text: &[u8]) -> Option<i8> {
    let text = core::str::from_utf8(text).ok()?;
    let (num, den) = text.split_once('/').unwrap_or((text, "1"));
    let (num, den): (i8, i8) = (num.parse().ok()?, den.parse().ok()?);
    (den > 0 && 4 % den == 0).then(|| num.checked_mul(4 / den))?
}

fn list<'d, 't>(datum: &'d Datum<'t>) -> Result<&'d [Datum<'t>], String> {
    match datum {
        Datum::List(items) if !items.iter().any(|d| matches!(d, Datum::Dot)) => Ok(items),
        _ => Err("expected a proper list".into()),
    }
}

/// `(car . cdr)`, the cdr a list or a datum: `(a . (b c))` reads as `(a b c)`
/// in Scheme, which this reader keeps as the pair it was written as.
fn pair<'d, 't>(datum: &'d Datum<'t>) -> Result<(&'d Datum<'t>, &'d Datum<'t>), String> {
    match datum {
        Datum::List(items) => match items.as_slice() {
            [car, Datum::Dot, cdr] => Ok((car, cdr)),
            _ => Err("expected a pair (a . b)".into()),
        },
        _ => Err("expected a pair (a . b)".into()),
    }
}

fn symbol<'t>(datum: &Datum<'t>) -> Result<&'t [u8], String> {
    match datum {
        Datum::Symbol(text) => Ok(text),
        _ => Err("expected a symbol".into()),
    }
}

/// What the reader gives: atoms keep their text (a number is a symbol here),
/// strings are skipped, a dotted list keeps its `.`.
#[derive(Debug)]
enum Datum<'t> {
    Symbol(&'t [u8]),
    String,
    Dot,
    List(Vec<Datum<'t>>),
    /// `'d`, `` `d ``, `,d` or `,@d` (as `,`).
    Prefix(u8, Box<Datum<'t>>),
}

struct Reader<'t> {
    text: &'t [u8],
    pos: usize,
}

impl<'t> Reader<'t> {
    fn skip_atmosphere(&mut self) {
        while let Some(&b) = self.text.get(self.pos) {
            match b {
                b';' => {
                    while self.text.get(self.pos).is_some_and(|&c| c != b'\n') {
                        self.pos += 1;
                    }
                }
                b' ' | b'\t' | b'\n' | b'\r' | b'\x0c' => self.pos += 1,
                _ => return,
            }
        }
    }

    /// The next datum, or None at the end of the text.
    fn datum(&mut self) -> Result<Option<Datum<'t>>, String> {
        self.skip_atmosphere();
        let Some(&b) = self.text.get(self.pos) else {
            return Ok(None);
        };
        self.pos += 1;
        let datum = match b {
            b'(' | b'[' => {
                let mut items = Vec::new();
                loop {
                    self.skip_atmosphere();
                    match self.text.get(self.pos) {
                        None => return Err("end of file inside a list".into()),
                        Some(b')' | b']') => {
                            self.pos += 1;
                            break;
                        }
                        Some(_) => items.push(self.datum()?.ok_or("end of file")?),
                    }
                }
                Datum::List(items)
            }
            b')' | b']' => return Err("unexpected )".into()),
            b'\'' | b'`' | b',' => {
                if b == b',' && self.text.get(self.pos) == Some(&b'@') {
                    self.pos += 1;
                }
                let datum = self.datum()?.ok_or("end of file after a prefix")?;
                Datum::Prefix(b, Box::new(datum))
            }
            b'"' => {
                loop {
                    match self.text.get(self.pos) {
                        None => return Err("end of file inside a string".into()),
                        Some(b'\\') => self.pos += 2,
                        Some(b'"') => break,
                        Some(_) => self.pos += 1,
                    }
                }
                self.pos += 1;
                Datum::String
            }
            _ => {
                let start = self.pos - 1;
                while self
                    .text
                    .get(self.pos)
                    .is_some_and(|c| !b"()[]\"; \t\n\r\x0c".contains(c))
                {
                    self.pos += 1;
                }
                match &self.text[start..self.pos] {
                    b"." => Datum::Dot,
                    text => Datum::Symbol(text),
                }
            }
        };
        Ok(Some(datum))
    }
}
