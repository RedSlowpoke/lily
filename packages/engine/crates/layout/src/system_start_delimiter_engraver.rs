//! `system-start-delimiter-engraver.cc` (MS3 step 18):
//! `System_start_delimiter_engraver`, a class of layout's
//! ([`SYSTEM_START_DELIMITER_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - `process_music` (`:180–194`): at the first timestep the nesting read
//!   from `systemStartDelimiterHierarchy` (`Bracket_nesting_group::from_list`,
//!   `:117–140`: a pair a nested group, `SystemStartBrace`,
//!   `SystemStartBracket`, `SystemStartBar` or `SystemStartSquare` the
//!   group's type, anything else a place for a staff), a delimiter spanner
//!   made for each group, of the group's type or `systemStartDelimiter`
//!   (`create_grobs`, `:80–89`), with no cause, and `currentCommandColumn`
//!   the left bound of each (`set_bound`, `:99–105`). As it must run at the
//!   first timestep whether or not the engraver heard anything, the class
//!   runs always.
//! - Each staff symbol acknowledged (`:209–222`) takes the first free place
//!   of the nesting, depth first, and is added to the `elements` of each
//!   group's delimiter on the way (`add_staff`, `:142–155`); with none free,
//!   a place for it is added to the outer group.
//! - Each delimiter acknowledged (`:224–229`), another engraver's: the
//!   delimiters of this nesting added to its `side-support-elements`
//!   (`add_support`, `:91–97`).
//! - `finalize` (`:196–207`): `currentCommandColumn` the right bound of each
//!   delimiter, and each nested delimiter's `side-support-elements` given
//!   its outer one (`set_nesting_support`, `:107–115`). The delimiters'
//!   ends are not announced, as in LilyPond.
//!
//! Without a command column, where LilyPond's would crash, no bound is set
//! (`ledger_line_engraver::set_bound`); an acknowledgement before the
//! first `process_music`, where LilyPond's would crash, does nothing. A
//! type that is no symbol throws as Guile's `symbol->string` does
//! (`ly_symbol2string`). LilyPond's acknowledger of a staff symbol asserts
//! a `Spanner`, which this does not read: one that is no spanner is added
//! as any.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{Cause, FINALIZE, Host, Kind, PROCESS, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Heap, Tag, Value};

use crate::geometry::Direction;
use crate::grob_array;
use crate::ledger_line_engraver::{command_column, set_bound};
use crate::side_position;
use crate::wrong_type;

type Run<T> = Result<T, Exception>;

pub static SYSTEM_START_DELIMITER_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "System_start_delimiter_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | FINALIZE,
    always: true,
    hears: &[],
    acknowledges: &[
        ("staff-symbol-interface", false),
        ("system-start-delimiter-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(SystemStartDelimiterEngraver::default())),
    reads: &[
        "currentCommandColumn",
        "systemStartDelimiter",
        "systemStartDelimiterHierarchy",
    ],
    writes: &[],
};

/// `nesting_`, the outer group, once made.
#[derive(Clone, Default)]
struct SystemStartDelimiterEngraver {
    nesting: Option<Group>,
}

/// A `Bracket_nesting_node`: a group or a place for a staff.
#[derive(Clone)]
enum Node {
    Group(Group),
    /// `Bracket_nesting_staff`: its staff symbol, once one is added.
    Staff(Option<Value>),
}

/// `Bracket_nesting_group`: `delimiter_` (once made), `symbol_` and
/// `children_`.
#[derive(Clone)]
struct Group {
    delimiter: Option<Value>,
    symbol: Value,
    children: Vec<Node>,
}

/// `ly_symbol2string`'s `scm_symbol_to_string`, which throws for a type
/// that is no symbol.
const SUBR: Option<&str> = Some("symbol->string");

const TYPES: [&str; 4] = [
    "SystemStartBrace",
    "SystemStartBracket",
    "SystemStartBar",
    "SystemStartSquare",
];

impl Group {
    /// A new group's `from_list`.
    fn from_list(heap: &Heap, x: Value) -> Group {
        let mut group = Group {
            delimiter: None,
            symbol: Value::NULL,
            children: Vec::new(),
        };
        let mut s = x;
        while let (Some(entry), Some(rest)) = (heap.car(s), heap.cdr(s)) {
            if entry.tag() == Tag::Pair {
                let node = Group::from_list(heap, entry);
                group.children.push(Node::Group(node));
            } else if entry.tag() == Tag::Symbol
                && heap.name(entry).is_some_and(|n| TYPES.contains(&n))
            {
                group.symbol = entry;
            } else {
                group.children.push(Node::Staff(None));
            }
            s = rest;
        }
        group
    }

    /// `create_grobs`.
    fn create_grobs(&mut self, tx: &mut Tx<'_>, default_type: Value) -> Run<()> {
        let ty = if self.symbol.tag() == Tag::Symbol {
            self.symbol
        } else {
            default_type
        };
        let name = match tx.heap().name(ty) {
            Some(n) if ty.tag() == Tag::Symbol => String::from(n),
            _ => return Err(wrong_type(tx.heap(), SUBR, 1, ty, "symbol")),
        };
        let id = tx.make_defined_grob(&name, Cause::None)?;
        self.delimiter = Some(tx.grob_handle(id));

        for child in &mut self.children {
            if let Node::Group(g) = child {
                g.create_grobs(tx, default_type)?;
            }
        }
        Ok(())
    }

    /// `add_support`.
    fn add_support(&self, host: &mut dyn Host, g: Value) -> Run<()> {
        if let Some(d) = self.delimiter {
            side_position::add_support(host, g, d)?;
        }
        for child in &self.children {
            if let Node::Group(c) = child {
                c.add_support(host, g)?;
            }
        }
        Ok(())
    }

    /// `set_bound`.
    fn set_bound(&self, tx: &mut Tx<'_>, d: Direction, g: Option<Value>) -> Run<()> {
        if let Some(delimiter) = self.delimiter {
            set_bound(tx, delimiter, d, g)?;
        }
        for child in &self.children {
            if let Node::Group(c) = child {
                c.set_bound(tx, d, g)?;
            }
        }
        Ok(())
    }

    /// `set_nesting_support`.
    fn set_nesting_support(&self, host: &mut dyn Host, parent: Option<Value>) -> Run<()> {
        if let (Some(parent), Some(d)) = (parent, self.delimiter) {
            side_position::add_support(host, d, parent)?;
        }
        for child in &self.children {
            if let Node::Group(c) = child {
                c.set_nesting_support(host, self.delimiter)?;
            }
        }
        Ok(())
    }

    /// `add_staff`.
    fn add_staff(&mut self, host: &mut dyn Host, grob: Value) -> Run<bool> {
        for i in 0..self.children.len() {
            let added = match &mut self.children[i] {
                Node::Group(c) => c.add_staff(host, grob)?,
                Node::Staff(staff @ None) => {
                    *staff = Some(grob);
                    true
                }
                Node::Staff(Some(_)) => false,
            };
            if added {
                if let Some(d) = self.delimiter {
                    let sym = host.heap().intern("elements");
                    grob_array::add_grob(host, d, sym, grob)?;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The values the nesting holds.
    /// Its shape, for N8's state digest (D240): what its values do not
    /// show.
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[self.delimiter.is_some()]);
        h.num(self.children.len() as i64);
        for child in &self.children {
            match child {
                Node::Group(c) => c.held(h),
                Node::Staff(s) => h.flags(&[s.is_some()]),
            }
        }
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.delimiter);
        out.push(self.symbol);
        for child in &self.children {
            match child {
                Node::Group(c) => c.values(out),
                Node::Staff(s) => out.extend(*s),
            }
        }
    }
}

impl Work for SystemStartDelimiterEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        match &self.nesting {
            Some(g) => g.held(h),
            None => h.num(-1),
        }
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.nesting.is_some() {
            return Ok(());
        }
        let hierarchy = tx.get("systemStartDelimiterHierarchy");
        let delimiter_name = tx.get("systemStartDelimiter");

        let nesting = Group::from_list(tx.heap(), hierarchy);
        // Kept before its grobs are made, so that what they hold lives
        // across the calls that make them.
        let nesting = self.nesting.insert(nesting);
        nesting.create_grobs(tx, delimiter_name)?;
        let col = command_column(tx);
        nesting.set_bound(tx, -1, col)
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(nesting) = &self.nesting {
            let col = command_column(tx);
            nesting.set_bound(tx, 1, col)?;
            nesting.set_nesting_support(tx.host, None)?;
        }
        self.nesting = None;
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let Some(nesting) = &mut self.nesting else {
            return Ok(());
        };
        let g = tx.grob_handle(grob);
        if which == 1 {
            return nesting.add_support(tx.host, g);
        }
        if !nesting.add_staff(tx.host, g)? {
            nesting.children.push(Node::Staff(None));
            nesting.add_staff(tx.host, g)?;
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        if let Some(nesting) = &self.nesting {
            nesting.values(out);
        }
    }
}
