//! Lily Studio's main process without a window (DECISIONS D42, D55): the
//! state the commands share (`studio`), the commands themselves without their
//! dialogs (`ops`), each score's playback setup, the recent list, Export
//! MIDI with Export for GarageBand's rewrite, its WAV aligner and its folder,
//! and Import Guitar Pro (D166). The app (src-tauri) adds native dialogs, the
//! menu and the IPC; the web server's workers (crates/server) add the page's
//! own dialogs and the socket.

pub mod garageband_folder;
pub mod guitar_pro;
pub mod midi_export;
pub mod ops;
pub mod playback;
pub mod recent;
pub mod studio;
pub mod wav_align;

pub use ops::{MutedPart, Opened, RecentAnswer, Shown, TemplateInfo, templates};
pub use studio::{Paths, Sink, Studio, StudioEvent, StudioOptions, lock};
