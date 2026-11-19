//! `key-signature-interface.cc` (part 3's, MS3 step 40): the key
//! signature's callback `ly:key-signature-interface::print`
//! (`key-signature-interface.cc:41–127`, 87 lines), the music crate's
//! placeholder until then (D67). It is Rust under a primitive that prints
//! as LilyPond's subr and calls it over `%host-call` (D127, [`HOSTS`]), as
//! the stem's are.
//!
//! Each alteration's staff positions are `output-lib.scm`'s
//! `key-signature-interface::alteration-positions`, as LilyPond imports
//! it; its glyph the accidental's `Font_metric::find_by_name`
//! (`accidental.rs`); the staff space the referencer's `%staff-space`;
//! `ly_assoc_get` and `ly_assoc` (`assoc`, the key being a pair) Scheme's.
//! The positions' `Slice`s are `int`'s, LilyPond's empty one
//! `(INT_MAX, -INT_MAX)`, its shift wrapping as LilyPond's does.

use alloc::format;
use alloc::string::String;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::accidental::find_by_name;
use crate::font_select;
use crate::geometry::{Direction, X};
use crate::grob;
use crate::spacing_options::double_or;
use crate::stem::to_int;
use crate::stencil::Stencil;

const LEFT: Direction = -1;

/// `Slice`: an interval of `int`s, empty when its left end is past its
/// right.
#[derive(Clone, Copy)]
struct Slice {
    left: i32,
    right: i32,
}

impl Slice {
    /// `Slice ()` and `set_empty`: `(INT_MAX, -INT_MAX)`.
    const EMPTY: Slice = Slice {
        left: i32::MAX,
        right: -i32::MAX,
    };

    fn add_point(&mut self, p: i32) {
        self.left = self.left.min(p);
        self.right = self.right.max(p);
    }

    fn is_empty(self) -> bool {
        self.left > self.right
    }

    /// `length`: 0 if empty.
    fn length(self) -> i32 {
        if self.is_empty() {
            0
        } else {
            self.right.wrapping_sub(self.left)
        }
    }

    /// `intersection`.
    fn intersection(self, other: Slice) -> Slice {
        Slice {
            left: self.left.max(other.left),
            right: self.right.min(other.right),
        }
    }

    /// `+ d`: both ends shifted, wrapping as LilyPond's `int`s do.
    fn shifted(self, d: i32) -> Slice {
        Slice {
            left: self.left.wrapping_add(d),
            right: self.right.wrapping_add(d),
        }
    }
}

/// The stencil `s`, its expression rooted until the caller unroots it.
fn kept(host: &mut dyn Host, s: Stencil) -> Stencil {
    host.heap().root(s.expr);
    s
}

/// `print`: from right to left, each alteration of the key signature's
/// `alteration-alist` (0 for a cancellation) as its glyph of
/// `alteration-glyph-name-alist` at each of its staff positions, a
/// warning for an alteration without a glyph or a glyph not in the font;
/// each column `padding` left of the last, or its `padding-pairs` entry
/// with the last glyph, a natural's 0.3 more where its corners overlap
/// the last column's and 0.15 where they touch; aligned left.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let inter = crate::staff_symbol_referencer::staff_space(host, me)? / 2.0;

    let mut mol = Stencil::default();

    let c0s = grob::property(host, me, "c0-position")?;
    host.heap().root(c0s);

    let is_cancellation = grob::has_interface(host.heap(), me, "key-cancellation-interface");

    // SCM lists are stacks, so we work from right to left, ending with
    // the cancellation signature.

    let mut last_ht_left = Slice::EMPTY; // ht intervals for natural glyph kerning
    let mut last_glyph_name = Value::FALSE;
    let padding_pairs = grob::property(host, me, "padding-pairs")?;
    host.heap().root(padding_pairs);

    let fm = font_select::get_default_font(host, me)?;
    host.heap().root(fm);
    let alist = grob::property(host, me, "alteration-glyph-name-alist")?;
    host.heap().root(alist);
    let mut s = grob::property(host, me, "alteration-alist")?;
    host.heap().root(s);
    while let (Some(entry), Some(rest)) = (host.heap().car(s), host.heap().cdr(s)) {
        let alt = if is_cancellation {
            Value::fixnum(0).unwrap()
        } else {
            match host.heap().cdr(entry) {
                Some(alt) => alt,
                None => {
                    return Err(crate::wrong_type(
                        host.heap(),
                        Some("cdr"),
                        1,
                        entry,
                        "pair",
                    ));
                }
            }
        };

        let glyph_name_scm = crate::call_lily(host, "ly:assoc-get", &[alt, alist, Value::FALSE])?;
        let Some(glyph_name) = host
            .heap()
            .str(glyph_name_scm)
            .map(|s| String::from(s.as_str()))
        else {
            let alt = rational_string(host, alt)?;
            grob::warning(host, me, &format!("No glyph found for alteration: {alt}"))?;
            s = rest;
            continue;
        };
        host.heap().root(glyph_name_scm);

        let acc = find_by_name(host, fm, &glyph_name)?;

        if acc.is_empty() {
            grob::warning(host, me, "alteration not found")?;
        } else {
            let mut ht_right = Slice::EMPTY;
            let mut column = Stencil::default();
            let pos_list = crate::call_lily(
                host,
                "key-signature-interface::alteration-positions",
                &[entry, c0s, me],
            )?;
            host.heap().root(pos_list);
            let mut pos_list = pos_list;
            while let (Some(p), Some(next)) = (host.heap().car(pos_list), host.heap().cdr(pos_list))
            {
                let p = to_int(host.heap(), p)?;
                ht_right.add_point(p.wrapping_mul(2).wrapping_sub(6)); // descender
                ht_right.add_point(p.wrapping_mul(2).wrapping_add(3)); // upper right corner
                let mut placed = acc;
                placed.translate(host.heap(), (0.0, f64::from(p) * inter));
                column.add_stencil(host.heap(), &placed);
                pos_list = next;
            }
            let column = kept(host, column);
            // The natural sign (unlike flat & sharp) has vertical edges
            // on both sides. A little padding is needed to prevent
            // collisions.
            let padding = grob::property(host, me, "padding")?;
            let mut padding = double_or(host, padding, 0.0);
            let key = host.heap().cons(glyph_name_scm, last_glyph_name);
            let handle = crate::call_lily(host, "assoc", &[key, padding_pairs])?;
            if let Some(d) = host.heap().cdr(handle) {
                padding = double_or(host, d, 0.0);
            } else if glyph_name == "accidentals.natural" {
                let overlap = ht_right.intersection(last_ht_left);
                if !overlap.is_empty() {
                    padding += if overlap.length() != 0 {
                        0.3 // edges overlap
                    } else {
                        0.15 // just touching at the corners
                    };
                }
            }

            mol.add_at_edge(host.heap(), X, LEFT, &column, padding);
            mol = kept(host, mol);

            last_ht_left = ht_right.shifted(3); // shift up (change to left side)
            last_glyph_name = glyph_name_scm;
        }
        s = rest;
    }

    mol.align_to(host.heap(), X, f64::from(LEFT));

    Ok(host.heap().wrap(mol))
}

/// `from_scm<Rational> (alt).to_string ()`: the numerator, then `/` and
/// the denominator if it is not 1 and the numerator not 0.
fn rational_string(host: &mut dyn Host, alt: Value) -> Result<String, Exception> {
    let num = crate::call_lily(host, "numerator", &[alt])?;
    let den = crate::call_lily(host, "denominator", &[alt])?;
    let (num, den) = (to_int(host.heap(), num)?, to_int(host.heap(), den)?);
    Ok(if den != 1 && num != 0 {
        format!("{num}/{den}")
    } else {
        format!("{num}")
    })
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for `ly:key-signature-interface::print` (D127):
/// the callback its second argument names, of the grob given, with the VM
/// as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "key-signature",
    f: host_key_signature,
}];

fn host_key_signature(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    match vm.heap().name(args[1]) {
        Some("print") => print(vm, args[2]),
        _ => Err(crate::misc_error(
            vm.heap(),
            "%host-call",
            "no such key signature callback",
        )),
    }
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:key-signature-interface::print grob)`: LilyPond's subr, a
/// primitive here so that it prints as LilyPond's. It checks its grob and
/// tail-calls `%host-call` with its name ([`HOSTS`]).
pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    grob::grob_arg(cx, "ly:key-signature-interface::print", 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let key_signature = cx.heap_mut().intern("key-signature");
    let print = cx.heap_mut().intern("print");
    Ok(cx.tail_call(f, &[key_signature, print, args[0]]))
}
