//! `rod.cc` (part 3's, MS3 step 29): a rod, the minimum distance between
//! two items, and `add_to_cols`, which moves it to their columns and keeps
//! it there (`spaceable.rs`' [`add_rod`](crate::spaceable::add_rod)).
//! `ly:spanner::set-spacing-rods` makes its rods with it (`spanner.rs`,
//! MS3 step 33).
//!
//! LilyPond's clang fuses `distance_ += -d * …` in `add_to_cols` and
//! `w += -d * …` in `bounds_protrusion` (one `fmadd` each in the 2.26.0
//! binary): [`fma`] here, which gives the same sums, `-d` being ±1.

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Value};

use crate::geometry::{Direction, X, fma};
use crate::grob;
use crate::item;
use crate::spaceable;

/// `LEFT` and `RIGHT`.
const SIDES: [Direction; 2] = [-1, 1];

/// A `Rod`: `item_drul_`, the left item first, and `distance_`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rod {
    pub item_drul: [Option<Value>; 2],
    pub distance: f64,
}

impl Rod {
    /// `add_to_cols`: the rod moved from its items to their columns, the
    /// distance grown by how far each item is inside its column, and kept
    /// on the left column if both are columns and not the same one.
    pub fn add_to_cols(&mut self, host: &mut dyn Host) -> Result<(), Exception> {
        if self.item_drul.iter().any(Option::is_none) {
            return Ok(());
        }
        for (i, d) in SIDES.into_iter().enumerate() {
            let Some(it) = self.item_drul[i] else {
                continue;
            };
            let pc = item::get_column(host.heap(), it);
            let x = grob::relative_coordinate(host, it, pc.unwrap_or(Value::NULL), X)?;
            self.distance = fma(f64::from(-d), x, self.distance);
            self.item_drul[i] = pc;
        }
        if let [Some(left), Some(right)] = self.item_drul
            && left != right
        {
            spaceable::add_rod(host, left, right, self.distance)?;
        }
        Ok(())
    }

    /// `bounds_protrusion`: how far the items reach into the rod, each by
    /// its extent's inner side.
    pub fn bounds_protrusion(&self, host: &mut dyn Host) -> Result<f64, Exception> {
        let mut w = 0.0;
        for (i, d) in SIDES.into_iter().enumerate() {
            if let Some(it) = self.item_drul[i] {
                let e = grob::extent(host, it, it, X)?;
                w = fma(f64::from(-d), e.at(-d), w);
            }
        }
        Ok(w)
    }
}
