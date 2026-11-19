//! `pure-from-neighbor-engraver.cc` (MS3 step 21): `Pure_from_neighbor_engraver`,
//! a class of layout's ([`PURE_FROM_NEIGHBOR_ENGRAVER`]) that the core
//! passes at its `\layout` call and that a group finds before the music
//! crate's placeholder of the name (D67, D108).
//!
//! - Each item acknowledged (`acknowledge_item`, `:51–59`, an
//!   `item-interface` here): one with `pure-from-neighbor-interface` kept
//!   as needing its neighbors, any other as relevant. A spanner is not
//!   acknowledged.
//! - `finalize` (`:72–141`): nothing if none needs its neighbors
//!   (`:75–76`); both lists sorted by their items' left column ranks
//!   (`Grob::less`, grob.cc `:574–578`) with libc++'s `std::sort`
//!   ([`lily_engine_skyline::sort`], MS3 step 60, OPEN A14), equal ranks
//!   in its order, as LilyPond's are; the needing ones clumped by equal
//!   left rank (`:87–103`). Then each relevant (`:110–137`) is added to
//!   the `neighbors` of each item of the last clump left of it and of the
//!   first not left of it (`Pointer_group_interface::add_grob`), except of
//!   one in its column (`in_same_column`, `:61–70`).
//!
//! The ranks are those `Paper_column_engraver` gave the columns as it made
//! them (`item::spanned_column_rank_interval`); an item without a column
//! has `(VPOS . 0)`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{FINALIZE, Kind, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::grob_array;
use crate::item;

type Run<T> = Result<T, Exception>;

/// `VPOS` as a position in the clumps: none.
const VPOS: usize = usize::MAX;

pub static PURE_FROM_NEIGHBOR_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Pure_from_neighbor_engraver",
    kind: Kind::Engraver,
    methods: FINALIZE,
    always: false,
    hears: &[],
    acknowledges: &[("item-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(PureFromNeighborEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `pure_relevants_` and `need_pure_heights_from_neighbors_`, the items by
/// their handles.
#[derive(Clone, Default)]
struct PureFromNeighborEngraver {
    pure_relevants: Vec<Value>,
    need_pure_heights_from_neighbors: Vec<Value>,
}

/// `in_same_column`: `g1` and `g2` span the same columns, and `g1` one.
fn in_same_column(g1: (u64, u64), g2: (u64, u64)) -> bool {
    g1.0 == g2.0 && g1.1 == g2.1 && g1.0 == g1.1
}

/// Each item of `items` with its spanned column rank interval, sorted by
/// the left rank (`std::sort` with `Grob::less`).
fn sorted(tx: &mut Tx<'_>, items: &[Value]) -> Vec<(Value, (u64, u64))> {
    let mut v: Vec<_> = items
        .iter()
        .map(|&g| (g, item::spanned_column_rank_interval(tx.heap(), g)))
        .collect();
    lily_engine_skyline::sort(
        &mut v,
        &|a: &(Value, (u64, u64)), b: &(Value, (u64, u64))| a.1.0 < b.1.0,
    );
    v
}

impl Work for PureFromNeighborEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let item = tx.grob_handle(grob);
        if grob::has_interface(tx.heap(), item, "pure-from-neighbor-interface") {
            self.need_pure_heights_from_neighbors.push(item);
        } else {
            self.pure_relevants.push(item);
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.need_pure_heights_from_neighbors.is_empty() {
            return Ok(());
        }

        let need = sorted(tx, &self.need_pure_heights_from_neighbors);
        let pure_relevants = sorted(tx, &self.pure_relevants);

        // first, clump need_pure_heights_from_neighbors into vectors of
        // grobs that have the same column.
        let mut need_pure_heights_from_neighbors: Vec<Vec<(Value, (u64, u64))>> = Vec::new();
        for &g in &need {
            match need_pure_heights_from_neighbors.last_mut() {
                Some(clump) if clump[0].1.0 == g.1.0 => clump.push(g),
                _ => need_pure_heights_from_neighbors.push(alloc::vec![g]),
            }
        }

        // then, loop through the pure_relevants_ list, adding the items to
        // the elements of need_pure_heights_from_neighbors_ on either side.
        let neighbors = tx.heap().intern("neighbors");
        let n = need_pure_heights_from_neighbors.len();
        let mut pos = [VPOS, 0];
        for &(pure_relevant, rank) in &pure_relevants {
            while pos[1] < n && rank.0 > need_pure_heights_from_neighbors[pos[1]][0].1.0 {
                pos[0] = pos[1];
                pos[1] += 1;
            }

            // `p != VPOS` too: VPOS is never below n.
            for p in pos {
                if p < n {
                    for &(g, g_rank) in &need_pure_heights_from_neighbors[p] {
                        if !in_same_column(g_rank, rank) {
                            grob_array::add_grob(tx.host, g, neighbors, pure_relevant)?;
                        }
                    }
                }
            }
        }

        self.need_pure_heights_from_neighbors.clear();
        self.pure_relevants.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.pure_relevants.iter().copied());
        out.extend(self.need_pure_heights_from_neighbors.iter().copied());
    }
}
