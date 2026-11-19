# Spikes for GarageBand projects

The scripts behind [the research note](../README.md). Each one's docstring
has its full usage. They are standard library only, run with `python3 -I`,
and are the repository's own code, under its MIT licence. None holds code
of another project.

| Script | What it does | Section |
| --- | --- | --- |
| `inspect_band.py` | Reads a `.band`: its files, plists and every record of `ProjectData`. `--links` prints how the records link and checks each link; it exits 1 if one fails | §2, §11 |
| `diff_band.py` | Compares two projects record by record, with GarageBand's ids and time stamps folded | §10 |
| `make_import_bundle.py` | Compiles a sample score and writes the MIDI file and stems of the MIDI-and-stems route. Needs `lilypond` | §4 |
| `patch_band_template.py` | Copies a `.band` GarageBand saved and changes values in place: tempo, names, an audio file | §5 |
| `wrap_projectdata.py` | Puts a `ProjectData` that GarageBand did not write, such as LogicProFormatWriter's, into a `.band` | §12 |
| `build_band.py` | Adds tracks, regions and audio files to a `.band` GarageBand saved, by copying its own tracks | §13 |

`diff_band.py`, `patch_band_template.py`, `wrap_projectdata.py` and
`build_band.py` import `inspect_band.py` from beside them.

## Trying a project in GarageBand

`patch_band_template.py`, `wrap_projectdata.py` and `build_band.py` take
`--open OUT_DIR`. For each project they write, GarageBand opens it, saves it
to `OUT_DIR/resaved/` and closes it without saving, through AppleScript.
The script then reads back what GarageBand kept (§5.1, §12.1). This needs:

- GarageBand 10.4.14 running, with no project with unsaved changes open.
  GarageBand holds one project at a time;
- the terminal's Apple Events consent for GarageBand (§3.6).

A bad file can crash GarageBand (§5.4). `wrap_projectdata.py` reports a new
crash report in `~/Library/Logs/DiagnosticReports/`.

## Inputs stay outside the repository

The projects these scripts read and write were saved by GarageBand, so they
hold Apple's bytes (§7.2). The templates, fixtures (§10.1) and outputs are
kept in a scratch folder and never committed. `make_import_bundle.py` and
`build_band.py` write their outputs again from their inputs.

The one exception is `make_import_bundle.py --fixture
packages/studio/crates/session/tests/fixtures`, which writes the session
crate's fixture of Export for GarageBand (D162): the sample score,
lilypond's MIDI of it, the plan the studio sends and the rewrite it should
give. It holds nothing of Apple's, so it is committed.

`make_import_bundle.py --lpfw LPFW_DIR` imports
[LogicProFormatWriter](https://github.com/jonkubis/LogicProFormatWriter)
(MIT) from a clone you make, to write its `.logicx` projects for §12. Clone
it into an empty folder of its own and read it before running it.
