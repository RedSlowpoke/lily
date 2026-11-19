//! `skyline.cc`'s constructors over the public `Skyline`: from boxes, from
//! one box and from skyline pairs, each against the skyline its buildings
//! must make, and the boxes' against W2's `from_segments` of their sides,
//! which builds the same skyline. Then its operations (raise, shift, height,
//! max_height, left, right, set_minimum_height, clear, padded, distance and
//! the debugging prints) and `skyline-pair.cc`'s calls on `SkylinePair`.
//! Touching points and max height positions are the probe's. Last, the calls of
//! tools/bench/probe-skylines.scm made on the port, which
//! tools/bench/test/skylines.test.mjs compares with LilyPond's.
use std::collections::HashMap;

use lily_engine_fonts::{Box, Interval};
use lily_engine_skyline::{Axis, Direction, Offset, Skyline, SkylinePair};

const INF: f64 = f64::INFINITY;

fn rect(x: [f64; 2], y: [f64; 2]) -> Box {
    Box {
        x: Interval::new(x[0], x[1]),
        y: Interval::new(y[0], y[1]),
    }
}

/// The side a skyline bounds as a segment: the top for `Up`, the bottom for
/// `Down`, along the horizon axis.
fn side(b: Box, horizon: Axis, sky: Direction) -> [Offset; 2] {
    let y = match sky {
        Direction::Down => b.y.left,
        Direction::Up => b.y.right,
    };
    let x = match sky {
        Direction::Down => b.x.left,
        Direction::Up => b.x.right,
    };
    match horizon {
        Axis::X => [[b.x.left, y], [b.x.right, y]],
        Axis::Y => [[x, b.y.left], [x, b.y.right]],
    }
}

const SIDES: [(Axis, Direction); 4] = [
    (Axis::X, Direction::Down),
    (Axis::X, Direction::Up),
    (Axis::Y, Direction::Down),
    (Axis::Y, Direction::Up),
];

#[test]
fn boxes_make_the_higher_roof() {
    let boxes = [
        rect([0.0, 4.0], [0.0, 1.0]),
        rect([1.0, 2.0], [-3.0, 2.0]),
        // Empty on Y: left out.
        rect([5.0, 6.0], [1.0, 0.0]),
    ];
    let up = Skyline::from_boxes(&boxes, Axis::X, Direction::Up);
    assert_eq!(
        up.to_points(Axis::X),
        [
            [-INF, -INF],
            [0.0, -INF],
            [0.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [2.0, 2.0],
            [2.0, 1.0],
            [4.0, 1.0],
            [4.0, -INF],
            [INF, -INF]
        ]
    );
    let down = Skyline::from_boxes(&boxes, Axis::X, Direction::Down);
    assert_eq!(down.sky(), Direction::Down);
    assert_eq!(
        down.to_points(Axis::X),
        [
            [-INF, INF],
            [0.0, INF],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, -3.0],
            [2.0, -3.0],
            [2.0, 0.0],
            [4.0, 0.0],
            [4.0, INF],
            [INF, INF]
        ]
    );
}

/// `Skyline (boxes, Y_AXIS, LEFT)`, as separation items and accidentals
/// build theirs: the horizon is Y and the points come out swapped.
#[test]
fn boxes_bound_the_left_along_y() {
    let boxes = [
        rect([0.5, 1.0], [-1.0, 1.0]),
        rect([-0.25, 2.0], [0.0, 0.5]),
    ];
    let left = Skyline::from_boxes(&boxes, Axis::Y, Direction::Down);
    assert_eq!(
        left.to_points(Axis::Y),
        [
            [INF, -INF],
            [INF, -1.0],
            [0.5, -1.0],
            [0.5, 0.0],
            [-0.25, 0.0],
            [-0.25, 0.5],
            [0.5, 0.5],
            [0.5, 1.0],
            [INF, 1.0],
            [INF, INF]
        ]
    );
}

/// W2's `from_segments` of the boxes' sides builds the same buildings, for
/// boxes with length along the horizon.
#[test]
fn boxes_build_their_sides_skyline() {
    let boxes = [
        rect([0.0, 4.0], [0.0, 1.0]),
        rect([1.0, 2.0], [-3.0, 2.0]),
        rect([3.5, 7.25], [-0.75, 0.125]),
        rect([-2.0, 0.5], [0.5, 1.5]),
    ];
    for (horizon, sky) in SIDES {
        let sides: Vec<[Offset; 2]> = boxes.iter().map(|&b| side(b, horizon, sky)).collect();
        assert_eq!(
            Skyline::from_boxes(&boxes, horizon, sky),
            Skyline::from_segments(&sides, horizon, sky),
            "{horizon:?} {sky:?}"
        );
        for &b in &boxes {
            assert_eq!(
                Skyline::from_box(b, horizon, sky),
                Skyline::from_segments(&[side(b, horizon, sky)], horizon, sky),
                "{b:?} {horizon:?} {sky:?}"
            );
        }
    }
}

/// A box without length along the horizon is not empty, so it makes a
/// building of no width (skyline.cc accepts them since 2012), where a
/// segment without length makes none.
#[test]
fn a_box_without_width_is_a_building() {
    let flat = rect([-2.0, 0.0], [0.5, 0.5]);
    let tall = rect([0.0, 1.0], [-1.0, 1.0]);
    let left = Skyline::from_boxes(&[tall, flat], Axis::Y, Direction::Down);
    assert_eq!(
        left.to_points(Axis::Y),
        [
            [INF, -INF],
            [INF, -1.0],
            [0.0, -1.0],
            [0.0, 0.5],
            [-2.0, 0.5],
            [-2.0, 0.5],
            [0.0, 0.5],
            [0.0, 1.0],
            [INF, 1.0],
            [INF, INF]
        ]
    );
    let sides = [
        side(tall, Axis::Y, Direction::Down),
        side(flat, Axis::Y, Direction::Down),
    ];
    assert_eq!(
        Skyline::from_segments(&sides, Axis::Y, Direction::Down),
        Skyline::from_box(tall, Axis::Y, Direction::Down)
    );
    let alone = Skyline::from_box(flat, Axis::Y, Direction::Down);
    assert!(!alone.is_empty());
    assert_eq!(
        alone.to_points(Axis::Y),
        [
            [INF, -INF],
            [INF, 0.5],
            [-2.0, 0.5],
            [-2.0, 0.5],
            [INF, 0.5],
            [INF, INF]
        ]
    );
}

#[test]
fn one_box_is_one_building() {
    let b = rect([1.0, 3.0], [-2.0, 0.5]);
    for (horizon, sky) in SIDES {
        assert_eq!(
            Skyline::from_box(b, horizon, sky),
            Skyline::from_boxes(&[b], horizon, sky)
        );
    }
    assert_eq!(
        Skyline::from_box(b, Axis::X, Direction::Up).to_points(Axis::X),
        [
            [-INF, -INF],
            [1.0, -INF],
            [1.0, 0.5],
            [3.0, 0.5],
            [3.0, -INF],
            [INF, -INF]
        ]
    );
    let endless = Skyline::from_box(rect([-INF, INF], [0.0, 1.0]), Axis::X, Direction::Up);
    assert!(!endless.is_empty());
    assert_eq!(endless.to_points(Axis::X), [[-INF, 1.0], [INF, 1.0]]);
}

/// No boxes, or only empty ones, make the empty skyline; one empty box makes
/// a skyline without buildings, which merges as an empty one.
#[test]
fn empty_boxes_make_empty_skylines() {
    let empty = rect([1.0, 0.0], [0.0, 1.0]);
    assert_eq!(
        Skyline::from_boxes(&[], Axis::X, Direction::Up),
        Skyline::empty(Direction::Up)
    );
    assert_eq!(
        Skyline::from_boxes(&[empty, Box::EMPTY], Axis::Y, Direction::Down),
        Skyline::empty(Direction::Down)
    );
    assert_eq!(Skyline::default(), Skyline::empty(Direction::Up));

    let none = Skyline::from_box(empty, Axis::X, Direction::Up);
    assert!(none.is_empty());
    assert!(none.to_points(Axis::X).is_empty());
    let b = Skyline::from_box(rect([0.0, 1.0], [0.0, 1.0]), Axis::X, Direction::Up);
    let mut merged = none.clone();
    merged.merge(&b);
    assert_eq!(merged, b);
    let mut kept = b.clone();
    kept.merge(&none);
    assert_eq!(kept, b);
}

/// `Skyline (std::vector<Skyline_pair>, Direction)`: the pairs' sides merged
/// from a queue, the first two first and their merge after the third.
#[test]
fn pairs_merge_from_a_queue() {
    let pair = |b: Box| SkylinePair {
        down: Skyline::from_box(b, Axis::X, Direction::Down),
        up: Skyline::from_box(b, Axis::X, Direction::Up),
    };
    let boxes = [
        rect([0.0, 2.0], [0.0, 1.0]),
        rect([1.0, 3.0], [-1.0, 2.0]),
        rect([2.5, 4.0], [0.5, 3.0]),
    ];
    let pairs: Vec<SkylinePair> = boxes.iter().map(|&b| pair(b)).collect();
    for sky in [Direction::Down, Direction::Up] {
        let side = |p: &SkylinePair| match sky {
            Direction::Down => p.down.clone(),
            Direction::Up => p.up.clone(),
        };
        let mut first_two = side(&pairs[0]);
        first_two.merge(&side(&pairs[1]));
        let mut expected = side(&pairs[2]);
        expected.merge(&first_two);
        assert_eq!(Skyline::from_pairs(&pairs, sky), expected);
        assert_eq!(Skyline::from_pairs(&pairs[..1], sky), side(&pairs[0]));
    }
    assert_eq!(
        Skyline::from_pairs(&pairs, Direction::Up).to_points(Axis::X),
        [
            [-INF, -INF],
            [0.0, -INF],
            [0.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [2.5, 2.0],
            [2.5, 3.0],
            [4.0, 3.0],
            [4.0, -INF],
            [INF, -INF]
        ]
    );
    let none = Skyline::from_pairs(&[], Direction::Down);
    assert!(none.is_empty());
    assert_eq!(none.sky(), Direction::Down);
    assert!(none.to_points(Axis::X).is_empty());
}

/// The points with a finite height, those a test reads.
fn roofs(sky: &Skyline) -> Vec<Offset> {
    sky.to_points(Axis::X)
        .into_iter()
        .filter(|p| p[1].is_finite())
        .collect()
}

/// A roof from `[0, 1]` up to `[2, 3]`.
fn slope(sky: Direction) -> Skyline {
    Skyline::from_segments(&[[[0.0, 1.0], [2.0, 3.0]]], Axis::X, sky)
}

#[test]
fn raise_moves_both_sides_up() {
    let b = rect([0.0, 2.0], [-1.0, 3.0]);
    let mut up = Skyline::from_box(b, Axis::X, Direction::Up);
    up.raise(1.5);
    assert_eq!(roofs(&up), [[0.0, 4.5], [2.0, 4.5]]);
    let mut down = Skyline::from_box(b, Axis::X, Direction::Down);
    down.raise(1.5);
    assert_eq!(roofs(&down), [[0.0, 0.5], [2.0, 0.5]]);
    let mut sloped = slope(Direction::Up);
    sloped.raise(-1.0);
    assert_eq!(roofs(&sloped), [[0.0, 0.0], [2.0, 2.0]]);
    let mut empty = Skyline::empty(Direction::Up);
    empty.raise(2.0);
    assert!(empty.is_empty());
}

/// A shifted skyline is the one built shifted, its slopes' intercepts
/// moved with it.
#[test]
fn shift_moves_along_the_horizon() {
    for sky in [Direction::Down, Direction::Up] {
        let mut shifted = slope(sky);
        shifted.shift(1.5);
        assert_eq!(
            shifted,
            Skyline::from_segments(&[[[1.5, 1.0], [3.5, 3.0]]], Axis::X, sky)
        );
    }
    let mut b = Skyline::from_box(rect([0.0, 2.0], [0.0, 1.0]), Axis::Y, Direction::Up);
    b.shift(-1.0);
    assert_eq!(
        b,
        Skyline::from_box(rect([0.0, 2.0], [-1.0, 0.0]), Axis::Y, Direction::Up)
    );
}

/// The height at x is the first building's not ending before x, so at a
/// building's start the one before it counts.
#[test]
fn height_is_the_building_s_at_x() {
    let up = slope(Direction::Up);
    assert_eq!(up.height(1.0), 2.0);
    assert_eq!(up.height(0.5), 1.5);
    assert_eq!(up.height(2.0), 3.0);
    assert_eq!(up.height(0.0), -INF);
    assert_eq!(up.height(-5.0), -INF);
    assert_eq!(up.height(2.5), -INF);
    let down = Skyline::from_box(rect([0.0, 2.0], [-1.0, 3.0]), Axis::X, Direction::Down);
    assert_eq!(down.height(1.0), -1.0);
    assert_eq!(down.height(3.0), INF);
    assert_eq!(Skyline::empty(Direction::Down).height(0.0), INF);
}

#[test]
#[should_panic(expected = "infinity")]
fn height_at_infinity_panics() {
    slope(Direction::Up).height(INF);
}

/// The highest point, which a `Down` skyline has lowest.
#[test]
fn max_height_is_the_highest_end() {
    assert_eq!(slope(Direction::Up).max_height(), 3.0);
    assert_eq!(slope(Direction::Down).max_height(), 1.0);
    let b = rect([0.0, 2.0], [-1.0, 3.0]);
    assert_eq!(
        Skyline::from_box(b, Axis::Y, Direction::Up).max_height(),
        2.0
    );
    assert_eq!(
        Skyline::from_box(b, Axis::Y, Direction::Down).max_height(),
        0.0
    );
    assert_eq!(Skyline::empty(Direction::Up).max_height(), -INF);
    assert_eq!(Skyline::empty(Direction::Down).max_height(), INF);
    let none = Skyline::from_box(Box::EMPTY, Axis::X, Direction::Down);
    assert_eq!(none.max_height(), INF);
}

#[test]
fn right_is_where_the_last_roof_ends() {
    let boxes = [rect([0.0, 2.0], [0.0, 1.0]), rect([3.0, 5.0], [0.0, 1.0])];
    assert_eq!(
        Skyline::from_boxes(&boxes, Axis::X, Direction::Up).right(),
        5.0
    );
    assert_eq!(
        Skyline::from_boxes(&boxes, Axis::Y, Direction::Down).right(),
        1.0
    );
    assert_eq!(Skyline::empty(Direction::Up).right(), -INF);
    let none = Skyline::from_box(Box::EMPTY, Axis::X, Direction::Up);
    assert_eq!(none.right(), -INF);
    let endless = Skyline::from_box(rect([-INF, INF], [0.0, 1.0]), Axis::X, Direction::Up);
    assert_eq!(endless.right(), INF);
}

#[test]
fn left_is_where_the_first_roof_starts() {
    let boxes = [rect([0.0, 2.0], [0.0, 1.0]), rect([3.0, 5.0], [0.0, 1.0])];
    assert_eq!(
        Skyline::from_boxes(&boxes, Axis::X, Direction::Up).left(),
        0.0
    );
    assert_eq!(
        Skyline::from_boxes(&boxes, Axis::Y, Direction::Down).left(),
        0.0
    );
    assert_eq!(Skyline::empty(Direction::Up).left(), INF);
    let none = Skyline::from_box(Box::EMPTY, Axis::X, Direction::Up);
    assert_eq!(none.left(), INF);
    let endless = Skyline::from_box(rect([-INF, INF], [0.0, 1.0]), Axis::X, Direction::Up);
    assert_eq!(endless.left(), -INF);
}

/// `printf`'s `%f` of each building, and of the points two to a line.
#[test]
fn print_writes_the_buildings_and_points() {
    let sky = Skyline::from_box(rect([0.0, 2.0], [-1.0, 0.5]), Axis::X, Direction::Down);
    assert_eq!(
        sky.print(),
        "0.000000 x + -inf from -inf to 0.000000\n\
         0.000000 x + 1.000000 from 0.000000 to 2.000000\n\
         0.000000 x + -inf from 2.000000 to inf\n"
    );
    assert_eq!(
        sky.print_points(),
        "(-inf,inf) (0.000000,inf)\n\
         (0.000000,-1.000000) (2.000000,-1.000000)\n\
         (2.000000,inf) (inf,inf)\n"
    );
    let pair = SkylinePair::new(sky.clone(), Skyline::empty(Direction::Up));
    assert_eq!(
        pair.print(),
        "0.000000 x + -inf from -inf to inf\n".to_string() + &sky.print()
    );
    assert_eq!(
        pair.print_points(),
        "(-inf,-inf) (inf,-inf)\n".to_string() + &sky.print_points()
    );
}

/// A roof at the height everywhere, under the skyline's own: below the roofs
/// of `Up`, above those of `Down`.
#[test]
fn set_minimum_height_merges_a_floor() {
    let b = rect([1.0, 3.0], [0.0, 2.0]);
    let mut up = Skyline::from_box(b, Axis::X, Direction::Up);
    up.set_minimum_height(0.5);
    assert_eq!(
        up.to_points(Axis::X),
        [
            [-INF, 0.5],
            [1.0, 0.5],
            [1.0, 2.0],
            [3.0, 2.0],
            [3.0, 0.5],
            [INF, 0.5]
        ]
    );
    let mut down = Skyline::from_box(b, Axis::X, Direction::Down);
    down.set_minimum_height(0.5);
    assert_eq!(
        down.to_points(Axis::X),
        [
            [-INF, 0.5],
            [1.0, 0.5],
            [1.0, 0.0],
            [3.0, 0.0],
            [3.0, 0.5],
            [INF, 0.5]
        ]
    );
    let mut floored = Skyline::from_box(b, Axis::X, Direction::Down);
    floored.set_minimum_height(-0.5);
    assert_eq!(floored.to_points(Axis::X), [[-INF, -0.5], [INF, -0.5]]);
    let mut empty = Skyline::empty(Direction::Down);
    empty.set_minimum_height(2.0);
    assert_eq!(empty.to_points(Axis::X), [[-INF, 2.0], [INF, 2.0]]);
    assert_eq!(empty.max_height(), 2.0);
    let mut kept = Skyline::from_box(b, Axis::X, Direction::Up);
    kept.set_minimum_height(-INF);
    assert_eq!(kept, Skyline::from_box(b, Axis::X, Direction::Up));
}

#[test]
fn clear_leaves_the_empty_skyline() {
    let mut down = slope(Direction::Down);
    down.clear();
    assert_eq!(down, Skyline::empty(Direction::Down));
    let mut none = Skyline::from_pairs(&[], Direction::Up);
    none.clear();
    assert_eq!(none, Skyline::empty(Direction::Up));
    assert_eq!(none.to_points(Axis::X).len(), 2);
}

/// Each roof continued flat for the padding, then falling back by the padding
/// over as long again: down for `Up`, up (right) for `Down`.
#[test]
fn padded_continues_the_roofs() {
    let up = Skyline::from_box(rect([0.0, 2.0], [0.0, 1.0]), Axis::X, Direction::Up);
    assert_eq!(
        up.padded(0.5).to_points(Axis::X),
        [
            [-INF, -INF],
            [-1.0, -INF],
            [-1.0, 0.5],
            [-0.5, 1.0],
            [-0.5, 1.0],
            [0.0, 1.0],
            [0.0, 1.0],
            [2.0, 1.0],
            [2.0, 1.0],
            [2.5, 1.0],
            [2.5, 1.0],
            [3.0, 0.5],
            [3.0, -INF],
            [INF, -INF]
        ]
    );
    let down = Skyline::from_box(rect([0.0, 2.0], [0.0, 1.0]), Axis::Y, Direction::Down);
    assert_eq!(
        roofs(&down.padded(0.25)),
        [
            [-0.5, 0.25],
            [-0.25, 0.0],
            [-0.25, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.0],
            [1.25, 0.0],
            [1.25, 0.0],
            [1.5, 0.25]
        ]
    );
    assert_eq!(up.padded(0.0), up);
    assert_eq!(up.padded(-1.0), up);
    assert_eq!(
        Skyline::empty(Direction::Up).padded(1.0),
        Skyline::empty(Direction::Up)
    );
    let none = Skyline::from_box(Box::EMPTY, Axis::X, Direction::Down);
    assert_eq!(none.padded(1.0), Skyline::empty(Direction::Down));
}

/// How far the skyline above must move up to clear the one below: the top of
/// one box at 1 against the bottom of another at 2 is -1 where they overlap
/// along the horizon, -infinity where they do not, unless the padding
/// reaches.
#[test]
fn distance_is_the_largest_overlap() {
    let below = Skyline::from_box(rect([0.0, 2.0], [0.0, 1.0]), Axis::X, Direction::Up);
    let over = Skyline::from_box(rect([1.0, 3.0], [2.0, 4.0]), Axis::X, Direction::Down);
    assert_eq!(below.distance(&over, 0.0), -1.0);
    assert_eq!(over.distance(&below, 0.0), -1.0);
    let beside = Skyline::from_box(rect([3.0, 4.0], [2.0, 4.0]), Axis::X, Direction::Down);
    assert_eq!(below.distance(&beside, 0.0), -INF);
    // The padding's flat part reaches beside's edge at 3 only.
    assert_eq!(below.distance(&beside, 0.5), -INF);
    assert_eq!(below.distance(&beside, 1.0), -1.0);
    assert_eq!(below.distance(&beside, 1.5), -1.0);
    assert_eq!(beside.distance(&below, 1.0), -1.0);
    // Only this skyline is padded.
    assert_eq!(below.distance(&beside.padded(1.0), 0.0), -1.0);
    // Sloped roofs touch where their sum is largest.
    let rising = slope(Direction::Up);
    let mut falling = Skyline::from_segments(&[[[0.0, 5.0], [2.0, 2.0]]], Axis::X, Direction::Down);
    assert_eq!(rising.distance(&falling, 0.0), 1.0);
    falling.raise(1.0);
    assert_eq!(rising.distance(&falling, 0.0), 0.0);
    let none = Skyline::from_box(Box::EMPTY, Axis::X, Direction::Down);
    assert_eq!(below.distance(&none, 0.0), -INF);
    assert_eq!(
        Skyline::empty(Direction::Up).distance(&Skyline::empty(Direction::Down), 0.0),
        -INF
    );
}

#[test]
#[should_panic(expected = "one side")]
fn distance_between_skylines_of_one_side_panics() {
    slope(Direction::Up).distance(&slope(Direction::Up), 0.0);
}

/// `Skyline_pair`'s constructors build both sides with `Skyline`'s.
#[test]
fn pairs_build_both_sides() {
    let boxes = [rect([0.0, 2.0], [0.0, 1.0]), rect([1.0, 3.0], [-1.0, 2.0])];
    let both =
        |f: &dyn Fn(Direction) -> Skyline| SkylinePair::new(f(Direction::Down), f(Direction::Up));
    assert_eq!(SkylinePair::default(), both(&Skyline::empty));
    assert!(SkylinePair::default().is_empty());
    for horizon in [Axis::X, Axis::Y] {
        assert_eq!(
            SkylinePair::from_boxes(&boxes, horizon),
            both(&|sky| Skyline::from_boxes(&boxes, horizon, sky))
        );
        assert_eq!(
            SkylinePair::from_box(boxes[1], horizon),
            both(&|sky| Skyline::from_box(boxes[1], horizon, sky))
        );
        let segments = [[[0.0, 1.0], [2.0, 3.0]], [[1.0, -1.0], [1.0, 4.0]]];
        assert_eq!(
            SkylinePair::from_segments(&segments, horizon),
            both(&|sky| Skyline::from_segments(&segments, horizon, sky))
        );
    }
    let pairs = [
        SkylinePair::from_box(boxes[0], Axis::X),
        SkylinePair::from_box(boxes[1], Axis::X),
    ];
    let merged = SkylinePair::from_pairs(&pairs);
    assert_eq!(merged, both(&|sky| Skyline::from_pairs(&pairs, sky)));
    assert_eq!(merged, SkylinePair::from_boxes(&boxes, Axis::X));
    assert!(!merged.is_empty());
}

/// `operator[]`, and raise, shift and merge on both sides.
#[test]
fn pairs_move_and_merge_both_sides() {
    let b = rect([0.0, 2.0], [-1.0, 1.0]);
    let mut pair = SkylinePair::from_box(b, Axis::X);
    assert_eq!(pair[Direction::Down], pair.down);
    assert_eq!(pair[Direction::Up], pair.up);
    pair.raise(1.0);
    pair.shift(3.0);
    assert_eq!(
        pair,
        SkylinePair::from_box(rect([3.0, 5.0], [0.0, 2.0]), Axis::X)
    );
    pair[Direction::Up].set_minimum_height(2.5);
    assert_eq!(pair.up.max_height(), 2.5);
    assert_eq!(pair.down.max_height(), 0.0);

    let mut pair = SkylinePair::from_box(b, Axis::X);
    let other = SkylinePair::from_box(rect([1.0, 3.0], [-2.0, 0.5]), Axis::X);
    pair.merge(&other);
    assert_eq!(
        pair,
        SkylinePair::from_boxes(&[b, rect([1.0, 3.0], [-2.0, 0.5])], Axis::X)
    );
    let mut empty = SkylinePair::default();
    empty.merge(&other);
    assert_eq!(empty, other);
}

/// The further right of the two sides; the pair is empty only when both are.
#[test]
fn a_pair_s_right_and_emptiness() {
    let pair = SkylinePair::new(
        Skyline::from_box(rect([0.0, 3.0], [0.0, 1.0]), Axis::X, Direction::Down),
        Skyline::from_box(rect([0.0, 5.0], [0.0, 1.0]), Axis::X, Direction::Up),
    );
    assert_eq!(pair.right(), 5.0);
    let mut half = SkylinePair::default();
    assert_eq!(half.right(), -INF);
    half.down = pair.down.clone();
    assert!(!half.is_empty());
    assert_eq!(half.right(), 3.0);
    half.down.clear();
    assert!(half.is_empty());
    assert_eq!(pair.left(), 0.0);
    assert_eq!(SkylinePair::default().left(), INF);
}

/// `Skyline_pair::pad`: each side `padded`, nothing for 0.
#[test]
fn a_pair_pads_both_sides() {
    let b = rect([0.0, 3.0], [0.0, 1.0]);
    let mut pair = SkylinePair::from_box(b, Axis::X);
    let before = pair.clone();
    pair.pad(0.0);
    assert_eq!(pair, before);
    pair.pad(0.5);
    assert_eq!(pair.up, before.up.padded(0.5));
    assert_eq!(pair.down, before.down.padded(0.5));
    assert_eq!(pair.left(), -1.0);
    assert_eq!(pair.right(), 4.0);
}

/// A datum of the probe as its reader gives it: an atom's text (a string's
/// with its quotes) or a list; `'d` is `(quote d)`.
#[derive(Clone, Debug)]
enum Datum {
    Atom(String),
    List(Vec<Datum>),
}

/// The data of `text`, a Scheme file, as far as the probe needs: lists,
/// quotes, strings, atoms and comments.
fn read_data(text: &str) -> Vec<Datum> {
    fn datum(chars: &mut core::iter::Peekable<core::str::Chars>) -> Option<Datum> {
        loop {
            match chars.peek()? {
                c if c.is_whitespace() => {
                    chars.next();
                }
                ';' => while chars.next().is_some_and(|c| c != '\n') {},
                _ => break,
            }
        }
        match chars.next()? {
            '(' => {
                let mut items = Vec::new();
                loop {
                    while chars.peek().is_some_and(|c| c.is_whitespace()) {
                        chars.next();
                    }
                    if chars.peek() == Some(&')') {
                        chars.next();
                        return Some(Datum::List(items));
                    }
                    items.push(datum(chars).expect("a list ends"));
                }
            }
            '\'' => {
                let quoted = datum(chars).expect("a quote quotes a datum");
                Some(Datum::List(vec![Datum::Atom("quote".into()), quoted]))
            }
            '"' => {
                let mut text = String::from('"');
                while let Some(c) = chars.next() {
                    text.push(c);
                    match c {
                        '\\' => text.extend(chars.next()),
                        '"' => break,
                        _ => {}
                    }
                }
                Some(Datum::Atom(text))
            }
            c => {
                let mut text = String::from(c);
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == '(' || c == ')' {
                        break;
                    }
                    text.push(c);
                    chars.next();
                }
                Some(Datum::Atom(text))
            }
        }
    }
    let mut chars = text.chars().peekable();
    core::iter::from_fn(|| datum(&mut chars)).collect()
}

/// A value of a probe's call.
enum Value {
    Number(f64),
    Bool(bool),
    Skyline(Skyline),
    Axis(Axis),
    Sky(Direction),
    Quoted(Datum),
    List(Vec<Value>),
}

/// A number as Scheme reads the probe's: n/d, the infinities, or decimal.
fn number(text: &str) -> Option<f64> {
    match text {
        "+inf.0" => Some(INF),
        "-inf.0" => Some(-INF),
        _ => match text.split_once('/') {
            Some((n, d)) => Some(n.parse::<i64>().ok()? as f64 / d.parse::<i64>().ok()? as f64),
            None => text.parse().ok(),
        },
    }
}

/// Evaluates a call of the probe on the port, the skylines its definitions
/// made in `skylines`.
fn eval(call: &Datum, skylines: &HashMap<String, Skyline>) -> Value {
    let items = match call {
        Datum::Atom(atom) => {
            return match atom.as_str() {
                "X" => Value::Axis(Axis::X),
                "Y" => Value::Axis(Axis::Y),
                "UP" => Value::Sky(Direction::Up),
                "DOWN" => Value::Sky(Direction::Down),
                _ => match number(atom) {
                    Some(n) => Value::Number(n),
                    None => Value::Skyline(
                        skylines
                            .get(atom)
                            .unwrap_or_else(|| panic!("{atom}?"))
                            .clone(),
                    ),
                },
            };
        }
        Datum::List(items) => items,
    };
    let Some((Datum::Atom(head), args)) = items.split_first() else {
        panic!("{call:?}");
    };
    if head == "quote" {
        return Value::Quoted(args[0].clone());
    }
    let args: Vec<Value> = args.iter().map(|arg| eval(arg, skylines)).collect();
    if head == "list" {
        return Value::List(args);
    }
    let n = |v: &Value| match v {
        Value::Number(n) => *n,
        _ => panic!("{head}: a number"),
    };
    let sky = |v: &Value| match v {
        Value::Skyline(s) => s.clone(),
        _ => panic!("{head}: a skyline"),
    };
    let axis = |v: &Value| match v {
        Value::Axis(a) => *a,
        _ => panic!("{head}: an axis"),
    };
    let numbers = |points: Vec<Offset>| {
        Value::List(points.into_iter().flatten().map(Value::Number).collect())
    };
    match (head.as_str(), &args[..]) {
        ("ly:make-skyline", [Value::Quoted(Datum::List(segments)), a, Value::Sky(d)]) => {
            let segments: Vec<[Offset; 2]> = segments
                .iter()
                .map(|segment| {
                    let Datum::List(xy) = segment else {
                        panic!("{segment:?}")
                    };
                    let xy: Vec<f64> = xy
                        .iter()
                        .map(|x| match x {
                            Datum::Atom(x) => number(x).unwrap(),
                            _ => panic!("{segment:?}"),
                        })
                        .collect();
                    [[xy[0], xy[1]], [xy[2], xy[3]]]
                })
                .collect();
            Value::Skyline(Skyline::from_segments(&segments, axis(a), *d))
        }
        ("ly:skyline->points", [s, a]) => numbers(sky(s).to_points(axis(a))),
        ("ly:skyline-empty?", [s]) => Value::Bool(sky(s).is_empty()),
        ("ly:skyline-merge", [s, t]) => {
            let mut merged = sky(s);
            merged.merge(&sky(t));
            Value::Skyline(merged)
        }
        ("ly:skyline-height", [s, x]) => Value::Number(sky(s).height(n(x))),
        ("ly:skyline-max-height", [s]) => Value::Number(sky(s).max_height()),
        ("ly:skyline-pad", [s, p]) => Value::Skyline(sky(s).padded(n(p))),
        ("ly:skyline-distance", [s, t]) => Value::Number(sky(s).distance(&sky(t), 0.0)),
        ("ly:skyline-distance", [s, t, p]) => Value::Number(sky(s).distance(&sky(t), n(p))),
        ("ly:skyline-touching-point", [s, t]) => Value::Number(sky(s).touching_point(&sky(t), 0.0)),
        ("ly:skyline-touching-point", [s, t, p]) => {
            Value::Number(sky(s).touching_point(&sky(t), n(p)))
        }
        ("ly:skyline-max-height-position", [s]) => Value::Number(sky(s).max_height_position()),
        _ => panic!("the port has no {call:?}"),
    }
}

/// A value's numbers and booleans, flattened and written as Guile's `write`
/// writes them.
fn write_flat(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Number(n) if n.is_nan() => out.push("+nan.0".into()),
        Value::Number(n) if n.is_infinite() => {
            out.push(if *n > 0.0 { "+inf.0" } else { "-inf.0" }.into())
        }
        Value::Number(n) => out.push(format!("{n:?}")),
        Value::Bool(b) => out.push(if *b { "#t" } else { "#f" }.into()),
        Value::List(items) => items.iter().for_each(|item| write_flat(item, out)),
        _ => panic!("a call's value is numbers and booleans"),
    }
}

/// tools/bench/probe-skylines.scm's calls on the port: a line each, as
/// LilyPond's probe writes it, into `$SKYLINE_CALLS_OUT` (tools/bench/test/
/// skylines.test.mjs compares them with LilyPond's) or else
/// `skyline-calls.txt` in the tests' temporary directory.
#[test]
fn the_probe_s_skyline_calls() {
    let probe = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/bench/probe-skylines.scm"
    );
    let mut skylines = HashMap::new();
    let mut lines = Vec::new();
    for form in read_data(&std::fs::read_to_string(probe).unwrap()) {
        let Datum::List(items) = &form else { continue };
        match &items[..] {
            [
                Datum::Atom(define),
                Datum::Atom(name),
                expr @ Datum::List(_),
            ] if define == "define" && name.starts_with("probe-") => {
                let Value::Skyline(s) = eval(expr, &skylines) else {
                    panic!("{name}: a skyline")
                };
                skylines.insert(name.clone(), s);
            }
            [Datum::Atom(each), Datum::Atom(run), Datum::List(quoted)]
                if each == "for-each" && run == "probe-run" =>
            {
                let [_, Datum::List(calls)] = &quoted[..] else {
                    panic!("{quoted:?}")
                };
                for call in calls {
                    let mut out = Vec::new();
                    write_flat(&eval(call, &skylines), &mut out);
                    lines.push(out.join(" "));
                }
            }
            _ => {}
        }
    }
    assert_eq!(skylines.len(), 12, "the probe's skylines");
    assert_eq!(lines.len(), 60, "the probe's calls");
    let out = std::env::var_os("SKYLINE_CALLS_OUT").map_or_else(
        || std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("skyline-calls.txt"),
        Into::into,
    );
    std::fs::write(
        out,
        lines
            .iter()
            .map(|line| format!("{line}\n"))
            .collect::<String>(),
    )
    .unwrap();
}
