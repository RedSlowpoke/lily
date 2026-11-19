//! The samples of README §1, which are copyrighted songs and so not in the
//! repository: each is read from `GUITAR_PRO_SAMPLES` (unset, none
//! is found), and the test skips the files it cannot find.
use std::path::PathBuf;

use lily_guitarpro::{Format, Version, sniff};

fn samples() -> PathBuf {
    std::env::var_os("GUITAR_PRO_SAMPLES").map_or_else(PathBuf::new, PathBuf::from)
}

const fn binary(major: u8, minor: u8) -> Format {
    Format::Binary(Version { major, minor })
}

#[test]
fn sniffs_every_sample() {
    let cases = [
        ("gpx-a.gpx", Format::Gpx),
        ("gp4-a.gp4", binary(4, 0)),
        ("gp3-b.gp3", binary(3, 0)),
        ("gp3-a.gp3", binary(3, 0)),
        ("gp5-b.gp5", binary(5, 10)),
        ("gp5-c.gp5", binary(5, 10)),
        ("gp5-a.gp5", binary(5, 0)),
    ];
    let mut read = 0;
    for (name, format) in cases {
        let path = samples().join(name);
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("skipped {}: not found", path.display());
            continue;
        };
        let extension = name.rsplit('.').next();
        assert_eq!(sniff(&bytes, extension), Ok(format), "{name}");
        read += 1;
    }
    if read == 0 {
        eprintln!("skipped: no samples in {}", samples().display());
    }
}
