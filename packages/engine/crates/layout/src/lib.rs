//! The engine's layout (chapter 05, D104). So far 01 §3.7's O1 (MS3 steps
//! 2 and 3): LilyPond's stencils, boxes and transforms, its shapes and its
//! lines, with their Scheme face, as `stencil.cc`, `stencil-scheme.cc`,
//! `stencil-expression.cc`, `box.cc`, `transform.cc`,
//! `transform-scheme.cc`, `lookup.cc`, `line-interface.cc` and
//! `line-interface-scheme.cc` give them, under LilyPond's names and
//! arities; and O2's font metrics and font selection (steps 4 and 5), as
//! `font-metric.cc`, `font-metric-scheme.cc`, `open-type-font.cc`,
//! `open-type-font-scheme.cc`, `modified-font-metric.cc`,
//! `all-font-metrics.cc`, `all-font-metrics-scheme.cc`, `font-select.cc`
//! and `font-interface.cc` give them.
//!
//! - [`Interval`], [`Box`], the offsets' functions and [`Transform`]
//!   (`geometry.rs`), and `ly:make-transform` and the rest of
//!   `transform-scheme.cc`'s five.
//! - [`Stencil`] (`stencil.rs`): its extents and expression, translated,
//!   rotated, scaled, aligned, combined at an edge, stacked, added; the
//!   `ly:` functions of `stencil-scheme.cc`.
//! - `Lookup`'s shapes (`lookup.rs`): beams, boxes, polygons, frames,
//!   circles, brackets, triangles, slashes and lines, with
//!   `ly:round-filled-box`, `ly:round-polygon` and `ly:bracket`; `slur`
//!   and `bezier_sandwich` throw by name until `bezier.cc` (MS4).
//! - The line interface (`line.rs`): plain, dashed, dotted and zigzag lines
//!   and arrows from a grob's style ([`LineStyle`]), and
//!   `ly:line-interface::line`, which throws for a grob until the engine's
//!   grobs have a staff symbol (MS3's part 2).
//! - LilyPond's font metrics over W1's packs (`font.rs`):
//!   [`OpenTypeFont`], `Open_type_font` read from `crates/fonts`' packs,
//!   which a host gives with [`font::put_pack`] and `ly:system-font-load`
//!   finds in `All_font_metrics`' table, which `ly:reset-all-fonts`
//!   empties; [`font::ModifiedFontMetric`], a scaled font; the `ly:font-`
//!   and `ly:otf-` functions, five of them placeholders for what the packs
//!   do not hold.
//! - Font selection (`font_select.rs`): the music font a property chain
//!   selects in an output definition, Emmentaler's optical size and its
//!   scaling ([`font_select::select_font`]), and `ly:paper-get-font`.
//! - Text (MS3 step 8): Pango's fonts over W1's text packs (`pango.rs`,
//!   `pango-font.cc`, `pango-font-scheme.cc`), which font selection gives
//!   for text, and `text-interface.cc`'s markup interpretation (`text.rs`).
//! - Grobs (MS3 step 13, D108): layout's part of each grob record of a
//!   translation, kept in the music crate's slot by the record's number
//!   (`grob.rs`, [`grob::grobs`]), and `grob-property.cc`'s lookup with
//!   its callbacks: `ly:grob-property` and `ly:grob-set-property!`, which
//!   replace the music crate's, and the object alist's `ly:grob-object`
//!   and `ly:grob-set-object!`; parents, offsets and extents on both axes,
//!   and `grob.cc`'s callbacks `ly:grob::stencil-height` and
//!   `ly:grob::x-parent-positioning`, which replace the music crate's
//!   placeholders; a grob's origin, at which its warnings are given,
//!   `suicide` and its print stencil, and the cause the SVG writer links.
//!   `align.rs` has `align-interface.cc`'s `get_pure_child_y_translation`
//!   and `add_element`.
//!   A grob's class, its interfaces (`has_interface`) and an item's break
//!   direction and column (MS3 step 14, `item.rs`), with `ly:item?`,
//!   `ly:item-break-dir` and `ly:item-get-column`. Grob arrays in a grob's
//!   object slots, with `grob-array-scheme.cc`'s five and
//!   `ly:pointer-group-interface::add-grob` (`grob_array.rs`, MS3 step 14).
//!   The pure lookups on a grob (`unpure.rs`, MS3 step 14):
//!   `call_pure_function`, the pure property, coordinate and height, with
//!   `ly:grob-pure-property`, `ly:grob-pure-height` and
//!   `ly:grob-pure-relative-coordinate`. The rest of `grob-scheme.cc`'s
//!   face (MS3 step 14, `grob.rs`), with `ly:grob-default-font` over
//!   `font_select.rs`' `get_default_font`.
//! - What systems and paper columns are made of (MS3 step 15), part 3's
//!   functions ported first, each in its file's module with its lines:
//!   `Paper_score` (`paper_score.rs`), the System's constructors,
//!   `typeset_grob` and `add_column` (`system.rs`), the column's
//!   constructors, rank and `is_breakable` (`paper_column.rs`),
//!   `Axis_group_interface::add_element` (`axis_group.rs`) and
//!   `Separation_item`'s `add_item` and `add_conditional_item`
//!   (`separation_item.rs`). Spanners'
//!   bounds and the slots of their broken pieces (`spanner.rs`), with
//!   `spanner-scheme.cc`'s five and `spanner.cc`'s callbacks
//!   `ly:spanner::calc-normalized-endpoints`, `::kill-zero-spanned-time`
//!   and `::set-spacing-rods`, which replace the music crate's
//!   placeholders.
//! - What note heads, stems, rests, dots and note columns are joined with
//!   (MS3 step 16), part 3's and part 4's functions ported first, each in
//!   its file's module with its lines: `Stem`'s `add_head`,
//!   `duration_log`, `is_normal_stem` and `set_beaming` (`stem.rs`),
//!   `Note_column`'s `add_head` and `set_stem` (`note_column.rs`),
//!   `Dot_column::add_head` (`dot_column.rs`), `Rhythmic_head`'s
//!   `get_stem` and `set_dots` (`rhythmic_head.rs`) and
//!   `Side_position_interface::add_support` (`side_position.rs`); and
//!   `ly:rest::width` (`rest.rs`), which `Dot_column::add_head` takes of
//!   a dotted rest, the image's Scheme over `rest.cc`'s extent of its
//!   glyph.
//! - What accidentals are joined with (MS3 step 17), part 3's function
//!   ported first with its lines: `Accidental_placement::add_accidental`
//!   (`accidental_placement.rs`); `add_support` is step 16's.
//! - What scripts are put in rows with (MS3 step 19), part 4's functions
//!   ported first with their lines: `Script_column::add_side_positioned`
//!   (`script_column.rs`) and `Side_position_interface::is_on_x_axis`
//!   (`side_position.rs`).
//! - What volta brackets take their bar lines with (MS3 step 20), part
//!   4's function ported first with its lines:
//!   `Volta_bracket_interface::add_bar` (`volta_bracket.rs`).
//! - What multi-measure rests set their scripts' properties with (MS3
//!   step 42), part 4's function ported first with its lines:
//!   `make_script_from_event` (`script_engraver.rs`).
//! - What axis groups and alignments take their elements with (MS3 step
//!   21), part 3's and part 4's functions ported first, each in its file's
//!   module with its lines: `Align_interface::add_element` (`align.rs`),
//!   `Break_alignment_interface::add_element` (`break_alignment.rs`) and
//!   `Hara_kiri_group_spanner::add_interesting_item`
//!   (`hara_kiri_group_spanner.rs`), with `grob.cc`'s
//!   `ly:grob::y-parent-positioning`.
//! - What non-musical scripts are put in columns with (MS3 step 22), part
//!   4's function ported first with its lines:
//!   `Side_position_interface::is_on_y_axis` (`side_position.rs`).
//! - What the staff callbacks remove empty groups and tell staves from
//!   loose lines with (MS3 step 27), part 4's and part 5's functions
//!   ported first with their lines: `Hara_kiri_group_spanner::
//!   consider_suicide` and `request_suicide`, with their `_alone` and
//!   unpure forms (`hara_kiri_group_spanner.rs`), over part 3's
//!   `Axis_group_interface::get_children` (`axis_group.rs`); and
//!   `Page_layout_problem::is_spaceable` (`page_layout_problem.rs`).
//! - The staff symbol (MS3 step 27, `staff_symbol.rs`): its callbacks
//!   `ly:staff-symbol::calc-line-positions`, `::height` and `::print`,
//!   the music crate's placeholders until then, with part 3's
//!   `Paper_column::break_align_width` (`paper_column.rs`) and
//!   `Break_alignment_interface::find_nonempty_break_align_group`
//!   (`break_alignment.rs`), which `::print` calls, ported first.
//! - The staff symbol's referencers (MS3 step 27,
//!   `staff_symbol_referencer.rs`): `ly:staff-symbol-referencer::callback`,
//!   the music crate's placeholder until then, the staff position, Y
//!   offset, staff space, line thickness and staff radius read through the
//!   staff symbol, and `ly:grob-staff-position`,
//!   `ly:staff-symbol-line-thickness`, `ly:staff-symbol-staff-space` and
//!   `ly:staff-symbol-staff-radius`.
//! - Note heads (MS3 step 37, `note_head.rs`): `ly:note-head::print`,
//!   `::stem-x-shift`, `::include-ledger-line-height`,
//!   `::calc-stem-attachment` and `::calc-tab-stem-attachment`, the music
//!   crate's placeholders until then, and `ly:note-head::stem-attachment`
//!   over the font's attachments; and the rest of `rhythmic-head.cc`
//!   (`rhythmic_head.rs`), a head's dots, their count and its duration log.
//! - Rests and the note column's main extent (MS3 step 37): `ly:rest::print`,
//!   `::height`, `::pure-height`, `::y-offset-callback` and
//!   `::calc-cross-staff`, the music crate's placeholders until then, with
//!   the rest's ledgered glyphs (`rest.rs`); and
//!   `ly:note-column::calc-main-extent` over `%host-call` (D127,
//!   `note_column.rs`).
//! - Dot columns (MS3 step 37): `ly:dot-column::calc-positioning-done`,
//!   the music crate's placeholder until then, with its configurations of
//!   dots over the heads' skyline (`dot_column.rs`), over `%host-call`; and
//!   the staff symbol's ledgers' positions and `on_line`
//!   (`staff_symbol.rs`), which it asks of each dot.
//! - Stems (MS3 step 38, `stem.rs`): the fourteen `ly:stem::` callbacks,
//!   the music crate's placeholders until then, primitives over
//!   `%host-call`, so that they print as LilyPond's subrs: the
//!   direction, length and begin position with their pure forms, the
//!   heads' positioning, stem info, offset, width, height with its pure
//!   form, cross-staff and print; the beam's functions they call are
//!   `beam.rs`' since MS4 step 3, the tremolo's, MS4 step 5's, throw
//!   `misc-error` by name; and `directional-element-interface.cc`
//!   (`direction.rs`).
//! - Beams (MS4): the beam engravers' grobs and patterns (step 2,
//!   `beam_engraver.rs`, `auto_beam_engraver.rs`, `beaming_pattern.rs`)
//!   and the Beam's callbacks but its stem lengths (step 3, `beam.rs`),
//!   the music crate's placeholders until then, primitives over
//!   `%host-call`: direction and knees, beaming, normal stems, segments,
//!   X positions, stem shortening, cross-staff, the tremolo's springs and
//!   rods and print, and the rest offsets under a beam; and the quanting
//!   (step 4, `beam_quanting.rs`), the stems' lengths set from it, and
//!   `Beam_collision_engraver` (`beam_collision_engraver.rs`), the grobs a
//!   beam should not cross; the stem tremolos' ten callbacks (step 5,
//!   `stem_tremolo.rs`) and `Chord_tremolo_engraver`
//!   (`chord_tremolo_engraver.rs`), a tremolo repeat's beam; with `Tweak_engraver` (`tweak_engraver.rs`),
//!   each grob given its event's `\tweak`s, which no part had.
//! - Slurs (MS4 step 7): `Slur_engraver` and `Phrasing_slur_engraver`
//!   (`slur_engraver.rs`, `phrasing_slur_engraver.rs`), which shadow the
//!   music crate's of their names but where a placeholder makes the
//!   heads; and the slurs' callbacks but their control points (step 8),
//!   the music crate's placeholders until then, primitives over
//!   `%host-call` (`slur.rs`): direction, cross-staff, print, height and
//!   pure height, and the outside-slur offsets and cross-staff of the
//!   scripts that avoid them; with what they read of `bezier.cc`
//!   (`bezier.rs`) and `note-column.cc`.
//! - Tuplets (MS4 step 12): `Tuplet_engraver` (`tuplet_engraver.rs`), the
//!   brackets and numbers of nested tuplets and `currentTupletDescription`,
//!   which shadows the music crate's placeholder; and the bracket's five
//!   callbacks, the music crate's placeholders until then, primitives over
//!   `%host-call` (`tuplet_bracket.rs`): direction, X positions, positions
//!   over the beam it follows or its columns, cross-staff and print; and
//!   the number's three (step 13, `tuplet_number.rs`), its print and its
//!   offsets, on the bracket or against a kneed beam. With
//!   `Drum_notes_engraver` (step 13, `drum_note_engraver.rs`), the drums'
//!   heads and scripts by `drumStyleTable`.
//! - Lyrics (MS4 step 15): `Lyric_engraver` (`lyric_engraver.rs`),
//!   `Extender_engraver` (`extender_engraver.rs`), `Hyphen_engraver`
//!   (`hyphen_engraver.rs`), `Stanza_number_engraver` and
//!   `Stanza_number_align_engraver` (`stanza_number_engraver.rs`), which
//!   shadow the music crate's placeholders, the syllables placed on the
//!   heads of the voice their lyrics follow and aligned in its melismata
//!   (`melisma_busy`, the music crate's); and the extender's print
//!   (`lyric_extender.rs`) and the hyphen's print and spacing rods
//!   (`lyric_hyphen.rs`), the music crate's placeholders until then,
//!   primitives over `%host-call`.
//! - Accidentals (MS3 step 39): the four `ly:accidental-interface::`
//!   callbacks (`accidental.rs`), print, height, horizontal skylines and
//!   the tied ones' removal, and
//!   `ly:accidental-placement::calc-positioning-done`
//!   (`accidental_placement.rs`), which places them over the skylines of
//!   their note columns' heads and stems: the music crate's placeholders
//!   until then, primitives over `%host-call`, as the stem's.
//! - Ledger lines (MS3 step 39, `ledger_line_spanner.rs`):
//!   `ly:ledger-line-spanner::set-spacing-rods`, the rods that keep the
//!   ledgers of neighbouring columns apart, and `::print`, the ledgers
//!   shortened between columns and by accidentals, merged at each
//!   position: the music crate's placeholders until then, primitives over
//!   `%host-call`, as the stem's.
//! - Side positions (MS3 step 40, part 4's, `side_position.rs`):
//!   `ly:side-position-interface::x-aligned-side`, `::y-aligned-side` and
//!   `::pure-y-aligned-side`, a grob put beside the skylines of its
//!   support, past its padding and kept from the staff, and
//!   `::move-to-extremal-staff`, a mark moved onto the outermost staff;
//!   which `SystemStartBar` and `BarNumber` read in every score: the
//!   music crate's placeholders until then, primitives over `%host-call`,
//!   as the stem's.
//! - Clefs and key signatures (MS3 step 40, `clef.rs`,
//!   `key_signature.rs`): `ly:clef::calc-glyph-name`, `ly:clef::print`
//!   and `ly:key-signature-interface::print`, the glyphs of the clef and
//!   of each alteration at its staff positions, kerned: the music crate's
//!   placeholders until then, primitives over `%host-call`, as the stem's.
//!   And since MS3 step 42 `ly:clef-modifier::calc-parent-alignment`
//!   (`clef_modifier.rs`), the alignment on the clef of the `8` or `15`
//!   below or above it for the clef's glyph: the music crate's placeholder
//!   until then, a primitive over `%host-call`.
//! - The system start delimiter (MS3 step 40, `system_start_delimiter.rs`):
//!   `ly:system-start-delimiter::print`, a bracket, brace, bar or square
//!   bracket as high as the staves it begins, or none where it collapses:
//!   the music crate's placeholder until then, a primitive over
//!   `%host-call`, as the stem's. And the break-alignable interface's
//!   `ly:break-alignable-interface::find-parent` and
//!   `::self-align-callback` (`break_alignment.rs`), which `BarNumber`'s
//!   `X-offset` reads, the image's Scheme.
//! - Flags (MS3 step 41, `flag.rs`): the six `ly:flag::` callbacks, the
//!   flag's glyph name, print with its stroke, width and offsets from its
//!   stem: the music crate's placeholders until then, primitives over
//!   `%host-call`, as the stem's.
//! - Note collisions (MS3 step 41, `note_collision.rs`):
//!   `ly:note-collision-interface::calc-positioning-done`, the note
//!   columns of several voices shifted apart on X, merged heads hidden
//!   and dots killed or turned: the music crate's placeholder until then,
//!   a primitive over `%host-call`, as the stem's; with what it reads of
//!   a note column (`note_column.rs`) and of a stem's head positions
//!   (`stem.rs`).
//! - Rest collisions (MS3 step 41, `rest_collision.rs`):
//!   `ly:rest-collision::calc-positioning-done`, the rests of several
//!   voices moved clear of each other and of the other voices' notes on
//!   Y, the music crate's placeholder until then, and
//!   `ly:rest-collision::force-shift-callback-rest`, chained on each
//!   rest's `Y-offset` to trigger it: primitives over `%host-call`, as
//!   the stem's.
//! - Multi-measure rests (MS3 step 42, `multi_measure_rest.rs`): the four
//!   `ly:multi-measure-rest::` callbacks, the rest's symbol (a big rest or
//!   church rests) between its columns' bar lines, its height, and the
//!   rods of its symbol and of its number's or text's stencil: the music
//!   crate's placeholders until then, primitives over `%host-call`, as the
//!   stem's.
//! - Line spanners (MS3 step 52, `line_spanner.rs`): the bound infos of
//!   `ly:line-spanner::` and `ly:horizontal-line-spanner::`, the line
//!   spanner's cross-staff and print, which text spanners, glissandi,
//!   dynamic text spanners and voice followers read: the music crate's
//!   placeholders until then, primitives over `%host-call`, as the stem's;
//!   with them `Line_interface::arrows` on a grob and
//!   `ly:line-interface::line` drawing on its grob (`line.rs`, OPEN A23).
//! - Self-alignment (MS3 step 27, `self_alignment.rs`): the five
//!   `ly:self-alignment-interface::` callbacks, the music crate's
//!   placeholders until then, which align a grob on its own extent
//!   (`x-aligned-on-self`, `y-aligned-on-self`, `pure-y-aligned-on-self`)
//!   and on its X parent's (`centered-on-x-parent`,
//!   `aligned-on-x-parent`), with part 3's
//!   `Paper_column::get_interface_extent` (`paper_column.rs`) ported first.
//! - The pure heights from neighbors, the staff grouper and the axis
//!   groups' Scheme face (MS3 step 27):
//!   `ly:pure-from-neighbor-interface::calc-pure-relevant-grobs`
//!   (`pure_from_neighbor.rs`), the music crate's placeholder until then;
//!   the staff grouper's `get_extremal_staff` and `maybe_pure_within_group`
//!   (`staff_grouper.rs`); and `ly:relative-group-extent` and
//!   `ly:generic-bound-extent` (`axis_group.rs`), with part 3's
//!   `Axis_group_interface::relative_group_extent`, `generic_bound_extent`
//!   and `internal_calc_pure_relevant_grobs` ported first.
//! - The axis groups' extents and pure heights (MS3 step 28,
//!   `axis_group.rs`): `ly:axis-group-interface::width`, `::height`,
//!   `::pure-height`, `::calc-pure-relevant-grobs`, `::calc-pure-y-common`,
//!   `::calc-x-common`, `::calc-y-common` and `::adjacent-pure-heights`,
//!   the music crate's placeholders until then, with part 2's
//!   `Item::pure_find_visible_prebroken_piece` and `break_visible`
//!   ported first, over the spanner's pure property cache.
//! - The axis groups' skylines and staff spacing (MS3 step 28,
//!   `axis_group.rs`): `ly:axis-group-interface::calc-skylines`
//!   (`skyline_spacing`, which places the outside-staff grobs, with
//!   flower's `Interval_set`), `::combine-skylines`,
//!   `::calc-staff-staff-spacing` and `::calc-pure-staff-staff-spacing`,
//!   the music crate's placeholders until then, over the skyline crate's
//!   pairs; and the Scheme faces they call of the staff grouper's
//!   `maybe_pure_within_group` (`staff_grouper.rs`) and of
//!   `Hara_kiri_group_spanner::request_suicide`
//!   (`hara_kiri_group_spanner.rs`), whose Rust now calls them.
//! - `VerticalAxisGroup`'s hara-kiri callbacks (MS3 step 28,
//!   `hara_kiri_group_spanner.rs`): `ly:hara-kiri-group-spanner::y-extent`,
//!   `::calc-skylines`, `::pure-height` and `::force-hara-kiri-callback`,
//!   the music crate's placeholders until then, which remove an empty
//!   group before its extent, skylines or offset is read; with
//!   `consider_suicide` and `Axis_group_interface::get_children` in
//!   Scheme, whose Rust now calls them.
//! - The spacing specs (MS3 step 28, `spacing_spec.rs`), part 5's
//!   `page-layout-problem.cc` functions ported first: the spec of the
//!   spring between two staves or loose lines (`get_spacing_spec`), a
//!   number of one (`read_spacing_spec`) and the spacing forced by
//!   `alignment-distances` (`get_fixed_spacing`), with part 2's spanner
//!   pure property cache (`spanner.rs`) and `Grob::get_system (Grob *)`;
//!   in Scheme, which the alignment calls, the Rust through a Host.
//! - The vertical alignment (MS3 step 28, `align.rs`):
//!   `ly:align-interface::align-to-minimum-distances` and
//!   `::align-to-ideal-distances`, the music crate's placeholders until
//!   then, the elements stacked by their skylines and spacing specs (pure
//!   too, for `get_pure_minimum_translations`); and step 34's page-layout
//!   problem of one system that the ideal distances solve
//!   (`page_layout_problem.rs`), over step 29's springs and solver
//!   (`spring.rs`, `simple_spacer.rs`), ported first.
//! - The springs (MS3 step 29, `spring.rs`): `spring.cc` whole, with
//!   `merge_springs`, and `spring-smob.cc`'s smob, `ly:make-spring`,
//!   `ly:spring?` and `ly:spring-set-inverse-compress-strength!` and
//!   `-stretch-strength!`.
//! - The rods and what columns keep of the spacing problem (MS3 step 29):
//!   `rod.cc`'s [`rod::Rod`], its `add_to_cols` and `bounds_protrusion`
//!   (`rod.rs`), and `spaceable-grob.cc`'s minimum distances, rods and
//!   springs on columns (`spaceable.rs`), which have no Scheme face.
//! - The solver's rods and the line's columns (MS3 step 29,
//!   `simple_spacer.rs`): `simple-spacer.cc`'s `add_rod`, `rod_force` and
//!   `force_penalty`, and `get_line_configuration` and `get_line_forces`,
//!   a line's columns spaced by their springs and rods into
//!   `column-x-positions.cc`'s [`simple_spacer::ColumnXPositions`]; and
//!   `simple-spacer-scheme.cc`'s `ly:solve-spring-rod-problem`.
//! - The separation items' skylines (MS3 step 30, `separation_item.rs`):
//!   `ly:separation-item::calc-skylines`, the music crate's placeholder
//!   until then, the skylines left and right of the boxes of a column's or
//!   a note column's elements, padded, in Scheme over `%boxes-skylines`;
//!   Rust since MS3 step 59, a callback of the table under a primitive.
//! - The minimum distance between columns (MS3 step 30,
//!   `spacing_interface.rs`): `spacing-interface.cc`'s `skylines` and
//!   `minimum_distance`, from the skylines of the separation items of a
//!   spacing grob's items and the right ones' `conditional_skyline`
//!   (`separation_item.rs`), whose accidentals
//!   `get_relevant_accidentals` keeps (`accidental_placement.rs`).
//! - A spacing grob's columns (MS3 step 31, `spacing_interface.rs`):
//!   `spacing-interface.cc`'s `right_column`, `left_column`, its note
//!   columns left and right, and `extremal_break_aligned_grob`, the
//!   break-aligned grob sticking out furthest towards it; and the spacing
//!   options read from a `SpacingSpanner`, with the space of a duration
//!   (`spacing_options.rs`), which have no Scheme face; and the springs
//!   by duration alone between columns (`spacing_basic.rs`), between
//!   breakable columns from the measure length and between musical ones
//!   from the shortest duration playing, which the spacing spanner calls;
//!   and a voice's spring from its note column adjusted by the heads, the
//!   skylines, a bar line and the stems' directions (`note_spacing.rs`),
//!   with what it reads of a note column (`note_column.rs`), a stem
//!   (`stem.rs`), a grob's direction (`grob.rs`) and a bar line's staff
//!   positions (`staff_spacing.rs`); and the spring from a non-musical
//!   column's prefatory matter by its `space-alist`, corrected for a
//!   down-stem after a bar line (`staff_spacing.rs`), and the distance
//!   between two separation items with its rod, and whether one is empty
//!   (`separation_item.rs`).
//! - The springs and rods between all columns (MS3 step 31,
//!   `spacing_spanner.rs`): `ly:spacing-spanner::set-springs` and
//!   `::calc-common-shortest-duration`, the music crate's placeholders
//!   until then, Scheme that reaches the Rust port through `%host-call`
//!   (D127): the columns' neighbours and loose columns
//!   (`spacing_determine_loose_columns.rs`, step 32's file pulled
//!   forward), a spring between each pair of columns by its note or staff
//!   spacings, and rods from the columns' skylines; with the system's
//!   used columns and its root (`system.rs`, step 33's lines pulled
//!   forward).
//! - The Paper_score's break indices and ranks (MS3 step 32,
//!   `paper_score.rs`): its used columns and the breakable ones among
//!   them, found once and kept, with `Output_def::c_variable` borrowed;
//!   what the axis groups' pure heights and the spacing specs' pure bound
//!   read of them, and `ly:paper-score-paper-systems`, Scheme over
//!   `%host-call` (D127).
//! - The Paper_score's one line set up (MS3 step 32,
//!   `constrained_breaking.rs`): `Constrained_breaking`'s constructors and
//!   `initialize`, the force of each line between its breaks and the
//!   breaks to start at, with `line_dimension_interval` borrowed; and
//!   `space_line`, the line between two breaks spaced by its springs and
//!   rods. Its loose columns are then placed between their neighbours
//!   (`spacing_loose_columns.rs`).
//! - The breakable items' pieces before line breaking (MS3 step 32,
//!   step 33's lines pulled forward, D129): `System::pre_processing`'s
//!   first three scopes (`system.rs`), each non-musical item copied for
//!   each side of a break (`item.rs`), the copies' objects their
//!   original's by break direction (`break_substitution.rs`), the items
//!   not visible there killed, and the parents fixed up (`grob.rs`); a
//!   traced compile makes them before its reads (`dump.rs`).
//! - What a spanner gives breaking (MS3 step 33): its bounds moved to
//!   their pieces and the ranks of the systems it spans (`spanner.rs`,
//!   `item.rs`, with the systems' ranks in `system.rs`), and the rods of
//!   `ly:spanner::set-spacing-rods`, past the breakable columns between
//!   its bounds (`System::broken_col_range`), over `%host-call` (D127).
//! - The System before and after breaking (MS3 step 33, `system.rs`):
//!   `pre_processing` whole, every grob's `before-line-breaking` and
//!   `springs-and-rods` read after the pieces, and `post_processing`, the
//!   system moved up by its height and its grobs' stencils read; and the
//!   System's callbacks, the music crate's placeholders until then: its
//!   height and pure height, its pure-relevant grobs, footnotes, vertical
//!   skyline elements and vertical alignment, with libc++'s `std::sort`
//!   over `%host-call` for the footnotes; and its staves.
//! - The paper systems (MS3 step 33): `Paper_score::get_paper_systems`
//!   (`paper_score.rs`) breaks the System into its lines and makes a
//!   paper system of each (`system.rs`), a prob (the music crate's
//!   `Prob`) holding its stencil, its grobs' print stencils combined by
//!   layer, and what page breaking reads of it, so
//!   `ly:paper-score-paper-systems` no longer throws.
//! - The book's `Paper_book` (MS3 step 35, `paper_book.rs`): `Book::process`,
//!   which makes a `Paper_book` of a book and of each of its parts, their
//!   papers scaled and parented and normalized as LilyPond's, and lists
//!   the items each processes; and `Paper_book`'s constructor,
//!   `top_paper`, `add_score` and `add_bookpart`, which a compile fills
//!   with its scores' Paper_scores; and the systems it prints
//!   (`get_system_specs`, `systems`): its titles' paper systems, its
//!   Paper_scores' and its markup lists' lines, in order, or its parts'.
//!   And its pages (`page.rs`): `Page_breaking::draw_page`, a page of
//!   `page.scm`'s `make-page` holding a set of systems spaced by the
//!   page-layout problem, a ragged last page by the force of the page
//!   before (`fixed_force_solution`), and `make_pages`, the pages of
//!   systems already divided among them, with their footnotes numbered
//!   and their labels' pages kept in the top paper's tables. Then
//!   `Paper_book::pages`: the paper's `page-breaking`'s pages (since MS3
//!   step 62 `ly:optimal-breaking`, `optimal_page_breaking.rs`, over
//!   `page_breaking.rs`' two sides and `page_spacing.rs`, which break the
//!   lines and the pages together; since MS8 step 9's part 3 the other
//!   five, `page_breakers.rs`, with `Page_turn_engraver`,
//!   `page_turn_engraver.rs`), each page's stencil `page.scm`'s
//!   `page-stencil`, the paper's `page-post-process`, and the systems from
//!   the pages; with `ly:paper-book-paper`, `ly:paper-book-scopes`,
//!   `ly:paper-book-pages`, `ly:paper-book-systems` and
//!   `ly:paper-book-header`. And
//!   `Paper_book::output` with its `output_aux`, as far as the pages'
//!   stencils, which the core writes with `crates/svg`, numbered from the
//!   paper's `first-page-number`, each part's paper told its first page
//!   and whether it is the last part. And the page markers
//!   (`page_marker.rs`, MS3 step 64) a toplevel `\label`, `\pageBreak`
//!   and their kin leave among a book's items, which the system specs
//!   read: a permission for the spec before, a label for the next
//!   Paper_score's first column or markup line (`set_labels`), whose page
//!   `make_pages` keeps; and `ly:paper-system?`.
//! - Layout's translator classes ([`CLASSES`], D108), which the core passes
//!   at its `\layout` call: so far `Score_engraver`'s layout part
//!   (`score_engraver.rs`, MS3 step 15), the Paper_score, the System and
//!   the grobs typeset into it; and `Paper_column_engraver`
//!   (`paper_column_engraver.rs`), the paper columns of each timestep and
//!   the items put on them; `Note_heads_engraver`
//!   (`note_heads_engraver.rs`, MS3 step 16), the note heads with their
//!   staff positions, `Rest_engraver` (`rest_engraver.rs`), the rests,
//!   `Stem_engraver` (`stem_engraver.rs`), the stems, their stubs and
//!   flags, with the beam counts that `\set` sets, `Dots_engraver`
//!   (`dots_engraver.rs`) and `Dot_column_engraver`
//!   (`dot_column_engraver.rs`), the dots and their columns,
//!   `Rhythmic_column_engraver` (`rhythmic_column_engraver.rs`), the note
//!   columns, and `Ledger_line_engraver` (`ledger_line_engraver.rs`), the
//!   ledger line spanners with their bounds; `Clef_engraver`
//!   (`clef_engraver.rs`, MS3 step 17), the clefs and their modifiers,
//!   with the image's `invalidate-alterations`; `Key_engraver`
//!   (`key_engraver.rs`), the key signatures and their cancellations, with
//!   `keyAlterations`, `lastKeyAlterations` and `tonic`;
//!   `Time_signature_engraver` (`time_signature_engraver.rs`), the time
//!   signatures; `Accidental_engraver` (`accidental_engraver.rs`), the
//!   accidentals made through their heads' engravers and their placements,
//!   with `localAlterations` and `autoAccidentals`' rules run from the
//!   image; and `Tie_engraver` (`tie_engraver.rs`, MS4's, ported first for
//!   the accidentals' tied entries), the ties and their columns, which
//!   keeps the music crate's class where a placeholder makes the heads;
//!   `Staff_symbol_engraver` (`staff_symbol_engraver.rs`, MS3 step 18),
//!   the staff symbols with their bounds, each grob's `staff-symbol`;
//!   `Bar_engraver` (`bar_engraver.rs`), the bar lines of the bar types
//!   that `whichBar`, the measures, repeats, sections, fines, segnos and
//!   caesuras give, with the image's
//!   `bar-line::calc-glyph-name-for-direction`, and `currentBarLine`;
//!   `Bar_number_engraver` (`bar_number_engraver.rs`), the bar numbers at
//!   the bar lines and allowed breaks, with the image's
//!   `barNumberVisibility` and `barNumberFormatter`;
//!   `System_start_delimiter_engraver` (`system_start_delimiter_engraver.rs`),
//!   the system start delimiters of `systemStartDelimiter` and
//!   `systemStartDelimiterHierarchy` with the staves they span;
//!   `Text_engraver` (`text_engraver.rs`, MS3 step 19), the text scripts
//!   with their priorities and directions, on their note columns;
//!   `Script_row_engraver` (`script_row_engraver.rs`), the script rows
//!   that take the accidental placements and the scripts on the X axis;
//!   `Dynamic_engraver` (`dynamic_engraver.rs`), the dynamic texts on
//!   their note columns and the hairpins and dynamic text spanners with
//!   their bounds, with `text.rs`' `Text_interface::is_markup`;
//!   `Dynamic_align_engraver` (`dynamic_align_engraver.rs`), the dynamic
//!   line spanners over them with their bounds and support;
//!   `Volta_engraver` (`volta_engraver.rs`, MS3 step 20), the volta
//!   brackets of `repeatCommands` and the volta span events, with the bar
//!   lines they take and the image's `volta-bracket::calc-hook-visibility`,
//!   and the volta bracket spanners over them; and
//!   `Ottava_spanner_engraver` (`ottava_engraver.rs`), the ottava brackets
//!   on their note columns, with `ottavaStartNow`, `middleCOffset` and
//!   `middleCPosition` through the image's `ly:set-middle-C!`;
//!   `Axis_group_engraver` (`axis_group_engraver.rs`, MS3 step 21), a
//!   context's VerticalAxisGroup with `hasAxisGroup`, its elements the
//!   grobs of the context and below, and its `items-worth-living`;
//!   `Vertical_align_engraver` (`vertical_align_engraver.rs`), the
//!   VerticalAlignment that takes the axis groups through
//!   `align.rs`' `add_element`, in the order `alignAboveContext` and
//!   `alignBelowContext` give, or a StaffGrouper;
//!   `Break_align_engraver` (`break_align_engraver.rs`), each timestep's
//!   BreakAlignment, the X parent of the break-alignable items, and its
//!   BreakAlignGroups, one per `break-align-symbol`, that take the
//!   break-aligned items; `Pure_from_neighbor_engraver`
//!   (`pure_from_neighbor_engraver.rs`), the `neighbors` of the items with
//!   `pure-from-neighbor-interface`, the other items of the columns
//!   around them, set at finalize; `Keep_alive_together_engraver`
//!   (`keep_alive_together_engraver.rs`), the `keep-alive-with` and
//!   `make-dead-when` of the hara-kiri groups below it, by their
//!   `remove-layer`s, set at finalize; `Grob_pq_engraver`
//!   (`grob_pq_engraver.rs`), `busyGrobs`, the grobs of each timestep with
//!   an event's length by their ends, dropped when they have ended;
//!   `Forbid_line_break_engraver` (`forbid_line_break_engraver.rs`),
//!   the Score's `forbidBreak` while a note head or rest of `busyGrobs`
//!   is still playing; `Separating_line_group_engraver`
//!   (`separating_line_group_engraver.rs`, MS3 step 22), a StaffSpacing
//!   at each timestep with a non-musical item, with `hasStaffSpacing`,
//!   its columns and break-aligned items, and the note spacings' next
//!   break-aligned items; `Spanner_break_forbid_engraver`
//!   (`spanner_break_forbid_engraver.rs`, MS3 step 22), the Score's
//!   `forbidBreak` while a spanner with `unbreakable-spanner-interface`
//!   whose `breakable` is not `#t` runs; `Spacing_engraver`
//!   (`spacing_engraver.rs`, MS3 step 22), a SpacingSpanner over the
//!   Score's columns, with their shortest playing and starting durations;
//!   `Note_spacing_engraver` (`note_spacing_engraver.rs`, MS3 step 22), a
//!   NoteSpacing at each timestep with a note column or rhythmic grob, its
//!   `left-items` those, the last one's `right-items` them and the
//!   parent's last one's the command column after a staff spacing;
//!   `Grace_spacing_engraver` (`grace_spacing_engraver.rs`, MS3 step 22),
//!   a GraceSpacing over each run of grace timesteps and the timestep
//!   after it, each column's `grace-spacing`; and
//!   `Non_musical_script_column_engraver`
//!   (`non_musical_script_column_engraver.rs`, MS3 step 22), a
//!   ScriptColumn at each timestep with two or more non-musical scripts,
//!   its `scripts` the live ones on the Y axis; and `Collision_engraver`
//!   (`collision_engraver.rs`, MS3 step 41), a NoteCollision at each
//!   timestep with two or more note columns without rests, which takes
//!   them; and `Rest_collision_engraver` (`rest_collision_engraver.rs`,
//!   MS3 step 41), a RestCollision at each timestep with two or more
//!   sounding columns, a rest's among them, which takes them; and
//!   `Multi_measure_rest_engraver` (`multi_measure_rest_engraver.rs`,
//!   MS3 step 42), a MultiMeasureRest of each rest event over the
//!   measures it lasts, with its number, scripts and texts; and
//!   `Span_bar_engraver` (`span_bar_engraver.rs`, MS3 step 42), a SpanBar
//!   at each timestep with two or more bar lines below its context, which
//!   takes them and gives each its `has-span-bar`, and
//!   `Span_bar_stub_engraver` (`span_bar_stub_engraver.rs`), a
//!   SpanBarStub in each context between them that the span bar crosses;
//!   and `Instrument_name_engraver` (`instrument_name_engraver.rs`, MS3
//!   step 43), an InstrumentName of each name its context gives, an
//!   element of the root system over the context's axis groups, printed
//!   and placed by output-lib.scm's `system-start-text::` callbacks;
//!   `Text_spanner_engraver` (`text_spanner_engraver.rs`, MS3 step 52),
//!   the text spanners of the text span events with their note columns
//!   and bounds, and `Note_head_line_engraver`
//!   (`note_head_line_engraver.rs`), a VoiceFollower from the last head to
//!   a head on another staff where `followVoice` is set.
//! - The grobs' default skylines (MS3 step 26, D111, `skylines.rs`):
//!   `stencil-integral.cc`'s `interpret_stencil_for_skyline`, a stencil's
//!   expression walked into segments under a transform, and
//!   `lazy-skyline-pair.hh`'s `Lazy_skyline_pair`, which builds them into
//!   `crates/skyline`'s skylines, for every head the walk knows; and the
//!   `Grob::` callbacks of the default skylines, under their names and
//!   arities (D67), eight of them the music crate's placeholders until
//!   then: from the stencil, from the extents' box (which the constructor
//!   sets where a grob has none) and from the elements' skylines, each
//!   with its pure form, over `ly:skyline?`'s smob, whose points
//!   `ly:skyline->points` gives (`skyline-scheme.cc`).
//! - `dimensions-scheme.cc`'s units (`geometry.rs`, MS3 step 14): `ly:pt`,
//!   `ly:cm`, `ly:inch`, `ly:mm` and `ly:bp`, which replace the scheme
//!   crate's.
//! - The data form of LilyPond's stencils (`dump.rs`, MS3 step 6), in which
//!   the references of the pages and markups are written: read into
//!   stencils and fonts ([`dump::read`], `%dump->stencil`) and written
//!   back ([`dump::dump`], `%stencil->dump`); and the laid-out grobs'
//!   lines of a traced compile in `layout.ly`'s form, every field after
//!   grobs.tsv's six unreached (MS3 step 25, [`dump::lay_out`]) but
//!   the Y extent (step 28) and, once the System is broken, the X and Y
//!   offsets (step 33), which `%dump-read` reads (`dump::PRELUDE`).
//!
//! [`install`] defines them in a VM's `(lily)` after the music crate's, and
//! [`EXTENSION`] restores an image of such a VM: its stencils, which
//! `define-markup-commands.scm` makes at load (`empty-stencil`,
//! `point-stencil`), among them. LilyPond's programming errors (an
//! improbable offset) are not written.
#![no_std]
extern crate alloc;

pub mod accidental;
pub mod accidental_engraver;
pub mod accidental_placement;
pub mod align;
pub mod ambitus_engraver;
pub mod apply_output_engraver;
pub mod arpeggio;
pub mod arpeggio_engraver;
pub mod auto_beam_engraver;
pub mod axis_group;
pub mod axis_group_engraver;
pub mod balloon;
pub mod bar_engraver;
pub mod bar_number_engraver;
pub mod beam;
pub mod beam_collision_engraver;
pub mod beam_engraver;
pub mod beam_quanting;
pub mod beaming_pattern;
pub mod bezier;
pub mod bracket;
pub mod break_align_engraver;
pub mod break_alignment;
pub mod break_substitution;
pub mod breathing_sign;
pub mod caesura_engraver;
mod callbacks;
pub mod cell;
pub mod chord_tremolo_engraver;
pub mod clef;
pub mod clef_engraver;
pub mod clef_modifier;
pub mod cluster;
pub mod collision_engraver;
pub mod completion;
pub mod constrained_breaking;
pub mod cue_clef_engraver;
pub mod direction;
pub mod dot_column;
pub mod dot_column_engraver;
pub mod dots_engraver;
pub mod drum_note_engraver;
pub mod dump;
pub mod dynamic_align_engraver;
pub mod dynamic_engraver;
pub mod episema_engraver;
pub mod extender_engraver;
pub mod fingering_column;
pub mod fingering_column_engraver;
pub mod fingering_engraver;
pub mod flag;
pub mod font;
pub mod font_select;
pub mod font_size_engraver;
pub mod forbid_line_break_engraver;
mod geometry;
pub mod grace_spacing_engraver;
pub mod grid;
pub mod grob;
pub mod grob_array;
pub mod grob_pq_engraver;
pub mod hairpin;
pub mod hara_kiri_group_spanner;
pub mod horizontal_bracket;
pub mod horizontal_bracket_engraver;
pub mod hyphen_engraver;
pub mod instrument_name_engraver;
pub mod instrument_switch_engraver;
pub mod interval_minefield;
pub mod item;
pub mod jump_engraver;
pub mod keep_alive_together_engraver;
pub mod key_engraver;
pub mod key_signature;
pub mod kievan_ligature;
pub mod laissez_vibrer_engraver;
pub mod ledger_line_engraver;
pub mod ledger_line_spanner;
pub mod ligature;
pub mod line;
pub mod line_spanner;
pub mod lookup;
pub mod lyric_engraver;
pub mod lyric_extender;
pub mod lyric_hyphen;
pub mod mark_engraver;
pub mod measure;
pub mod melody_engraver;
pub mod mensural_ligature;
pub mod metronome_engraver;
pub mod multi_measure_rest;
pub mod multi_measure_rest_engraver;
pub mod new_fingering_engraver;
pub mod non_musical_script_column_engraver;
pub mod note_collision;
pub mod note_column;
pub mod note_head;
pub mod note_head_line_engraver;
pub mod note_heads_engraver;
pub mod note_name_engraver;
pub mod note_spacing;
pub mod note_spacing_engraver;
pub mod optimal_page_breaking;
pub mod ottava_bracket;
pub mod ottava_engraver;
pub mod page;
pub mod page_breakers;
pub mod page_breaking;
pub mod page_layout_problem;
pub mod page_marker;
pub mod page_spacing;
pub mod page_turn_engraver;
pub mod pango;
pub mod paper_book;
pub mod paper_column;
pub mod paper_column_engraver;
pub mod paper_score;
pub mod parenthesis_engraver;
pub mod part_combine_engraver;
pub mod percent_repeat;
pub mod percent_repeat_engraver;
pub mod phrasing_slur_engraver;
pub mod piano_pedal;
pub mod piano_pedal_engraver;
pub mod pitch_squash_engraver;
pub mod pitched_trill_engraver;
mod pure_from_neighbor;
pub mod pure_from_neighbor_engraver;
pub mod queue;
mod rest;
pub mod rest_collision;
pub mod rest_collision_engraver;
pub mod rest_engraver;
pub mod rhythmic_column_engraver;
pub mod rhythmic_head;
pub mod rod;
pub mod score_engraver;
pub mod script;
pub mod script_column;
pub mod script_column_engraver;
pub mod script_engraver;
pub mod script_row_engraver;
mod self_alignment;
pub mod semi_tie;
pub mod semi_tie_column;
pub mod separating_line_group_engraver;
pub mod separation_item;
pub mod side_position;
pub mod simple_spacer;
pub mod skylines;
mod slots;
pub mod slur;
pub mod slur_configuration;
pub mod slur_engraver;
pub mod slur_scoring;
pub mod spaceable;
pub mod spacing_basic;
pub mod spacing_determine_loose_columns;
pub mod spacing_engraver;
pub mod spacing_interface;
pub mod spacing_loose_columns;
pub mod spacing_options;
pub mod spacing_spanner;
pub mod spacing_spec;
pub mod span_arpeggio_engraver;
pub mod span_bar_engraver;
pub mod span_bar_stub_engraver;
pub mod spanner;
pub mod spanner_break_forbid_engraver;
pub mod spring;
pub mod staff_collecting_engraver;
pub mod staff_grouper;
pub mod staff_spacing;
mod staff_symbol;
pub mod staff_symbol_engraver;
mod staff_symbol_referencer;
pub mod stanza_number_engraver;
pub mod stem;
pub mod stem_engraver;
pub mod stem_tremolo;
mod stencil;
pub mod system;
pub mod system_start_delimiter;
pub mod system_start_delimiter_engraver;
mod text;
pub mod text_engraver;
pub mod text_spanner_engraver;
pub mod tie;
pub mod tie_column;
pub mod tie_configuration;
pub mod tie_engraver;
pub mod tie_formatting_problem;
pub mod time_signature_engraver;
pub mod tuplet_bracket;
pub mod tuplet_engraver;
pub mod tuplet_number;
pub mod tweak_engraver;
pub mod unpure;
pub mod vaticana_ligature;
pub mod vertical_align_engraver;
pub mod volta_bracket;
pub mod volta_engraver;
pub mod vowel_transition;

use alloc::boxed::Box as Owned;

use lily_engine_scheme::{
    Cx, Exception, Extension, Heap, HostHandler, Loader, Prim, REST, Thrown, Value, Vm,
};

pub use font::OpenTypeFont;
pub use geometry::{
    Axis, BIGPOINT, Box, CM, Direction, INCH, Interval, MM, Offset, POINT, Transform,
    TransformObject, X, Y, angle_degrees, complex_multiply, direction, length, offset_directed,
    other_axis,
};
pub use line::{LineStyle, Style};
pub use lookup::Bezier;
pub use stencil::Stencil;

/// What the notation crates beside layout (D223) define their callbacks
/// with, as layout's own are (D127): a subr over `%host-call`, its
/// handler's dispatch, and `Text_interface::is_markup`.
pub use callbacks::{Callback, dispatch};
pub use text::is_markup;
pub use tie::host_call_of;

type R = Result<Value, Thrown>;

/// Layout's translator classes, which a `\layout` translation is run with
/// (`lily_engine_music::run_translator_with`, D108): `Score_engraver`'s
/// layout part, `Tie_engraver` and, since MS3 step 22, 35 of part 2's 36,
/// each found before the music crate's placeholder of its name (the 36th,
/// `Mark_tracking_translator`, is the music crate's own); since MS3 step
/// 41 `Collision_engraver` and `Rest_collision_engraver` too, the 36th
/// and 37th placeholders shadowed, and since step 42
/// `Multi_measure_rest_engraver`, `Span_bar_engraver` and
/// `Span_bar_stub_engraver`, the 38th to 40th, since step 43
/// `Instrument_name_engraver`, the 41st, since step 48
/// `Font_size_engraver` and `Staff_collecting_engraver`, the 42nd and
/// 43rd, since step 49 `Horizontal_bracket_engraver`, the 44th, since
/// step 52 `Text_spanner_engraver` and `Note_head_line_engraver`, the 45th
/// and 46th, since step 53 `Script_engraver`, `Script_column_engraver`,
/// `Fingering_engraver`, `New_fingering_engraver` and
/// `Fingering_column_engraver`, the 47th to 51st, and since step 54
/// `Arpeggio_engraver`, `Span_arpeggio_engraver` and
/// `Metronome_mark_engraver`, the 52nd to 54th. Since step 55 (D213),
/// `Apply_output_engraver` too, found before the image's Scheme engraver
/// of its name.
pub static CLASSES: &[&lily_engine_music::TranslatorClass] = &[
    &score_engraver::SCORE_ENGRAVER,
    &paper_column_engraver::PAPER_COLUMN_ENGRAVER,
    &note_heads_engraver::NOTE_HEADS_ENGRAVER,
    &rest_engraver::REST_ENGRAVER,
    &stem_engraver::STEM_ENGRAVER,
    &dots_engraver::DOTS_ENGRAVER,
    &dot_column_engraver::DOT_COLUMN_ENGRAVER,
    &collision_engraver::COLLISION_ENGRAVER,
    &rest_collision_engraver::REST_COLLISION_ENGRAVER,
    &multi_measure_rest_engraver::MULTI_MEASURE_REST_ENGRAVER,
    &span_bar_engraver::SPAN_BAR_ENGRAVER,
    &span_bar_stub_engraver::SPAN_BAR_STUB_ENGRAVER,
    &instrument_name_engraver::INSTRUMENT_NAME_ENGRAVER,
    &rhythmic_column_engraver::RHYTHMIC_COLUMN_ENGRAVER,
    &ledger_line_engraver::LEDGER_LINE_ENGRAVER,
    &clef_engraver::CLEF_ENGRAVER,
    &key_engraver::KEY_ENGRAVER,
    &time_signature_engraver::TIME_SIGNATURE_ENGRAVER,
    &accidental_engraver::ACCIDENTAL_ENGRAVER,
    &tie_engraver::TIE_ENGRAVER,
    &laissez_vibrer_engraver::LAISSEZ_VIBRER_ENGRAVER,
    &laissez_vibrer_engraver::REPEAT_TIE_ENGRAVER,
    &pitch_squash_engraver::PITCH_SQUASH_ENGRAVER,
    &tuplet_engraver::TUPLET_ENGRAVER,
    &drum_note_engraver::DRUM_NOTES_ENGRAVER,
    &lyric_engraver::LYRIC_ENGRAVER,
    &extender_engraver::EXTENDER_ENGRAVER,
    &hyphen_engraver::HYPHEN_ENGRAVER,
    &stanza_number_engraver::STANZA_NUMBER_ENGRAVER,
    &stanza_number_engraver::STANZA_NUMBER_ALIGN_ENGRAVER,
    &staff_symbol_engraver::STAFF_SYMBOL_ENGRAVER,
    &bar_engraver::BAR_ENGRAVER,
    &bar_number_engraver::BAR_NUMBER_ENGRAVER,
    &system_start_delimiter_engraver::SYSTEM_START_DELIMITER_ENGRAVER,
    &text_engraver::TEXT_ENGRAVER,
    &script_row_engraver::SCRIPT_ROW_ENGRAVER,
    &dynamic_engraver::DYNAMIC_ENGRAVER,
    &dynamic_align_engraver::DYNAMIC_ALIGN_ENGRAVER,
    &volta_engraver::VOLTA_ENGRAVER,
    &ottava_engraver::OTTAVA_SPANNER_ENGRAVER,
    &horizontal_bracket_engraver::HORIZONTAL_BRACKET_ENGRAVER,
    &text_spanner_engraver::TEXT_SPANNER_ENGRAVER,
    &note_head_line_engraver::NOTE_HEAD_LINE_ENGRAVER,
    &script_engraver::SCRIPT_ENGRAVER,
    &script_column_engraver::SCRIPT_COLUMN_ENGRAVER,
    &fingering_engraver::FINGERING_ENGRAVER,
    &new_fingering_engraver::NEW_FINGERING_ENGRAVER,
    &fingering_column_engraver::FINGERING_COLUMN_ENGRAVER,
    &arpeggio_engraver::ARPEGGIO_ENGRAVER,
    &span_arpeggio_engraver::SPAN_ARPEGGIO_ENGRAVER,
    &metronome_engraver::METRONOME_MARK_ENGRAVER,
    &mark_engraver::MARK_ENGRAVER,
    &jump_engraver::JUMP_ENGRAVER,
    &percent_repeat_engraver::PERCENT_REPEAT_ENGRAVER,
    &percent_repeat_engraver::DOUBLE_PERCENT_REPEAT_ENGRAVER,
    &percent_repeat_engraver::SLASH_REPEAT_ENGRAVER,
    &caesura_engraver::CAESURA_ENGRAVER,
    &cue_clef_engraver::CUE_CLEF_ENGRAVER,
    &instrument_switch_engraver::INSTRUMENT_SWITCH_ENGRAVER,
    &part_combine_engraver::PART_COMBINE_ENGRAVER,
    &parenthesis_engraver::PARENTHESIS_ENGRAVER,
    &note_name_engraver::NOTE_NAME_ENGRAVER,
    &melody_engraver::MELODY_ENGRAVER,
    &piano_pedal_engraver::PIANO_PEDAL_ENGRAVER,
    &piano_pedal_engraver::PIANO_PEDAL_ALIGN_ENGRAVER,
    &pitched_trill_engraver::PITCHED_TRILL_ENGRAVER,
    &grid::GRID_POINT_ENGRAVER,
    &grid::GRID_LINE_SPAN_ENGRAVER,
    &completion::COMPLETION_HEADS_ENGRAVER,
    &completion::COMPLETION_REST_ENGRAVER,
    &ambitus_engraver::AMBITUS_ENGRAVER,
    &cluster::CLUSTER_SPANNER_ENGRAVER,
    &balloon::BALLOON_ENGRAVER,
    &balloon::FOOTNOTE_ENGRAVER,
    &ligature::LIGATURE_BRACKET_ENGRAVER,
    &mensural_ligature::MENSURAL_LIGATURE_ENGRAVER,
    &episema_engraver::EPISEMA_ENGRAVER,
    &vaticana_ligature::VATICANA_LIGATURE_ENGRAVER,
    &kievan_ligature::KIEVAN_LIGATURE_ENGRAVER,
    &page_turn_engraver::PAGE_TURN_ENGRAVER,
    &beam_engraver::BEAM_ENGRAVER,
    &beam_engraver::GRACE_BEAM_ENGRAVER,
    &slur_engraver::SLUR_ENGRAVER,
    &phrasing_slur_engraver::PHRASING_SLUR_ENGRAVER,
    &auto_beam_engraver::AUTO_BEAM_ENGRAVER,
    &auto_beam_engraver::GRACE_AUTO_BEAM_ENGRAVER,
    &beam_collision_engraver::BEAM_COLLISION_ENGRAVER,
    &tweak_engraver::TWEAK_ENGRAVER,
    &chord_tremolo_engraver::CHORD_TREMOLO_ENGRAVER,
    &axis_group_engraver::AXIS_GROUP_ENGRAVER,
    &vertical_align_engraver::VERTICAL_ALIGN_ENGRAVER,
    &break_align_engraver::BREAK_ALIGN_ENGRAVER,
    &pure_from_neighbor_engraver::PURE_FROM_NEIGHBOR_ENGRAVER,
    &keep_alive_together_engraver::KEEP_ALIVE_TOGETHER_ENGRAVER,
    &grob_pq_engraver::GROB_PQ_ENGRAVER,
    &forbid_line_break_engraver::FORBID_LINE_BREAK_ENGRAVER,
    &font_size_engraver::FONT_SIZE_ENGRAVER,
    &separating_line_group_engraver::SEPARATING_LINE_GROUP_ENGRAVER,
    &spanner_break_forbid_engraver::SPANNER_BREAK_FORBID_ENGRAVER,
    &staff_collecting_engraver::STAFF_COLLECTING_ENGRAVER,
    &spacing_engraver::SPACING_ENGRAVER,
    &note_spacing_engraver::NOTE_SPACING_ENGRAVER,
    &grace_spacing_engraver::GRACE_SPACING_ENGRAVER,
    &non_musical_script_column_engraver::NON_MUSICAL_SCRIPT_COLUMN_ENGRAVER,
    &apply_output_engraver::APPLY_OUTPUT_ENGRAVER,
];

/// Defines the `ly:` functions of stencils and transforms in `vm`'s
/// `(lily)`. The music crate's [`lily_engine_music::install`] comes first.
pub fn install(vm: &mut Vm) -> Result<(), Exception> {
    vm.define_primitives("lily", PRIMS, PRELUDE)?;
    vm.define_primitives("lily", &[], text::PRELUDE)?;
    vm.define_primitives("lily", &[], grob::PRELUDE)?;
    vm.define_primitives("lily", &[], grob_array::PRELUDE)?;
    vm.define_primitives("lily", &[], align::PRELUDE)?;
    vm.define_primitives("lily", &[], unpure::PRELUDE)?;
    vm.define_primitives("lily", &[], spanner::PRELUDE)?;
    vm.define_primitives("lily", &[], rest::PRELUDE)?;
    vm.define_primitives("lily", &[], axis_group::PRELUDE)?;
    vm.define_primitives("lily", &[], hara_kiri_group_spanner::PRELUDE)?;
    vm.define_primitives("lily", &[], staff_grouper::PRELUDE)?;
    vm.define_primitives("lily", &[], spacing_spec::PRELUDE)?;
    vm.define_primitives("lily", &[], page_layout_problem::PRELUDE)?;
    vm.define_primitives("lily", &[], page::PRELUDE)?;
    vm.define_primitives("lily", &[], optimal_page_breaking::PRELUDE)?;
    vm.define_primitives("lily", &[], page_breakers::PRELUDE)?;
    vm.define_primitives("lily", &[], paper_book::PRELUDE)?;
    vm.define_primitives("lily", &[], skylines::PRELUDE)?;
    vm.define_primitives("lily", &[], break_alignment::PRELUDE)?;
    vm.define_primitives("lily", &[], spacing_spanner::PRELUDE)?;
    vm.define_primitives("lily", &[], paper_score::PRELUDE)?;
    vm.define_primitives("lily", &[], system::PRELUDE)?;
    vm.add_host_handlers(&HOSTS);
    vm.define_primitives("lily", &[], paper_column::PRELUDE)?;
    vm.define_primitives("lily", &[], staff_symbol::PRELUDE)?;
    vm.define_primitives("lily", &[], staff_symbol_referencer::PRELUDE)?;
    vm.define_primitives("lily", &[], note_head::PRELUDE)?;
    vm.define_primitives("lily", &[], note_column::PRELUDE)?;
    vm.define_primitives("lily", &[], dot_column::PRELUDE)?;
    vm.define_primitives("lily", &[], self_alignment::PRELUDE)?;
    vm.define_primitives("lily", &[], pure_from_neighbor::PRELUDE)?;
    vm.define_primitives("lily", &[], dump::PRELUDE)?;
    vm.define_primitives("lily", &[], font_select::PRELUDE)
}

/// What `%host-call` runs for layout's callbacks (D127): the spacing
/// spanner's, the Paper_score's, a spanner's rods, the dump's, the
/// system's sort, the note column's main extent, the dot column's
/// positioning, the staff symbol's lines and ledgers, the stem's
/// callbacks, the accidental's, the accidental placement's, the ledger
/// line spanner's, the side positions', the clef's, the key signature's,
/// the system start delimiter's, the flag's, the note collision's, the
/// rest collision's two, the multi-measure rest's and the clef modifier's;
/// since MS3 step 46 grob.cc's and the staff symbol referencer's, since
/// step 48 the breathing sign's, since step 49 the hairpin's and the
/// volta, ottava and horizontal brackets', since step 52 the line
/// spanners' and `ly:line-interface::line`'s, since step 53 the script's,
/// the script column's and the fingering column's, and since step 54 the
/// arpeggio's, the chord bracket's and the chord slur's, since step 59
/// the separation item's, since step 62 the page breaker's (since MS8
/// step 9's part 3 the other five breakers too) and the
/// `Paper_book`'s pages and systems, since MS4 step 1 the pure
/// coordinates and heights and the axis groups' pure heights, since
/// step 7 the slur's, since step 9 the tie column's, since step 10 the
/// tie's, the semi-tie's and the semi-tie column's, since step 12 the
/// tuplet bracket's, since step 13 the tuplet number's, and since step 15
/// the lyric extender's and the lyric hyphen's.
static HOSTS: [HostHandler; 86] = {
    let [a, b] = spacing_spanner::HOSTS;
    let [c, d, e, sef] = paper_score::HOSTS;
    let [f, g] = dump::HOSTS;
    let [i] = spanner::HOSTS;
    let [j] = system::HOSTS;
    let [k] = note_column::HOSTS;
    let [l] = dot_column::HOSTS;
    let [m] = staff_symbol::HOSTS;
    let [n] = stem::HOSTS;
    let [o] = accidental::HOSTS;
    let [q] = accidental_placement::HOSTS;
    let [r] = ledger_line_spanner::HOSTS;
    let [s] = side_position::HOSTS;
    let [t] = clef::HOSTS;
    let [u] = key_signature::HOSTS;
    let [v] = system_start_delimiter::HOSTS;
    let [w] = flag::HOSTS;
    let [x] = note_collision::HOSTS;
    let [y, z] = rest_collision::HOSTS;
    let [mmr] = multi_measure_rest::HOSTS;
    let [cm] = clef_modifier::HOSTS;
    let [gr, gw] = grob::HOSTS;
    let [ssr] = staff_symbol_referencer::HOSTS;
    let [bs] = breathing_sign::HOSTS;
    let [hp] = hairpin::HOSTS;
    let [vb] = volta_bracket::HOSTS;
    let [ob] = ottava_bracket::HOSTS;
    let [hb] = horizontal_bracket::HOSTS;
    let [ls, hls] = line_spanner::HOSTS;
    let [li] = line::HOSTS;
    let [si] = script::HOSTS;
    let [sc] = script_column::HOSTS;
    let [fc] = fingering_column::HOSTS;
    let [ar, cb, cs] = arpeggio::HOSTS;
    let [sep] = separation_item::HOSTS;
    let [opb] = optimal_page_breaking::HOSTS;
    let [pbp, pbs] = paper_book::HOSTS;
    let [up] = unpure::HOSTS;
    let [ag] = axis_group::HOSTS;
    let [bm, bq, brc, bprc] = beam::HOSTS;
    let [stt] = stem_tremolo::HOSTS;
    let [sl, slo, slpo, slcs] = slur::HOSTS;
    let [tc] = tie_column::HOSTS;
    let [ti] = tie::HOSTS;
    let [st] = semi_tie::HOSTS;
    let [stc] = semi_tie_column::HOSTS;
    let [tb] = tuplet_bracket::HOSTS;
    let [tn] = tuplet_number::HOSTS;
    let [lex] = lyric_extender::HOSTS;
    let [lhy] = lyric_hyphen::HOSTS;
    let [prp] = percent_repeat::HOSTS;
    let [vtr] = vowel_transition::HOSTS;
    let [mel] = melody_engraver::HOSTS;
    let [ppb, sus] = piano_pedal::HOSTS;
    let [gli] = grid::HOSTS;
    let [mgr, msp] = measure::HOSTS;
    let [clu, clb] = cluster::HOSTS;
    let [bal] = balloon::HOSTS;
    let [mlg] = mensural_ligature::HOSTS;
    let [vlg] = vaticana_ligature::HOSTS;
    let [klg] = kievan_ligature::HOSTS;
    let [ptb, mnb, opg, oln, olh] = page_breakers::HOSTS;
    [
        a, b, c, d, e, f, g, i, j, k, l, m, n, o, q, r, s, t, u, v, w, x, y, z, mmr, cm, sef, gr,
        gw, ssr, bs, hp, vb, ob, hb, ls, hls, li, si, sc, fc, ar, cb, cs, sep, opb, pbp, pbs, up,
        ag, bm, bq, brc, bprc, stt, sl, slo, slpo, slcs, tc, ti, st, stc, tb, tn, lex, lhy, prp,
        vtr, mel, ppb, sus, gli, mgr, msp, clu, clb, bal, mlg, vlg, klg, ptb, mnb, opg, oln, olh,
    ]
};

/// What [`lily_engine_scheme::Vm::restore`] needs of a VM that [`install`]
/// was given, after the music crate's.
pub static EXTENSION: Extension = Extension {
    prims: &[PRIMS],
    operators: None,
    hosts: &HOSTS,
    loaders: &[
        Loader {
            name: "Stencil",
            load: stencil::load,
        },
        Loader {
            name: "Transform",
            load: |r| Some(Owned::new(TransformObject::load(r)?)),
        },
        Loader {
            name: "All_font_metrics",
            load: font::load_all,
        },
    ],
};

const fn p(name: &'static str, min: u8, max: u8, f: lily_engine_scheme::PrimFn) -> Prim {
    Prim { name, min, max, f }
}

static PRIMS: &[Prim] = &[
    // accidental.cc's and accidental-placement.cc's callbacks, over
    // %host-call
    p(
        "ly:accidental-interface::height",
        1,
        1,
        accidental::ly_height,
    ),
    p(
        "ly:accidental-interface::horizontal-skylines",
        1,
        1,
        accidental::ly_horizontal_skylines,
    ),
    p("ly:accidental-interface::print", 1, 1, accidental::ly_print),
    p(
        "ly:accidental-interface::remove-tied",
        1,
        1,
        accidental::ly_remove_tied,
    ),
    p(
        "ly:accidental-placement::calc-positioning-done",
        1,
        1,
        accidental_placement::ly_calc_positioning_done,
    ),
    // bezier-scheme.cc's ly:bezier-extent (ly:bezier-extract is the music
    // crate's)
    p("ly:bezier-extent", 2, 2, bezier::ly_bezier_extent),
    // arpeggio.cc's callbacks, over %host-call
    p(
        "ly:arpeggio::calc-cross-staff",
        1,
        1,
        arpeggio::ly_calc_cross_staff,
    ),
    p(
        "ly:arpeggio::calc-positions",
        1,
        1,
        arpeggio::ly_calc_positions,
    ),
    p("ly:arpeggio::print", 1, 1, arpeggio::ly_print),
    p("ly:arpeggio::pure-height", 3, 3, arpeggio::ly_pure_height),
    p("ly:arpeggio::width", 1, 1, arpeggio::ly_width),
    p("ly:chord-bracket::print", 1, 1, arpeggio::ly_bracket_print),
    p("ly:chord-bracket::width", 1, 1, arpeggio::ly_bracket_width),
    p("ly:chord-slur::print", 1, 1, arpeggio::ly_slur_print),
    p("ly:chord-slur::width", 1, 1, arpeggio::ly_slur_width),
    // beam.cc's callbacks and rest offsets, over %host-call
    p(
        "ly:beam::calc-beam-segments",
        1,
        1,
        beam::ly_calc_beam_segments,
    ),
    p("ly:beam::calc-beaming", 1, 1, beam::ly_calc_beaming),
    p("ly:beam::calc-cross-staff", 1, 1, beam::ly_calc_cross_staff),
    p("ly:beam::calc-direction", 1, 1, beam::ly_calc_direction),
    p(
        "ly:beam::calc-normal-stems",
        1,
        1,
        beam::ly_calc_normal_stems,
    ),
    p(
        "ly:beam::calc-stem-shorten",
        1,
        1,
        beam::ly_calc_stem_shorten,
    ),
    p("ly:beam::calc-x-positions", 1, 1, beam::ly_calc_x_positions),
    p("ly:beam::print", 1, 1, beam::ly_print),
    p("ly:beam::quanting", 3, 3, beam::ly_quanting),
    p(
        "ly:beam::pure-rest-collision-callback",
        3,
        4,
        beam::ly_pure_rest_collision_callback,
    ),
    p(
        "ly:beam::rest-collision-callback",
        1,
        2,
        beam::ly_rest_collision_callback,
    ),
    p("ly:beam::set-stem-lengths", 1, 1, beam::ly_set_stem_lengths),
    p(
        "ly:beam::tremolo-springs-and-rods",
        1,
        1,
        beam::ly_tremolo_springs_and_rods,
    ),
    // clef.cc's callbacks, over %host-call
    p("ly:clef::calc-glyph-name", 1, 1, clef::ly_calc_glyph_name),
    p("ly:clef::print", 1, 1, clef::ly_print),
    // clef-modifier.cc's callback, over %host-call
    p(
        "ly:clef-modifier::calc-parent-alignment",
        1,
        1,
        clef_modifier::ly_calc_parent_alignment,
    ),
    // flag.cc's callbacks, over %host-call
    p("ly:flag::calc-x-offset", 1, 1, flag::ly_calc_x_offset),
    p("ly:flag::calc-y-offset", 1, 1, flag::ly_calc_y_offset),
    p("ly:flag::glyph-name", 1, 1, flag::ly_glyph_name),
    p("ly:flag::print", 1, 1, flag::ly_print),
    p(
        "ly:flag::pure-calc-y-offset",
        3,
        3,
        flag::ly_pure_calc_y_offset,
    ),
    p("ly:flag::width", 1, 1, flag::ly_width),
    // key-signature-interface.cc's callback, over %host-call
    p(
        "ly:key-signature-interface::print",
        1,
        1,
        key_signature::ly_print,
    ),
    // ledger-line-spanner.cc's callbacks, over %host-call
    p(
        "ly:ledger-line-spanner::print",
        1,
        1,
        ledger_line_spanner::ly_print,
    ),
    p(
        "ly:ledger-line-spanner::set-spacing-rods",
        1,
        1,
        ledger_line_spanner::ly_set_spacing_rods,
    ),
    // multi-measure-rest.cc's callbacks, over %host-call
    p(
        "ly:multi-measure-rest::height",
        1,
        1,
        multi_measure_rest::ly_height,
    ),
    p(
        "ly:multi-measure-rest::print",
        1,
        1,
        multi_measure_rest::ly_print,
    ),
    p(
        "ly:multi-measure-rest::set-spacing-rods",
        1,
        1,
        multi_measure_rest::ly_set_spacing_rods,
    ),
    p(
        "ly:multi-measure-rest::set-text-rods",
        1,
        1,
        multi_measure_rest::ly_set_text_rods,
    ),
    // note-collision.cc's callback, over %host-call
    p(
        "ly:note-collision-interface::calc-positioning-done",
        1,
        1,
        note_collision::ly_calc_positioning_done,
    ),
    // rest-collision.cc's callbacks, over %host-call
    p(
        "ly:rest-collision::calc-positioning-done",
        1,
        1,
        rest_collision::ly_calc_positioning_done,
    ),
    p(
        "ly:rest-collision::force-shift-callback-rest",
        1,
        2,
        rest_collision::ly_force_shift_callback_rest,
    ),
    // slur.cc's callbacks, over %host-call
    p(
        "ly:slur::calc-control-points",
        1,
        1,
        slur::ly_calc_control_points,
    ),
    p("ly:slur::calc-cross-staff", 1, 1, slur::ly_calc_cross_staff),
    p("ly:slur::calc-direction", 1, 1, slur::ly_calc_direction),
    p("ly:slur::height", 1, 1, slur::ly_height),
    p(
        "ly:slur::outside-slur-callback",
        1,
        2,
        slur::ly_outside_slur_callback,
    ),
    p(
        "ly:slur::outside-slur-cross-staff",
        2,
        2,
        slur::ly_outside_slur_cross_staff,
    ),
    p("ly:slur::print", 1, 1, slur::ly_print),
    p("ly:slur::pure-height", 3, 3, slur::ly_pure_height),
    p(
        "ly:slur::pure-outside-slur-callback",
        3,
        4,
        slur::ly_pure_outside_slur_callback,
    ),
    // semi-tie.cc's and semi-tie-column.cc's callbacks, over %host-call
    p(
        "ly:semi-tie-column::calc-head-direction",
        1,
        1,
        semi_tie_column::ly_calc_head_direction,
    ),
    p(
        "ly:semi-tie-column::calc-positioning-done",
        1,
        1,
        semi_tie_column::ly_calc_positioning_done,
    ),
    p(
        "ly:semi-tie::calc-control-points",
        1,
        1,
        tie::ly_semi_calc_control_points,
    ),
    // stem-tremolo.cc's callbacks, over %host-call
    p(
        "ly:stem-tremolo::calc-cross-staff",
        1,
        1,
        stem_tremolo::ly_calc_cross_staff,
    ),
    p(
        "ly:stem-tremolo::calc-direction",
        1,
        1,
        stem_tremolo::ly_calc_direction,
    ),
    p(
        "ly:stem-tremolo::calc-shape",
        1,
        1,
        stem_tremolo::ly_calc_shape,
    ),
    p(
        "ly:stem-tremolo::calc-slope",
        1,
        1,
        stem_tremolo::ly_calc_slope,
    ),
    p(
        "ly:stem-tremolo::calc-width",
        1,
        1,
        stem_tremolo::ly_calc_width,
    ),
    p(
        "ly:stem-tremolo::calc-y-offset",
        1,
        1,
        stem_tremolo::ly_calc_y_offset,
    ),
    p("ly:stem-tremolo::print", 1, 1, stem_tremolo::ly_print),
    p(
        "ly:stem-tremolo::pure-calc-y-offset",
        3,
        3,
        stem_tremolo::ly_pure_calc_y_offset,
    ),
    p(
        "ly:stem-tremolo::pure-height",
        3,
        3,
        stem_tremolo::ly_pure_height,
    ),
    p("ly:stem-tremolo::width", 1, 1, stem_tremolo::ly_width),
    // tie.cc's and tie-column.cc's callbacks, over %host-call
    p(
        "ly:tie-column::before-line-breaking",
        1,
        1,
        tie::ly_column_before_line_breaking,
    ),
    p(
        "ly:tie-column::calc-positioning-done",
        1,
        1,
        tie::ly_column_calc_positioning_done,
    ),
    p(
        "ly:tie::calc-control-points",
        1,
        1,
        tie::ly_calc_control_points,
    ),
    p("ly:tie::calc-direction", 1, 1, tie::ly_calc_direction),
    p("ly:tie::print", 1, 1, tie::ly_print),
    // tuplet-bracket.cc's callbacks, over %host-call
    p(
        "ly:tuplet-bracket::calc-cross-staff",
        1,
        1,
        tuplet_bracket::ly_calc_cross_staff,
    ),
    p(
        "ly:tuplet-bracket::calc-direction",
        1,
        1,
        tuplet_bracket::ly_calc_direction,
    ),
    p(
        "ly:tuplet-bracket::calc-positions",
        1,
        1,
        tuplet_bracket::ly_calc_positions,
    ),
    p(
        "ly:tuplet-bracket::calc-x-positions",
        1,
        1,
        tuplet_bracket::ly_calc_x_positions,
    ),
    p("ly:tuplet-bracket::print", 1, 1, tuplet_bracket::ly_print),
    // tuplet-number.cc's callbacks, over %host-call
    p(
        "ly:tuplet-number::calc-x-offset",
        1,
        1,
        tuplet_number::ly_calc_x_offset,
    ),
    p(
        "ly:tuplet-number::calc-y-offset",
        1,
        1,
        tuplet_number::ly_calc_y_offset,
    ),
    p("ly:tuplet-number::print", 1, 1, tuplet_number::ly_print),
    // lyric-extender.cc's and lyric-hyphen.cc's callbacks, over %host-call
    p("ly:lyric-extender::print", 1, 1, lyric_extender::ly_print),
    p("ly:lyric-hyphen::print", 1, 1, lyric_hyphen::ly_print),
    p(
        "ly:lyric-hyphen::set-spacing-rods",
        1,
        1,
        lyric_hyphen::ly_set_spacing_rods,
    ),
    // grid-line-interface.cc's, measure-grouping-spanner.cc's and
    // measure-spanner.cc's callbacks, over %host-call
    p("ly:balloon-interface::print", 1, 1, balloon::ly_print),
    p(
        "ly:balloon-interface::pure-height",
        3,
        3,
        balloon::ly_pure_height,
    ),
    p("ly:balloon-interface::width", 1, 1, balloon::ly_width),
    p(
        "ly:cluster::calc-cross-staff",
        1,
        1,
        cluster::ly_calc_cross_staff,
    ),
    p("ly:cluster::print", 1, 1, cluster::ly_print),
    p("ly:cluster-beacon::height", 1, 1, cluster::ly_beacon_height),
    p("ly:grid-line-interface::print", 1, 1, grid::ly_print),
    p("ly:grid-line-interface::width", 1, 1, grid::ly_width),
    p(
        "ly:measure-grouping::print",
        1,
        1,
        measure::ly_grouping_print,
    ),
    p("ly:measure-spanner::print", 1, 1, measure::ly_spanner_print),
    // mensural-ligature.cc's callbacks, over %host-call
    p(
        "ly:mensural-ligature::brew-ligature-primitive",
        1,
        1,
        mensural_ligature::ly_brew_ligature_primitive,
    ),
    p(
        "ly:mensural-ligature::print",
        1,
        1,
        mensural_ligature::ly_print,
    ),
    // vaticana-ligature.cc's and kievan-ligature.cc's callbacks, over
    // %host-call
    p(
        "ly:vaticana-ligature::brew-ligature-primitive",
        1,
        1,
        vaticana_ligature::ly_brew_ligature_primitive,
    ),
    p(
        "ly:vaticana-ligature::print",
        1,
        1,
        vaticana_ligature::ly_print,
    ),
    p("ly:kievan-ligature::print", 1, 1, kievan_ligature::ly_print),
    // piano-pedal-bracket.cc's and sustain-pedal.cc's stencils, over %host-call
    p(
        "ly:piano-pedal-bracket::print",
        1,
        1,
        piano_pedal::ly_bracket_print,
    ),
    p(
        "ly:sustain-pedal::print",
        1,
        1,
        piano_pedal::ly_sustain_print,
    ),
    // melody-spanner.cc's callback, over %host-call
    p(
        "ly:melody-spanner::calc-neutral-stem-direction",
        1,
        1,
        melody_engraver::ly_calc_neutral_stem_direction,
    ),
    // vowel-transition.cc's rods, over %host-call
    p(
        "ly:vowel-transition::set-spacing-rods",
        1,
        1,
        vowel_transition::ly_set_spacing_rods,
    ),
    // separation-item.cc's skylines, over %host-call
    p(
        "ly:separation-item::calc-skylines",
        1,
        1,
        separation_item::ly_calc_skylines,
    ),
    // percent-repeat-interface.cc's callbacks, over %host-call
    p(
        "ly:percent-repeat-interface::beat-slash",
        1,
        1,
        percent_repeat::ly_beat_slash,
    ),
    p(
        "ly:percent-repeat-interface::double-percent",
        1,
        1,
        percent_repeat::ly_double_percent,
    ),
    p(
        "ly:percent-repeat-interface::percent",
        1,
        1,
        percent_repeat::ly_percent,
    ),
    // breathing-sign.cc's offset callback and divisiones, over %host-call
    p(
        "ly:breathing-sign::offset-callback",
        1,
        1,
        breathing_sign::ly_offset_callback,
    ),
    p(
        "ly:breathing-sign::divisio-minima",
        1,
        1,
        breathing_sign::ly_divisio_minima,
    ),
    p(
        "ly:breathing-sign::divisio-maior",
        1,
        1,
        breathing_sign::ly_divisio_maior,
    ),
    p(
        "ly:breathing-sign::divisio-maxima",
        1,
        1,
        breathing_sign::ly_divisio_maxima,
    ),
    p(
        "ly:breathing-sign::finalis",
        1,
        1,
        breathing_sign::ly_finalis,
    ),
    // hairpin.cc's callbacks, over %host-call
    p(
        "ly:hairpin::broken-bound-padding",
        1,
        1,
        hairpin::ly_broken_bound_padding,
    ),
    p("ly:hairpin::print", 1, 1, hairpin::ly_print),
    p("ly:hairpin::pure-height", 3, 3, hairpin::ly_pure_height),
    // line-spanner.cc's callbacks, over %host-call
    p(
        "ly:horizontal-line-spanner::calc-left-bound-info",
        1,
        1,
        line_spanner::ly_horizontal_calc_left_bound_info,
    ),
    p(
        "ly:horizontal-line-spanner::calc-left-bound-info-and-text",
        1,
        1,
        line_spanner::ly_horizontal_calc_left_bound_info_and_text,
    ),
    p(
        "ly:horizontal-line-spanner::calc-right-bound-info",
        1,
        1,
        line_spanner::ly_horizontal_calc_right_bound_info,
    ),
    p(
        "ly:line-spanner::calc-cross-staff",
        1,
        1,
        line_spanner::ly_calc_cross_staff,
    ),
    p(
        "ly:line-spanner::calc-left-bound-info",
        1,
        1,
        line_spanner::ly_calc_left_bound_info,
    ),
    p(
        "ly:line-spanner::calc-left-bound-info-and-text",
        1,
        1,
        line_spanner::ly_calc_left_bound_info_and_text,
    ),
    p(
        "ly:line-spanner::calc-right-bound-info",
        1,
        1,
        line_spanner::ly_calc_right_bound_info,
    ),
    p("ly:line-spanner::print", 1, 1, line_spanner::ly_print),
    // horizontal-bracket.cc's, ottava-bracket.cc's and volta-bracket.cc's
    // prints, over %host-call
    p(
        "ly:horizontal-bracket::print",
        1,
        1,
        horizontal_bracket::ly_print,
    ),
    p("ly:ottava-bracket::print", 1, 1, ottava_bracket::ly_print),
    p(
        "ly:volta-bracket-interface::print",
        1,
        1,
        volta_bracket::ly_print,
    ),
    // fingering-column.cc's, script-interface.cc's and script-column.cc's
    // callbacks, over %host-call
    p(
        "ly:fingering-column::calc-positioning-done",
        1,
        1,
        fingering_column::ly_calc_positioning_done,
    ),
    p(
        "ly:script-column::before-line-breaking",
        1,
        1,
        script_column::ly_before_line_breaking,
    ),
    p(
        "ly:script-column::row-before-line-breaking",
        1,
        1,
        script_column::ly_row_before_line_breaking,
    ),
    p(
        "ly:script-interface::calc-cross-staff",
        1,
        1,
        script::ly_calc_cross_staff,
    ),
    p(
        "ly:script-interface::calc-positioning-done",
        1,
        1,
        script::ly_calc_positioning_done,
    ),
    // side-position-interface.cc's callbacks, over %host-call
    p(
        "ly:side-position-interface::calc-cross-staff",
        1,
        1,
        side_position::ly_calc_cross_staff,
    ),
    p(
        "ly:side-position-interface::move-to-extremal-staff",
        1,
        1,
        side_position::ly_move_to_extremal_staff,
    ),
    p(
        "ly:side-position-interface::pure-y-aligned-side",
        3,
        4,
        side_position::ly_pure_y_aligned_side,
    ),
    p(
        "ly:side-position-interface::set-axis!",
        2,
        2,
        side_position::ly_set_axis,
    ),
    p(
        "ly:side-position-interface::x-aligned-side",
        1,
        2,
        side_position::ly_x_aligned_side,
    ),
    p(
        "ly:side-position-interface::y-aligned-side",
        1,
        2,
        side_position::ly_y_aligned_side,
    ),
    // stem.cc's callbacks, over %host-call
    p("ly:stem::calc-cross-staff", 1, 1, stem::ly_calc_cross_staff),
    p(
        "ly:stem::calc-default-direction",
        1,
        1,
        stem::ly_calc_default_direction,
    ),
    p("ly:stem::calc-direction", 1, 1, stem::ly_calc_direction),
    p("ly:stem::calc-length", 1, 1, stem::ly_calc_length),
    p(
        "ly:stem::calc-positioning-done",
        1,
        1,
        stem::ly_calc_positioning_done,
    ),
    p(
        "ly:stem::calc-stem-begin-position",
        1,
        1,
        stem::ly_calc_stem_begin_position,
    ),
    p("ly:stem::calc-stem-info", 1, 1, stem::ly_calc_stem_info),
    p("ly:stem::height", 1, 1, stem::ly_height),
    p("ly:stem::offset-callback", 1, 1, stem::ly_offset_callback),
    p("ly:stem::print", 1, 1, stem::ly_print),
    p("ly:stem::pure-calc-length", 3, 3, stem::ly_pure_calc_length),
    p(
        "ly:stem::pure-calc-stem-begin-position",
        3,
        3,
        stem::ly_pure_calc_stem_begin_position,
    ),
    p("ly:stem::pure-height", 3, 3, stem::ly_pure_height),
    p("ly:stem::width", 1, 1, stem::ly_width),
    // system-start-delimiter.cc's callback, over %host-call
    p(
        "ly:system-start-delimiter::print",
        1,
        1,
        system_start_delimiter::ly_print,
    ),
    // stencil-scheme.cc
    p("ly:angle", 1, 2, stencil::angle),
    p("ly:length", 1, 2, stencil::ly_length),
    p("ly:directed", 1, 2, stencil::directed),
    p("ly:stencil-translate-axis", 3, 3, stencil::translate_axis),
    p("ly:stencil-translate", 2, 2, stencil::translate),
    p("ly:stencil-expr", 1, 1, stencil::expr),
    p("ly:stencil-extent", 2, 2, stencil::extent),
    p("ly:stencil-empty?", 1, 2, stencil::is_empty),
    p("ly:stencil-combine-at-edge", 4, 5, stencil::combine_at_edge),
    p("ly:stencil-stack", 4, 6, stencil::stack),
    p("ly:stencil-add", 0, REST, stencil::add),
    p("%make-stencil", 3, 3, stencil::make),
    p("ly:stencil-aligned-to", 3, 3, stencil::aligned_to),
    p("ly:stencil-rotate", 4, 4, stencil::rotate),
    p("ly:stencil-rotate-absolute", 4, 4, stencil::rotate_absolute),
    p("ly:stencil-scale", 2, 3, stencil::scale),
    p("ly:stencil-outline", 2, 2, stencil::outline),
    p("ly:stencil?", 1, 1, stencil::is_stencil),
    p("ly:round-filled-box", 3, 3, lookup::ly_round_filled_box),
    p("ly:round-polygon", 2, 4, lookup::ly_round_polygon),
    p("ly:bracket", 4, 4, lookup::ly_bracket),
    // line-interface-scheme.cc
    p(
        "ly:line-interface::line",
        5,
        5,
        lookup::ly_line_interface_line,
    ),
    // transform-scheme.cc
    p("ly:make-transform", 0, 6, geometry::make_transform),
    p("ly:make-scaling", 1, 2, geometry::make_scaling),
    p("ly:make-rotation", 1, 2, geometry::make_rotation),
    p("ly:make-translation", 1, 2, geometry::make_translation),
    p("ly:transform->list", 1, 1, geometry::transform_to_list),
    p("ly:transform?", 1, 1, geometry::is_transform),
    p("%transform-call", 2, 2, geometry::transform_call),
    // dimensions-scheme.cc
    p("ly:pt", 1, 1, geometry::ly_pt),
    p("ly:cm", 1, 1, geometry::ly_cm),
    p("ly:inch", 1, 1, geometry::ly_inch),
    p("ly:mm", 1, 1, geometry::ly_mm),
    p("ly:bp", 1, 1, geometry::ly_bp),
    // font-metric-scheme.cc
    p("ly:font-get-glyph", 2, 2, font::get_glyph),
    p(
        "ly:font-glyph-name-to-index",
        2,
        2,
        font::glyph_name_to_index,
    ),
    p("ly:font-index-to-charcode", 2, 2, font::index_to_charcode),
    p("ly:font-file-name", 1, 1, font::file_name),
    p("ly:font-name", 1, 1, font::name),
    p("ly:font-magnification", 1, 1, font::magnification),
    p("ly:font-design-size", 1, 1, font::design_size),
    p("ly:font-metric?", 1, 1, font::is_font_metric),
    // open-type-font-scheme.cc
    p("ly:otf-font-glyph-info", 2, 2, font::glyph_info),
    p("ly:otf-font-table-data", 2, 2, font::table_data),
    p("ly:otf-font?", 1, 1, font::is_otf_font),
    p("ly:otf-glyph-count", 1, 1, font::glyph_count),
    p("ly:otf-glyph-list", 1, 1, font::glyph_list),
    p("ly:get-font-format", 1, 2, font::get_font_format),
    p("ly:has-glyph-names?", 1, 2, font::has_glyph_names),
    p("ly:get-cff-offset", 1, 2, font::get_cff_offset),
    p(
        "ly:extract-subfont-from-collection",
        3,
        3,
        font::extract_subfont,
    ),
    // all-font-metrics-scheme.cc
    p("ly:reset-all-fonts", 0, 0, font::reset),
    p("ly:system-font-load", 1, 1, font::system_font_load),
    // output-def-scheme.cc
    p("ly:paper-get-font", 2, 2, font_select::paper_get_font),
    // paper-book-scheme.cc; ly:paper-book-pages and ::systems are Scheme
    // (paper_book.rs).
    p("ly:paper-book-paper", 1, 1, paper_book::ly_paper_book_paper),
    p(
        "ly:paper-book-header",
        1,
        1,
        paper_book::ly_paper_book_header,
    ),
    // page-marker-scheme.cc
    p(
        "ly:make-page-permission-marker",
        2,
        2,
        page_marker::make_permission_marker,
    ),
    p(
        "ly:make-page-label-marker",
        1,
        1,
        page_marker::make_label_marker,
    ),
    p("ly:page-marker?", 1, 1, page_marker::is_page_marker),
    // paper-system-scheme.cc
    p("ly:paper-system?", 1, 1, system::is_paper_system),
    p("%queue-check", 1, 1, queue::scheme_check),
    // music-output.cc
    p("ly:music-output?", 1, 1, paper_score::is_music_output),
    p(
        "ly:paper-book-scopes",
        1,
        1,
        paper_book::ly_paper_book_scopes,
    ),
    // pango-font-scheme.cc
    p("ly:pango-font?", 1, 1, pango::is_pango_font),
    p("ly:pango-font-physical-fonts", 1, 1, pango::physical_fonts),
    // text-interface.cc; ly:text-interface::interpret-markup, ::print and
    // ::grob-interpret-markup are Scheme (text.rs).
    p(
        "ly:perform-text-replacements",
        3,
        3,
        text::perform_text_replacements,
    ),
    p("%text-stencil", 4, 4, text::text_stencil),
    // grob-scheme.cc's property and object primitives (grob.rs); a
    // procedure-with-setter and ly:grob-set-property! are Scheme.
    p("ly:grob-property", 2, 3, grob::ly_grob_property),
    p(
        "%grob-set-property-data!",
        3,
        3,
        grob::set_property_data_prim,
    ),
    p("ly:grob-object", 2, 3, grob::ly_grob_object),
    p("ly:grob-set-object!", 3, 3, grob::ly_grob_set_object),
    // grob-scheme.cc's parents and refpoints, and what grob.rs' prelude
    // computes offsets and extents with.
    p("ly:grob-parent", 2, 3, grob::ly_grob_parent),
    p("ly:grob-set-parent!", 3, 3, grob::ly_grob_set_parent),
    p(
        "ly:grob-translate-axis!",
        3,
        3,
        grob::ly_grob_translate_axis,
    ),
    p(
        "ly:grob-common-refpoint",
        3,
        3,
        grob::ly_grob_common_refpoint,
    ),
    p("%grob-check-refp", 4, 4, grob::check_refp),
    p("%grob-offset-begin", 2, 2, grob::offset_begin),
    p("%grob-offset-end", 3, 3, grob::offset_end),
    p("%grob-cache-extent!", 4, 4, grob::cache_extent),
    p("%grob-enter", 3, 3, grob::grob_enter),
    p("%grob-leave", 2, 2, grob::grob_leave),
    p("%grob-extent-at", 3, 3, grob::extent_at),
    p("%grob-property-data", 2, 2, grob::property_data_prim),
    p("%host-waiting?", 0, 0, grob::host_waiting),
    p("%grob-live?", 1, 1, grob::is_live_prim),
    p("%grob-name", 1, 1, grob::name_prim),
    p("ly:grob-suicide!", 1, 1, grob::ly_grob_suicide),
    // The rest of grob-scheme.cc; ly:grob-set-nested-property!,
    // ly:grob-robust-relative-extent, ly:grob-chain-callback,
    // ly:grob-vertical<?, ly:grob-get-vertical-axis-group-index and
    // ly:grob-default-font are Scheme (grob.rs, font_select.rs).
    p("ly:grob-property-data", 2, 2, grob::ly_grob_property_data),
    p("ly:grob-interfaces", 1, 1, grob::ly_grob_interfaces),
    p("ly:grob-layout", 1, 1, grob::ly_grob_layout),
    p("ly:grob-alist-chain", 1, 2, grob::ly_grob_alist_chain),
    p(
        "ly:grob-basic-properties",
        1,
        1,
        grob::ly_grob_basic_properties,
    ),
    p("ly:grob-system", 1, 1, grob::ly_grob_system),
    p("ly:grob-original", 1, 1, grob::ly_grob_original),
    p(
        "ly:grob-common-refpoint-of-array",
        3,
        3,
        grob::ly_grob_common_refpoint_of_array,
    ),
    p(
        "ly:grob-spanned-column-rank-interval",
        1,
        1,
        grob::ly_grob_spanned_column_rank_interval,
    ),
    p(
        "%grob-root-vertical-alignment",
        1,
        1,
        grob::root_vertical_alignment_prim,
    ),
    p(
        "%grob-vertical-axis-group",
        1,
        1,
        grob::vertical_axis_group_prim,
    ),
    p(
        "%music-font-alist-chain",
        1,
        1,
        font_select::music_font_alist_chain_prim,
    ),
    // grob-scheme.cc's pure lookups, and what unpure.rs' prelude calls.
    p("ly:grob-pure-property", 4, 5, unpure::ly_grob_pure_property),
    p("ly:grob-pure-height", 4, 5, unpure::ly_grob_pure_height),
    p(
        "ly:grob-pure-relative-coordinate",
        4,
        4,
        unpure::ly_grob_pure_relative_coordinate,
    ),
    p("%grob-pure-offset-begin", 1, 1, unpure::pure_offset_begin),
    p("%grob-pure-offset-end", 1, 1, unpure::pure_offset_end),
    p("%grob-del-property!", 2, 2, unpure::del_property),
    p("%grob-align-child?", 1, 1, unpure::is_align_child),
    p("%grob-warning", 2, 2, rest::grob_warning),
    p(
        "%grob-programming-error",
        2,
        2,
        unpure::grob_programming_error,
    ),
    p("%pure-y-extent", 3, 3, unpure::pure_extent),
    p("%item-pure-height", 1, 1, unpure::item_pure_height),
    p("%align-translations-assoc", 3, 3, align::translations_assoc),
    p(
        "%item-cache-pure-height!",
        2,
        2,
        unpure::item_cache_pure_height,
    ),
    // item-scheme.cc
    p("ly:item?", 1, 1, item::ly_item_p),
    p("ly:item-break-dir", 1, 1, item::ly_item_break_dir),
    p("ly:item-get-column", 1, 1, item::ly_item_get_column),
    p("%item-prebroken-piece", 2, 2, item::prebroken_piece),
    // spanner-scheme.cc; ly:spanner-set-bound! and spanner.cc's callbacks
    // are Scheme (spanner.rs).
    p("ly:spanner-bound", 2, 3, spanner::ly_spanner_bound),
    p("%spanner-set-bound!", 3, 3, spanner::set_bound_prim),
    p(
        "ly:spanner-broken-into",
        1,
        1,
        spanner::ly_spanner_broken_into,
    ),
    p(
        "ly:spanner-broken-neighbor",
        2,
        2,
        spanner::ly_spanner_broken_neighbor,
    ),
    p("ly:spanner?", 1, 1, spanner::ly_spanner_p),
    p("%spanner-break-index", 1, 1, spanner::break_index),
    p("%zero-spanned-time?", 2, 2, spanner::zero_spanned_time),
    p(
        "%spanner-cached-pure-property",
        4,
        4,
        spanner::cached_pure_property,
    ),
    p(
        "%spanner-cache-pure-property!",
        5,
        5,
        spanner::cache_pure_property_prim,
    ),
    // grob-array-scheme.cc; ly:grob-array-filter and
    // ly:pointer-group-interface::add-grob are Scheme (grob_array.rs).
    p("ly:grob-array?", 1, 1, grob_array::is_grob_array),
    p("ly:grob-array-length", 1, 1, grob_array::length),
    p("ly:grob-array-ref", 2, 2, grob_array::grob_array_ref),
    p("ly:grob-array->list", 1, 1, grob_array::to_list),
    p("ly:grob-list->grob-array", 1, 1, grob_array::from_list),
    p("%grob-array-filtered", 2, 2, grob_array::filtered),
    p("%grob-array-add!", 2, 2, grob_array::add),
    p("%grob-array-set!", 2, 2, grob_array::set),
    // skyline.cc's smob, and what stencil-integral.cc's Grob:: callbacks
    // of the skylines, which are Scheme, compute with (skylines.rs).
    p("ly:skyline?", 1, 1, skylines::is_skyline),
    p("ly:skyline->points", 2, 2, skylines::skyline_points),
    // skyline-scheme.cc's
    p("ly:make-skyline", 3, 3, skylines::ly_make_skyline),
    p("ly:skyline-distance", 2, 3, skylines::ly_skyline_distance),
    p("ly:skyline-empty?", 1, 1, skylines::ly_skyline_empty_p),
    p("ly:skyline-height", 2, 2, skylines::ly_skyline_height),
    p(
        "ly:skyline-max-height",
        1,
        1,
        skylines::ly_skyline_max_height,
    ),
    p(
        "ly:skyline-max-height-position",
        1,
        1,
        skylines::ly_skyline_max_height_position,
    ),
    p("ly:skyline-merge", 2, 2, skylines::ly_skyline_merge),
    p("ly:skyline-pad", 2, 2, skylines::ly_skyline_pad),
    p(
        "ly:skyline-touching-point",
        2,
        3,
        skylines::ly_skyline_touching_point,
    ),
    p(
        "ly:skylines-for-stencil",
        2,
        2,
        skylines::ly_skylines_for_stencil,
    ),
    // What align-interface.cc's and page-layout-problem.cc's skylines,
    // which are Scheme, do with one skyline (skylines.rs).
    p("%skyline-empty", 1, 1, skylines::skyline_empty),
    p("%skyline-distance", 3, 3, skylines::skyline_distance),
    p("%skyline-raised", 2, 2, skylines::skyline_raised),
    p("%skyline-shifted", 2, 2, skylines::skyline_shifted),
    p("%skyline-merged", 2, 2, skylines::skyline_merged),
    p("%skyline-max-height", 1, 1, skylines::skyline_max_height),
    p(
        "%skyline-with-minimum-height",
        2,
        2,
        skylines::skyline_with_minimum_height,
    ),
    p("%boxes-skylines", 1, 3, skylines::boxes_skylines),
    // spring-smob.cc's smob (spring.rs).
    p("ly:make-spring", 2, 2, spring::ly_make_spring),
    p("ly:spring?", 1, 1, spring::is_spring),
    p(
        "ly:spring-set-inverse-compress-strength!",
        2,
        2,
        spring::ly_set_inverse_compress_strength,
    ),
    p(
        "ly:spring-set-inverse-stretch-strength!",
        2,
        2,
        spring::ly_set_inverse_stretch_strength,
    ),
    // simple-spacer-scheme.cc's (simple_spacer.rs).
    p(
        "ly:solve-spring-rod-problem",
        4,
        4,
        simple_spacer::ly_solve_spring_rod_problem,
    ),
    // The springs and the solver that page-layout-problem.cc's Scheme
    // spaces the staves with (spring.rs, simple_spacer.rs), and the
    // systems it spaces (system.rs).
    p("%make-spring", 2, 2, spring::make_spring),
    p("%spring-set!", 3, 3, spring::spring_set),
    p("%spacer-solve", 3, 4, simple_spacer::solve),
    p("%system?", 1, 1, system::is_system),
    p("%system-rank", 1, 1, system::rank_prim),
    p("%paper-column-rank", 1, 1, paper_column::rank_prim),
    p("%stencil-skylines", 4, 4, skylines::stencil_skylines),
    p("%box-skylines", 3, 3, skylines::box_skylines),
    p("%merged-skylines", 2, 2, skylines::merged_skylines),
    // What staff-symbol.cc's callbacks, which are Scheme, make their
    // stencil with (staff_symbol.rs).
    p("%staff-symbol-lines", 4, 4, staff_symbol::lines),
    // What ly:paper-column::print, which is Scheme, draws with
    // (paper_column.rs).
    p("%paper-column-rank-string", 1, 1, paper_column::rank_string),
    p("%paper-column-when-string", 1, 1, paper_column::when_string),
    p(
        "%paper-column-distance-string",
        1,
        1,
        paper_column::distance_string,
    ),
    p("%paper-column-arrow-y", 3, 3, paper_column::arrow_y),
    p("%spring-ideal-distance", 1, 1, spring::ideal_distance),
    p(
        "%points-to-line-stencil",
        2,
        2,
        lookup::ly_points_to_line_stencil,
    ),
    p("%stencil-add-stencil", 2, 2, stencil::add_stencil),
    // note-head-scheme.cc; note-head.cc's callbacks are Scheme
    // (note_head.rs).
    p(
        "ly:note-head::stem-attachment",
        2,
        3,
        note_head::ly_stem_attachment,
    ),
    // What self-alignment-interface.cc's callbacks, which are Scheme,
    // compute with (self_alignment.rs).
    p(
        "%interval-linear-combination",
        2,
        2,
        self_alignment::linear_combination,
    ),
    p("%grob-has-interface?", 2, 2, self_alignment::has_interface),
    // What axis-group-interface.cc's internal_calc_pure_relevant_grobs,
    // which is Scheme, sorts with (axis_group.rs).
    p("%grob-array-by-priority", 1, 1, axis_group::by_priority),
    // What axis-group-interface.cc's skylines, which are Scheme, do with
    // Skyline_pair and flower's Interval_set (axis_group.rs).
    p(
        "%skyline-pair-empty?",
        1,
        1,
        axis_group::skyline_pair_is_empty,
    ),
    p(
        "%skyline-pair-shifted",
        2,
        2,
        axis_group::skyline_pair_shifted,
    ),
    p(
        "%skyline-pair-raised",
        2,
        2,
        axis_group::skyline_pair_raised,
    ),
    p(
        "%skyline-pairs-union",
        1,
        1,
        axis_group::skyline_pairs_union,
    ),
    p(
        "%skyline-pairs-merged",
        2,
        2,
        axis_group::skyline_pairs_merged,
    ),
    p("%outside-staff-move", 7, 7, axis_group::outside_staff_move),
    // The packs a host gives, and the table they are kept in (font.rs).
    p("%put-font-pack", 1, 1, font::put_font_pack),
    p("%make-all-font-metrics", 0, 0, font::make_all),
    // The references' data form (dump.rs).
    p("%dump->stencil", 3, 3, dump::from_dump),
    p("%stencil->dump", 1, 1, dump::to_dump),
    p("%dump-forget-offset!", 2, 2, dump::forget_offset),
];

/// `stencil-expression.cc`'s stencil heads, which
/// `ly:register-stencil-expression` marks with the object property
/// `stencil-head?`, as `define-stencil-commands.scm` does at load, and
/// `ly:make-stencil`, which takes no other head and checks the extents
/// before `%make-stencil` makes the stencil. Moved from the scheme crate's
/// `(lily)` (MS2 step 12) with the stencils. And `%warned`, through which
/// a primitive gives LilyPond's warnings to `ly:warning` ([`warned`]).
/// And `%all-fonts`, `All_font_metrics` (`font.rs`): the packs a host gave
/// `%put-font-pack` and the fonts loaded from them.
const PRELUDE: &str = r#"
(define %stencil-heads '())
(define-public (ly:register-stencil-expression symbol)
  (%check-type "ly:register-stencil-expression" 1 symbol? "symbol" symbol)
  (set-object-property! symbol 'stencil-head? #t)
  (set! %stencil-heads (cons symbol %stencil-heads))
  (if #f #f))
(define-public (ly:all-stencil-expressions) %stencil-heads)
(define (%number-pair? x) (and (pair? x) (number? (car x)) (number? (cdr x))))
(define*-public (ly:make-stencil expr #:optional (xext '(+inf.0 . -inf.0))
                                 (yext '(+inf.0 . -inf.0)))
  (if (and (pair? expr) (not (object-property (car expr) 'stencil-head?)))
      (scm-error 'wrong-type-arg "ly_make_stencil"
                 "Wrong type argument in position ~A (expecting ~A): ~S"
                 (list 1 "registered stencil expression" expr) (list expr)))
  (%check-type "ly:make-stencil" 2 %number-pair? "number pair" xext)
  (%check-type "ly:make-stencil" 3 %number-pair? "number pair" yext)
  (%make-stencil expr xext yext))
(define (%warned texts value) (for-each ly:warning texts) value)
(define (%programming-errored texts value) (for-each ly:programming-error texts) value)
(define %all-fonts (%make-all-font-metrics))
"#;

/// `value`, after LilyPond's `warnings`, through `(lily)`'s `%warned`,
/// which calls `ly:warning` with each.
pub(crate) fn warned(cx: &mut Cx, value: Value, warnings: &[alloc::string::String]) -> R {
    reported(cx, "%warned", value, warnings)
}

/// `value`, after LilyPond's programming errors `errors`.
pub(crate) fn programming_errored(
    cx: &mut Cx,
    value: Value,
    errors: &[alloc::string::String],
) -> R {
    reported(cx, "%programming-errored", value, errors)
}

/// `value`, after `(lily)`'s `reporter` is given `warnings`.
fn reported(cx: &mut Cx, reporter: &str, value: Value, warnings: &[alloc::string::String]) -> R {
    if warnings.is_empty() {
        return Ok(value);
    }
    let Some(proc) = cx.global("lily", reporter) else {
        return Ok(value);
    };
    let texts: alloc::vec::Vec<Value> = warnings
        .iter()
        .map(|w| cx.heap_mut().string(w.replace('~', "~~")))
        .collect();
    let texts = cx.heap_mut().list(&texts);
    Ok(cx.tail_call(proc, &[texts, value]))
}

/// `misc-error` from `subr`, with `message`, as a primitive throws it.
pub(crate) fn misc_error(heap: &mut Heap, subr: &str, message: &str) -> Exception {
    let key = heap.intern("misc-error");
    let subr = heap.string(subr);
    let message = heap.string(message);
    let args = heap.list(&[subr, message, Value::NULL, Value::FALSE]);
    Exception { key, args }
}

/// `numerical-overflow` from `subr`, a checked result that did not fit
/// where LilyPond's would wrap (D61), as the music crate's primitives
/// throw it.
#[cold]
pub(crate) fn overflow(heap: &mut Heap, subr: &str) -> Exception {
    let key = heap.intern("numerical-overflow");
    let subr = heap.string(subr);
    let message = heap.string("Numerical overflow");
    let args = heap.list(&[subr, message, Value::NULL, Value::FALSE]);
    Exception { key, args }
}

/// The procedure `name` of `(lily)`, as LilyPond's `Lily::` variables
/// give it; Guile's error if it is unbound.
pub(crate) fn lily_procedure(
    host: &mut dyn lily_engine_music::Host,
    name: &str,
) -> Result<Value, Exception> {
    if let Some(proc) = host.lily(name) {
        return Ok(proc);
    }
    let heap = host.heap();
    let key = heap.intern("unbound-variable");
    let message = heap.string("Unbound variable: ~S");
    let sym = heap.intern(name);
    let items = heap.list(&[sym]);
    let args = heap.list(&[Value::FALSE, message, items, Value::FALSE]);
    Err(Exception { key, args })
}

/// The procedure `name` of `(lily)` called on `args`.
pub(crate) fn call_lily(
    host: &mut dyn lily_engine_music::Host,
    name: &str,
    args: &[Value],
) -> Result<Value, Exception> {
    let f = lily_procedure(host, name)?;
    host.call(f, args)
}

/// `wrong-type-arg` as Guile throws it: from `subr`
/// (`scm_wrong_type_arg_msg`, at no position for 0), or, for no `subr`,
/// from a conversion (`scm_to_double`).
pub fn wrong_type(
    heap: &mut Heap,
    subr: Option<&str>,
    position: usize,
    v: Value,
    expecting: &str,
) -> Exception {
    let key = heap.intern("wrong-type-arg");
    let expecting = heap.string(expecting);
    let (message, items) = match (subr, position) {
        (Some(_), 1..) => {
            let position = heap.integer(position as i64);
            (
                "Wrong type argument in position ~A (expecting ~A): ~S",
                heap.list(&[position, expecting, v]),
            )
        }
        (Some(_), 0) => (
            "Wrong type argument (expecting ~A): ~S",
            heap.list(&[expecting, v]),
        ),
        (None, _) => ("Wrong type (expecting ~A): ~S", heap.list(&[expecting, v])),
    };
    let subr = subr.map_or(Value::FALSE, |subr| heap.string(subr));
    let message = heap.string(message);
    let rest = heap.list(&[v]);
    let args = heap.list(&[subr, message, items, rest]);
    Exception { key, args }
}

// --- arguments, as LilyPond checks and converts them ------------------------------------

/// `to_scm (Offset)`: a pair of reals.
pub(crate) fn offset(heap: &mut Heap, (x, y): Offset) -> Value {
    let (x, y) = (heap.flonum(x), heap.flonum(y));
    heap.cons(x, y)
}

/// `scm_to_double`: a complex number is Guile's type error, from no
/// procedure and with no position.
pub(crate) fn to_double(cx: &mut Cx, v: Value) -> Result<f64, Thrown> {
    match cx.heap().to_f64(v) {
        Some(x) => Ok(x),
        None => Err(cx.wrong_type_vm(None, v, "real number")),
    }
}

/// `LY_ASSERT_TYPE (scm_is_number, …)`, then `from_scm<double>`.
pub(crate) fn number_arg(
    cx: &mut Cx,
    subr: &str,
    position: usize,
    v: Value,
) -> Result<f64, Thrown> {
    if !cx.heap().is_number(v) {
        return Err(cx.wrong_type(subr, position, v, "number"));
    }
    to_double(cx, v)
}

/// `LY_ASSERT_TYPE (is_number_pair, …)`, then `from_scm<Offset>` or
/// `from_scm<Interval>`.
pub(crate) fn number_pair_arg(
    cx: &mut Cx,
    subr: &str,
    position: usize,
    v: Value,
) -> Result<Offset, Thrown> {
    let heap = cx.heap();
    let pair = heap.car(v).zip(heap.cdr(v));
    match pair {
        Some((a, d)) if heap.is_number(a) && heap.is_number(d) => {
            Ok((to_double(cx, a)?, to_double(cx, d)?))
        }
        _ => Err(cx.wrong_type(subr, position, v, "number pair")),
    }
}

/// `LY_ASSERT_TYPE (is_scm<Offset>, …)`, then `from_scm<Offset>`.
pub(crate) fn offset_arg(
    cx: &mut Cx,
    subr: &str,
    position: usize,
    v: Value,
) -> Result<Offset, Thrown> {
    let heap = cx.heap();
    match heap.car(v).zip(heap.cdr(v)) {
        Some((a, d)) if heap.is_real(a) && heap.is_real(d) => {
            Ok((to_double(cx, a)?, to_double(cx, d)?))
        }
        _ => Err(cx.wrong_type(subr, position, v, "pair of reals")),
    }
}

/// `LY_ASSERT_TYPE (is_scm<Axis>, …)`: an exact 0 or 1.
pub(crate) fn axis_arg(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<Axis, Thrown> {
    match v.as_fixnum() {
        Some(a @ 0..=1) => Ok(a as Axis),
        _ => Err(cx.wrong_type(subr, position, v, "axis")),
    }
}

/// `LY_ASSERT_TYPE (is_scm<Direction>, …)`: an exact -1, 0 or 1.
pub(crate) fn direction_arg(
    cx: &mut Cx,
    subr: &str,
    position: usize,
    v: Value,
) -> Result<Direction, Thrown> {
    match v.as_fixnum() {
        Some(d @ -1..=1) => Ok(d as Direction),
        _ => Err(cx.wrong_type(subr, position, v, "direction")),
    }
}
