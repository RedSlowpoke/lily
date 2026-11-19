#!/usr/bin/env python3
"""Wrap a ProjectData that GarageBand did not write into a .band, and try it (§12).

    python3 -I wrap_projectdata.py SOURCE OUT.band [--logic-info]
    python3 -I wrap_projectdata.py --open OUT_DIR [NAME ...]

The first form builds OUT.band as §2.2 lays a package out, from SOURCE: a
`.logicx` (LogicProFormatWriter's, say), a folder holding
`Alternatives/000/ProjectData`, or a bare `ProjectData` file.

    projectData                         GarageBand for iOS's stub (§2.4), copied
                                        from a template in GarageBand.app
    Alternatives/000/ProjectData        SOURCE's, byte for byte
    Alternatives/000/MetaData.plist     SOURCE's, or a minimal one when it has none
    Media/Audio Files/                  SOURCE's audio files, byte for byte
    Resources/ProjectInformation.plist  as GarageBand 10.4.14 writes it, with the
                                        package's name; --logic-info keeps SOURCE's

No `DisplayState` files, `WindowImage.jpg` or `Contents/`: the projects
GarageBand saved have no `Contents/`, and the rest are window state (§2.3).

The second form opens each OUT_DIR/NAME.band (every .band there by default)
in GarageBand through AppleScript, has it save the project to
OUT_DIR/resaved/NAME.band, closes it without saving, and reads back what
GarageBand kept, as §5.1 does. It says whether GarageBand left a new crash
report in ~/Library/Logs/DiagnosticReports/. It needs GarageBand running,
the terminal's Apple Events consent (§3.6), and no project with unsaved
changes open.

Standard library only; it imports inspect_band.py and patch_band_template.py
from beside it.
"""

from __future__ import annotations

import glob
import hashlib
import os
import plistlib
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # -I leaves it out
import inspect_band as ib  # noqa: E402
import patch_band_template as pt  # noqa: E402

TEMPLATES = "/Applications/GarageBand.app/Contents/Resources/Project Templates/NewProject"
STUB_MD5 = "788ce1355c5036d5419ecf112bfade5e"  # the same in every template and save (§2.4)
SAVED_FROM = "GarageBand 10.4.14 (6648)"
ASSET_FLAGS = 8409  # what 10.4.14 writes (§2.3)
REPORTS = os.path.expanduser("~/Library/Logs/DiagnosticReports")


def stub() -> bytes:
    for band in sorted(glob.glob(os.path.join(TEMPLATES, "*.band"))):
        path = os.path.join(band, "projectData")
        if os.path.isfile(path):
            with open(path, "rb") as f:
                data = f.read()
            if hashlib.md5(data).hexdigest() != STUB_MD5:
                print("note: %s is not the stub §2.4 measured" % path, file=sys.stderr)
            return data
    raise SystemExit("no GarageBand template with a projectData in %s" % TEMPLATES)


def source_parts(source: str) -> tuple[str, str | None, str | None, str | None]:
    """(ProjectData, MetaData.plist, Media/Audio Files, ProjectInformation.plist) of SOURCE."""
    if os.path.isfile(source):
        return source, None, None, None
    alt = os.path.join(source, "Alternatives", "000")
    data = os.path.join(alt, "ProjectData")
    if not os.path.isfile(data):
        raise SystemExit("%s has no Alternatives/000/ProjectData" % source)

    def found(path: str) -> str | None:
        return path if os.path.exists(path) else None
    return (data, found(os.path.join(alt, "MetaData.plist")),
            found(os.path.join(source, "Media", "Audio Files")),
            found(os.path.join(source, "Resources", "ProjectInformation.plist")))


def project_information(name: str) -> bytes:
    return plistlib.dumps({
        "BundleVersion": 2,
        "LastSavedFrom": SAVED_FROM,
        "projectAssetFlags": ASSET_FLAGS,
        "VariantNames": {"0": name},
        "VariantNamesV2": {"0": "{PROJECT_NAME}"},
    }, fmt=plistlib.FMT_BINARY)


def minimal_meta(audio: list[str]) -> bytes:
    """Only for a bare ProjectData: GarageBand does not read MetaData.plist on open (§5.3)."""
    return plistlib.dumps({"AudioFiles": ["Audio Files/" + a for a in audio], "Version": 3},
                          fmt=plistlib.FMT_BINARY)


def wrap(source: str, out: str, logic_info: bool = False) -> None:
    data, meta, media, info = source_parts(os.path.abspath(source))
    out = os.path.abspath(out)
    if not out.endswith(".band"):
        raise SystemExit("OUT must end in .band")
    if os.path.exists(out):
        raise SystemExit("%s exists" % out)
    name = os.path.basename(out)[:-5]
    alt = os.path.join(out, "Alternatives", "000")
    os.makedirs(alt)
    os.makedirs(os.path.join(out, "Resources"))
    with open(os.path.join(out, "projectData"), "wb") as f:
        f.write(stub())
    shutil.copyfile(data, os.path.join(alt, "ProjectData"))
    audio = sorted(os.listdir(media)) if media else []
    if audio:
        os.makedirs(os.path.join(out, "Media", "Audio Files"))
        for a in audio:
            shutil.copyfile(os.path.join(media, a), os.path.join(out, "Media", "Audio Files", a))
    if meta:
        shutil.copyfile(meta, os.path.join(alt, "MetaData.plist"))
    else:
        with open(os.path.join(alt, "MetaData.plist"), "wb") as f:
            f.write(minimal_meta(audio))
    info_path = os.path.join(out, "Resources", "ProjectInformation.plist")
    if logic_info and info:
        shutil.copyfile(info, info_path)
    else:
        with open(info_path, "wb") as f:
            f.write(project_information(name))
    with open(data, "rb") as f:
        header = ib.read_header(f.read(24))
    print("%s: format 0x%04x, %d audio files, %s" % (out, header["format"], len(audio),
          "the source's ProjectInformation" if logic_info and info else "GarageBand's ProjectInformation"))


def crash_reports() -> set[str]:
    return set(glob.glob(os.path.join(REPORTS, "GarageBand*")))


def open_and_resave(band: str, resaved: str, wait: int = 180) -> dict:
    """§5.1's loop, but it saves only a document whose path is BAND.

    A project that opens late, after `open` gave up waiting, would otherwise be
    taken for the next one. `open` returns the document, or `missing value`
    when GarageBand declines the file without a word."""
    pt.close_all()
    r: dict = {}
    ok, msg = pt.gb('open POSIX file "%s"' % band, timeout=wait)
    r["open"] = msg if ok else "no answer: " + msg
    path = ""
    for _ in range(wait // 5):
        ok, n = pt.gb("count of documents", timeout=10)
        if ok and n not in ("", "0"):
            path = pt.gb("path of document 1", timeout=10)[1]
            break
        time.sleep(5)
    r["windows"] = pt.gb("name of every window", timeout=10)[1]
    if not path:
        r["documents"] = "none"
        return r
    if os.path.realpath(path) != os.path.realpath(band):
        r["documents"] = "another project opened: %s" % path
        pt.gb("close document 1 saving no", timeout=30)
        return r
    time.sleep(3)  # let it settle before reading its state
    r["modified"] = pt.gb("modified of document 1", timeout=10)[1]
    if os.path.exists(resaved):
        shutil.rmtree(resaved)
    ok, msg = pt.gb('save document 1 in POSIX file "%s"' % resaved, timeout=90)
    r["save"] = "ok" if ok else msg
    pt.gb("close document 1 saving no", timeout=30)
    return r


def try_open(out_dir: str, names: list[str]) -> int:
    out_dir = os.path.abspath(out_dir)
    if not names:
        names = sorted(n[:-5] for n in os.listdir(out_dir) if n.endswith(".band"))
    failed = 0
    for name in names:
        band = os.path.join(out_dir, name + ".band")
        resaved = os.path.join(out_dir, "resaved", name + ".band")
        os.makedirs(os.path.dirname(resaved), exist_ok=True)
        print("== %s" % name)
        before = crash_reports()
        if os.path.exists(resaved):
            shutil.rmtree(resaved)
        result = open_and_resave(band, resaved)
        time.sleep(2)
        for k, v in result.items():
            print("  %-10s %s" % (k, v))
        new = sorted(crash_reports() - before)
        print("  %-10s %s" % ("crash", ", ".join(os.path.basename(n) for n in new) or "none"))
        if not os.path.isdir(resaved):
            print("  %-10s not written" % "resaved")
            failed += 1
            continue
        with open(ib.project_data_path(resaved), "rb") as f:
            data = f.read()
        meta = ib.plist(os.path.join(resaved, "Alternatives", "000", "MetaData.plist"))
        header = ib.read_header(data[:24])
        print("  %-10s format 0x%04x, %d bytes; saved by %s" % (
            "resaved", header["format"], len(data),
            (ib.plist(os.path.join(resaved, "Resources", "ProjectInformation.plist")) or {}).get("LastSavedFrom")))
        try:
            for k, v in pt.read_back(resaved).items():
                print("  %-10s %s" % (k, v))
        except Exception as e:  # a project of an unexpected shape should not stop the run
            print("  read back failed: %r" % e)
        lines, fails = ib.links(data, meta)
        for line in lines:
            if line.startswith("  arrange") or line.startswith("    track") or line.startswith("    master") \
                    or line.startswith("        q+") or line.startswith("  FAILED"):
                print("  " + line)
        print("  %-10s %d failed" % ("links", len(fails)))
        failed += bool(fails) or bool(new)
    return 1 if failed else 0


def main() -> int:
    args = sys.argv[1:]
    if args[:1] == ["--open"] and len(args) >= 2:
        return try_open(args[1], args[2:])
    logic_info = "--logic-info" in args
    args = [a for a in args if a != "--logic-info"]
    if len(args) == 2 and not args[0].startswith("-"):
        wrap(args[0], args[1], logic_info)
        return 0
    print(__doc__.split("\n\n")[1], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
