//! The commands behind src/renderer/bridge.ts (DECISIONS D42), one per call,
//! as main.ts's IPC handlers were: file access (D29), compiling (D31, D33,
//! D36, D39), point-and-click (D32), changes on disk (D34), LilyPond's setup
//! and the sample (D37), the agents (D40), each score's playback setup (D45),
//! Export MIDI (D54), Export for GarageBand (D162, D164), Import Guitar Pro
//! (D166) and the recent list (D49). Every path from the page is checked
//! against what the user opened; only the window "main" may call.
//! What a command does once the user has chosen is crates/session's (D55),
//! shared with the web server; the native dialogs are the app's.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use lily_agents::{AGENTS, AgentId, AgentStatus, ChatInfo, ChatMessage, Decision, OpenChat};
use lily_engrave::setup::{self, DetectOptions, LilyPondState};
use lily_engrave::{FolderListing, LilyPondStatus, PdfOutcome, SourceLocation};
use lily_session::garageband_folder::{self, ScoreAudio};
use lily_session::guitar_pro;
use lily_session::ops::score_extensions;
use lily_session::{
    MutedPart, Opened, RecentAnswer, Shown, Studio, StudioEvent, TemplateInfo, lock, midi_export,
};
use serde::Serialize;
use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_opener::OpenerExt;

use crate::dialogs::{self, Alert, Open, Save, Saved};
use crate::smoke::Smoke;
use crate::video;

type Studios<'a> = State<'a, Arc<Studio>>;
type Answer<T> = Result<T, String>;

/// The page listens from now on; a score opened before it did opens now (D123).
#[tauri::command]
pub fn subscribe(app: AppHandle, studio: Studios<'_>, channel: Channel<StudioEvent>) {
    studio.subscribe(move |event| {
        if let Err(error) = channel.send(event) {
            eprintln!("studio channel: {error}");
        }
    });
    crate::opening::listening(&app);
}

#[tauri::command]
pub fn templates() -> Vec<TemplateInfo> {
    lily_session::templates()
}

#[tauri::command]
pub async fn open_folder(app: AppHandle, studio: Studios<'_>) -> Answer<Option<Opened>> {
    let directory = lock(&studio.access).folder.clone();
    let chosen = dialogs::open(
        &app,
        Open {
            title: "Open Folder".into(),
            directories: true,
            create_directories: true,
            directory,
            ..Open::default()
        },
    )
    .await?;
    Ok(match chosen {
        Some(folder) => Some(studio.open_folder_at(&folder, None).await),
        None => None,
    })
}

#[tauri::command]
pub async fn open_file(app: AppHandle, studio: Studios<'_>) -> Answer<Option<Opened>> {
    let directory = lock(&studio.access).folder.clone();
    let chosen = dialogs::open(
        &app,
        Open {
            title: "Open Score".into(),
            files: true,
            extensions: score_extensions(),
            directory,
            ..Open::default()
        },
    )
    .await?;
    Ok(match chosen {
        Some(file) => Some(studio.open_file_at(&file).await),
        None => None,
    })
}

#[tauri::command]
pub async fn list_folder(studio: Studios<'_>) -> Answer<Option<FolderListing>> {
    Ok(studio.list_folder().await)
}

#[tauri::command]
pub async fn read_file(studio: Studios<'_>, file: Value) -> Answer<String> {
    studio.read_file(&file).await
}

#[tauri::command]
pub async fn save_file(studio: Studios<'_>, file: Value, text: String) -> Answer<()> {
    studio.save_file(&file, &text).await
}

#[tauri::command]
pub async fn new_score(
    app: AppHandle,
    studio: Studios<'_>,
    template: String,
) -> Answer<Option<Opened>> {
    if lily_engrave::templates::template(&template).is_none() {
        return Err(format!("Unknown template: {template}"));
    }
    let suggested = studio.new_score_suggestion().await;
    let chosen = dialogs::save(
        &app,
        Save {
            title: "New Score".into(),
            prompt: Some("Create".into()),
            directory: suggested.parent().map(Path::to_path_buf),
            name: suggested
                .file_name()
                .map(|n| n.to_string_lossy().into_owned()),
            extensions: score_extensions(),
            checkbox: None,
        },
    )
    .await?;
    match chosen {
        Some(chosen) => studio.create_score(&chosen, &template).await.map(Some),
        None => Ok(None),
    }
}

/// File › Import Guitar Pro… (D166): `source`, a file dropped on the window,
/// or else one an open panel chooses, written as a LilyPond score where a
/// save panel says, `<source>.ly` in the open folder or beside the source at
/// first, and opened as New Score's is; null when a panel was cancelled. The
/// file is converted before the save panel, so one that cannot be imported
/// is refused without asking where to save it. It is only read, and never
/// let into what the page may touch.
#[tauri::command]
pub async fn import_guitar_pro(
    app: AppHandle,
    studio: Studios<'_>,
    source: Option<PathBuf>,
) -> Answer<Option<Opened>> {
    let source = match source {
        Some(source) => source,
        None => {
            let directory = lock(&studio.access).folder.clone();
            let chosen = dialogs::open(
                &app,
                Open {
                    title: "Import Guitar Pro".into(),
                    message: Some("Choose a Guitar Pro file: .gp3, .gp4, .gp5 or .gpx.".into()),
                    prompt: Some("Import".into()),
                    files: true,
                    extensions: guitar_pro::extensions(),
                    directory,
                    ..Open::default()
                },
            )
            .await?;
            match chosen {
                Some(source) => source,
                None => return Ok(None),
            }
        }
    };
    let bytes = guitar_pro::read_source(&source).await?;
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = guitar_pro::convert(&name, &bytes)?;
    let suggested = studio.guitar_pro_suggestion(&source);
    let chosen = dialogs::save(
        &app,
        Save {
            title: "Import Guitar Pro".into(),
            prompt: Some("Import".into()),
            directory: suggested.parent().map(Path::to_path_buf),
            name: suggested
                .file_name()
                .map(|n| n.to_string_lossy().into_owned()),
            extensions: score_extensions(),
            checkbox: None,
        },
    )
    .await?;
    match chosen {
        Some(chosen) => studio.create_imported_score(&chosen, &text).await.map(Some),
        None => Ok(None),
    }
}

/// The score `file` belongs to, with its last result; the compile that
/// brings it up to date follows on the channel (D39).
#[tauri::command]
pub async fn show_score(studio: Studios<'_>, file: Value) -> Answer<Option<Shown>> {
    studio.show_score(&file).await
}

#[tauri::command]
pub fn set_dirty(window: WebviewWindow, studio: Studios<'_>, dirty: bool) {
    studio.dirty.store(dirty, Ordering::SeqCst);
    dialogs::set_edited(&window, dirty);
}

#[tauri::command]
pub fn reveal_source(studio: Studios<'_>, href: Value) -> Answer<Option<SourceLocation>> {
    studio.reveal_source(&href)
}

#[tauri::command]
pub async fn compile_pdf(studio: Studios<'_>, root_file: Value) -> Answer<Option<PdfOutcome>> {
    studio.compile_pdf(&root_file).await
}

#[tauri::command]
pub async fn export_pdf(studio: Studios<'_>, root_file: Value) -> Answer<Vec<PathBuf>> {
    studio.export_pdf(&root_file).await
}

#[tauri::command]
pub async fn confirm_reload(
    app: AppHandle,
    studio: Studios<'_>,
    smoke: State<'_, Smoke>,
    file: Value,
) -> Answer<bool> {
    let allowed = studio.check(&file)?;
    // The smoke test does not answer dialogs; it keeps the edits.
    if smoke.0.is_some() {
        return Ok(false);
    }
    let name = allowed
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let answer = dialogs::alert(
        &app,
        Alert {
            message: format!("{name} was changed by another program."),
            detail: "Reload it and lose the changes you have not saved, or keep your changes? Saving them will replace the other version."
                .into(),
            // The first is the default: keeping is the safe answer.
            buttons: vec!["Keep My Changes".into(), "Reload".into()],
        },
    )
    .await?;
    Ok(answer == 1)
}

// Synchronous, as the next is: Tauri runs those in order on the main thread,
// as the page sent them, where async ones may overtake each other and an older
// text would replace a newer one.
#[tauri::command]
pub fn edited(studio: Studios<'_>, file: Value, text: Option<String>) {
    studio.edited(&file, text);
}

#[tauri::command]
pub fn set_live(studio: Studios<'_>, on: bool) {
    studio.set_live(on);
}

#[tauri::command]
pub async fn lilypond_status(studio: Studios<'_>) -> Answer<LilyPondStatus> {
    Ok(studio.inner().lilypond_status().await)
}

#[tauri::command]
pub async fn choose_lilypond(
    app: AppHandle,
    studio: Studios<'_>,
) -> Answer<Option<LilyPondStatus>> {
    let chosen = dialogs::open(
        &app,
        Open {
            title: "Choose LilyPond".into(),
            message: Some("Choose the LilyPond folder you downloaded, or the lilypond program in its bin folder.".into()),
            prompt: Some("Choose".into()),
            files: true,
            directories: true,
            directory: Some(PathBuf::from("/Applications")),
            ..Open::default()
        },
    )
    .await?;
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    let chosen = chosen.to_string_lossy().into_owned();
    let configured = setup::choice_path(&chosen);
    let status = setup::detect_lilypond(DetectOptions {
        configured_path: Some(configured.clone()),
        path: studio.search_path.get(),
        ..DetectOptions::default()
    })
    .await;
    if status.state == LilyPondState::Missing {
        return Ok(Some(LilyPondStatus {
            message: format!(
                "There is no LilyPond in {chosen}. Choose the folder you downloaded from lilypond.org, or the lilypond program inside its bin folder."
            ),
            ..status
        }));
    }
    // Kept even when too old or broken, so the message stays about this one.
    let settings = {
        let mut settings = lock(&studio.settings);
        settings.lilypond_path = Some(configured);
        settings.clone()
    };
    studio.save_settings(&settings).await?;
    Ok(Some(studio.inner().found(status)))
}

#[tauri::command]
pub fn open_link(app: AppHandle, link: Value) -> Answer<()> {
    // Only the setup's own pages; the page cannot name any other address.
    let url = link
        .as_str()
        .and_then(setup::setup_link)
        .ok_or_else(|| format!("Unknown link: {link}"))?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn open_sample(studio: Studios<'_>) -> Answer<Opened> {
    studio.open_sample().await
}

#[tauri::command]
pub async fn agent_status(studio: Studios<'_>) -> Answer<Vec<AgentStatus>> {
    let mut statuses = Vec::new();
    for agent in AGENTS.iter() {
        statuses.push(studio.detect_agent(agent.id).await);
    }
    Ok(statuses)
}

fn agent_id(agent: &Value) -> Answer<AgentId> {
    agent
        .as_str()
        .and_then(AgentId::parse)
        .ok_or_else(|| format!("Unknown agent: {agent}"))
}

#[tauri::command]
pub async fn choose_agent(
    app: AppHandle,
    studio: Studios<'_>,
    agent: Value,
) -> Answer<Option<AgentStatus>> {
    let id = agent_id(&agent)?;
    let info = lily_agents::agents::agent(id);
    let chosen = dialogs::open(
        &app,
        Open {
            title: format!("Choose {}", info.label),
            message: Some(format!(
                "Choose the {0} program. In Terminal, `which {0}` prints where it is.",
                info.command
            )),
            prompt: Some("Choose".into()),
            files: true,
            hidden_files: true,
            directory: Some(home().unwrap_or_default().join(".local").join("bin")),
            ..Open::default()
        },
    )
    .await?;
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    studio
        .set_agent_settings(id, |settings| {
            settings.path = Some(chosen.to_string_lossy().into_owned())
        })
        .await?;
    Ok(Some(studio.detect_agent(id).await))
}

#[tauri::command]
pub async fn set_agent_model(studio: Studios<'_>, agent: Value, model: String) -> Answer<()> {
    let id = agent_id(&agent)?;
    let trimmed = model.trim().to_string();
    studio
        .set_agent_settings(id, |settings| {
            settings.model = (!trimmed.is_empty()).then_some(trimmed)
        })
        .await
}

/// The open folder: chats belong to it, and its agent works in it.
fn chat_folder(studio: &Studio) -> Answer<String> {
    lock(&studio.access)
        .folder
        .as_ref()
        .map(|folder| folder.to_string_lossy().into_owned())
        .ok_or_else(|| "Open a score or a folder first; the agent works on its files.".to_string())
}

#[tauri::command]
pub async fn chat_list(studio: Studios<'_>) -> Answer<Vec<ChatInfo>> {
    Ok(match chat_folder(&studio) {
        Ok(folder) => studio.chats.list(&folder).await,
        Err(_) => Vec::new(),
    })
}

#[tauri::command]
pub async fn chat_get(studio: Studios<'_>, chat_id: Value) -> Answer<Option<OpenChat>> {
    let (Some(chat_id), Ok(folder)) = (chat_id.as_str(), chat_folder(&studio)) else {
        return Ok(None);
    };
    Ok(studio.chats.get(chat_id, &folder).await)
}

#[tauri::command]
pub async fn chat_send(studio: Studios<'_>, message: Value) -> Answer<String> {
    let message = ChatMessage::from_value(message)?;
    let folder = chat_folder(&studio)?;
    studio.chats.send(message, &folder).await
}

/// A pasted image of a chat (D46), as base64; null for a file that is not one.
#[tauri::command]
pub async fn chat_image(studio: Studios<'_>, file: String) -> Answer<Option<String>> {
    use base64::Engine as _;
    let bytes = studio.chats.image(&file).await;
    Ok(bytes.map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// Answers what the agent of `chat_id` asked to be allowed (D52):
/// `allow`, `always` or `deny`.
#[tauri::command]
pub async fn chat_answer(
    studio: Studios<'_>,
    chat_id: Value,
    ask_id: Value,
    decision: Value,
) -> Answer<()> {
    let (Some(chat_id), Some(ask_id), Some(decision)) = (
        chat_id.as_str(),
        ask_id.as_str(),
        decision.as_str().and_then(Decision::parse),
    ) else {
        return Err("Expected a chat, a question and an answer.".to_string());
    };
    let folder = chat_folder(&studio)?;
    studio
        .chats
        .answer(chat_id, ask_id, decision, &folder)
        .await
}

#[tauri::command]
pub fn chat_stop(studio: Studios<'_>, chat_id: Value) {
    if let Some(chat_id) = chat_id.as_str() {
        studio.chats.stop(chat_id);
    }
}

#[tauri::command]
pub async fn chat_delete(studio: Studios<'_>, chat_id: Value) -> Answer<()> {
    let Some(chat_id) = chat_id.as_str() else {
        return Ok(());
    };
    let folder = chat_folder(&studio)?;
    studio.chats.delete(chat_id, &folder).await;
    Ok(())
}

/// The playback setup kept for `root_file` (D45); null when there is none.
/// The path is only a key: nothing is read from it.
#[tauri::command]
pub async fn playback_setup(studio: Studios<'_>, root_file: String) -> Answer<Option<Value>> {
    Ok(studio.playback.get(&root_file).await)
}

/// Keeps the playback setup of `root_file`; null or an empty one forgets it.
#[tauri::command]
pub async fn set_playback_setup(
    studio: Studios<'_>,
    root_file: String,
    setup: Value,
) -> Answer<()> {
    studio.playback.set(&root_file, &setup).await
}

/// Where Export MIDI wrote, and how many muted parts it left out.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedMidi {
    file: PathBuf,
    left_out: usize,
}

/// Writes `midi`, the music the transport of `root_file` plays, where the
/// user chooses (D54); null when the panel was cancelled. While some parts
/// are muted, and not all of them, the panel's checkbox leaves them out.
#[tauri::command]
pub async fn export_midi(
    app: AppHandle,
    studio: Studios<'_>,
    smoke: State<'_, Smoke>,
    root_file: Value,
    midi: String,
    muted: Vec<MutedPart>,
) -> Answer<Option<ExportedMidi>> {
    let allowed = studio.check(&root_file)?;
    let bytes = decode_midi(midi, "The MIDI could not be exported")?;
    // Read before the panel, so a file that is not MIDI is not asked about.
    midi_export::without_tracks(&bytes, &[])?;
    let Some(Saved { path, checked }) =
        save_midi(&app, &smoke, &allowed, "Export MIDI", &muted).await?
    else {
        return Ok(None);
    };
    let tracks = left_out(&muted, checked);
    let out = midi_export::without_tracks(&bytes, &tracks)?;
    write_midi(path, out, tracks.len()).await
}

/// What Export for GarageBand wrote (D162 stage 2): the folder, the `.mid`
/// in it, its WAVs, how many muted parts it left out, and the score's files
/// it could not put in, with why.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedGarageBand {
    file: PathBuf,
    midi: PathBuf,
    wavs: Vec<String>,
    left_out: usize,
    left_out_files: Vec<garageband_folder::LeftOut>,
}

/// Export for GarageBand (D162 stage 2): `<score> (GarageBand)/` in a folder
/// the user chooses, the score's own at first, holding `midi`, lilypond's
/// MIDI of `root_file` at the score's own tempo, rewritten by `plan` (the
/// renderer's `garageBandPlan`, which leaves the muted parts in), a WAV for
/// each file of `audio` on the score's timeline, and `Read me.txt`; null when
/// the panel was cancelled, or the user would not replace the folder there.
/// The panel's checkbox leaves out the `muted` parts as Export MIDI's does.
/// Once written, the `.mid` opens in GarageBand, or the Finder shows the
/// folder when GarageBand is not found (stage 3, opening.rs).
/// Export MIDI itself stays lilypond's bytes (D54).
#[tauri::command]
#[allow(clippy::too_many_arguments)] // A Tauri command takes its state and its arguments as these.
pub async fn export_garageband(
    app: AppHandle,
    studio: Studios<'_>,
    smoke: State<'_, Smoke>,
    root_file: Value,
    midi: String,
    plan: midi_export::GarageBandPlan,
    muted: Vec<MutedPart>,
    audio: Option<ScoreAudio>,
) -> Answer<Option<ExportedGarageBand>> {
    let allowed = studio.check(&root_file)?;
    let audio = audio.unwrap_or_default();
    files_beside_score(&audio)?;
    let bytes = decode_midi(midi, "The MIDI could not be exported for GarageBand")?;
    // Rewritten before the panel, so a file or a plan that fails is not asked about.
    midi_export::for_garageband(&bytes, &plan)?;
    let Some(Saved {
        path: into,
        checked,
    }) = choose_garageband_folder(&app, &smoke, &allowed, &muted).await?
    else {
        return Ok(None);
    };
    let folder = into.join(garageband_folder::folder_name(&allowed));
    let replace = folder.symlink_metadata().is_ok();
    if replace && smoke.0.is_none() && !confirm_replace(&app, &folder).await? {
        return Ok(None);
    }
    let tracks = left_out(&muted, checked);
    let lead_in = plan.lead_in;
    let out = midi_export::for_garageband(&bytes, &plan.without_parts(&tracks))?;
    let left_out_parts: Vec<String> = muted
        .iter()
        .filter(|part| tracks.contains(&part.track))
        .map(|part| part.name.clone())
        .collect();
    let ffmpeg = video::find_ffmpeg(&studio.search_path.get()).ok();
    let written = tauri::async_runtime::spawn_blocking(move || {
        garageband_folder::write_folder(
            &folder,
            &garageband_folder::FolderExport {
                root_file: &allowed,
                midi: &out,
                lead_in,
                audio: &audio,
                left_out_parts: &left_out_parts,
                ffmpeg: ffmpeg.as_deref(),
            },
            replace,
        )
    })
    .await
    .map_err(|error| format!("The export for GarageBand stopped: {error}"))??;
    // Stage 3: GarageBand imports the .mid, or the Finder shows the folder.
    // The smoke test opens neither.
    if smoke.0.is_none() {
        let (shown_midi, shown_folder) = (written.midi.clone(), written.folder.clone());
        let app = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            crate::opening::show_garageband_export(&app, &shown_midi, &shown_folder)
        })
        .await;
    }
    Ok(Some(ExportedGarageBand {
        file: written.folder,
        midi: written.midi,
        wavs: written.wavs,
        left_out: tracks.len(),
        left_out_files: written.left_out,
    }))
}

/// The files of `audio` are named as the score's lines name them: each a
/// file beside the score, never a path elsewhere.
fn files_beside_score(audio: &ScoreAudio) -> Answer<()> {
    let named = [&audio.audio, &audio.backing, &audio.music, &audio.stem];
    match named
        .into_iter()
        .flatten()
        .find(|name| name.is_empty() || name.contains(['/', '\\']) || *name == "." || *name == "..")
    {
        Some(name) => Err(format!("{name} is not a file beside the score.")),
        None => Ok(()),
    }
}

/// Asks where to write `<score> (GarageBand)`, the score's folder first, with
/// D54's checkbox while some parts are `muted`. The smoke test answers no
/// panels: it takes the score's folder, as it is checked.
async fn choose_garageband_folder(
    app: &AppHandle,
    smoke: &Smoke,
    root_file: &Path,
    muted: &[MutedPart],
) -> Answer<Option<Saved>> {
    let names: Vec<String> = muted.iter().map(|part| part.name.clone()).collect();
    let checkbox = (!names.is_empty()).then(|| (midi_export::checkbox_title(&names), true));
    let directory = root_file.parent().map(Path::to_path_buf);
    match (&smoke.0, directory) {
        (Some(_), Some(directory)) => Ok(Some(Saved {
            path: directory,
            checked: checkbox.is_some(),
        })),
        (_, directory) => {
            let name = garageband_folder::folder_name(root_file);
            dialogs::open_checking(
                app,
                Open {
                    title: "Export for GarageBand".into(),
                    message: Some(format!("The folder to write “{name}” in")),
                    prompt: Some("Export".into()),
                    directories: true,
                    create_directories: true,
                    directory,
                    checkbox,
                    ..Open::default()
                },
            )
            .await
        }
    }
}

/// Whether to replace `folder`, which is there already: Cancel is the default.
async fn confirm_replace(app: &AppHandle, folder: &Path) -> Answer<bool> {
    let name = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let within = folder
        .parent()
        .and_then(Path::file_name)
        .map(|name| format!(" in “{}”", name.to_string_lossy()))
        .unwrap_or_default();
    let answer = dialogs::alert(
        app,
        Alert {
            message: format!("“{name}” already exists{within}. Do you want to replace it?"),
            detail: "Replacing it deletes what it holds, a project saved in it too, and writes the export in its place."
                .into(),
            buttons: vec!["Cancel".into(), "Replace".into()],
        },
    )
    .await?;
    Ok(answer == 1)
}

fn decode_midi(midi: String, failed: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(midi)
        .map_err(|error| format!("{failed}: {error}"))
}

/// Asks where to write the MIDI of `root_file`, suggesting `<score>.mid`
/// beside it, with D54's checkbox while some parts are `muted`. The smoke
/// test answers no panels: it takes the suggestion, as it is checked.
async fn save_midi(
    app: &AppHandle,
    smoke: &Smoke,
    root_file: &Path,
    title: &str,
    muted: &[MutedPart],
) -> Result<Option<Saved>, String> {
    let names: Vec<String> = muted.iter().map(|part| part.name.clone()).collect();
    let checkbox = (!names.is_empty()).then(|| (midi_export::checkbox_title(&names), true));
    let directory = root_file.parent().map(Path::to_path_buf);
    let name = midi_export::suggested_name(root_file);
    match (&smoke.0, directory) {
        (Some(_), Some(directory)) => Ok(Some(Saved {
            path: directory.join(name),
            checked: checkbox.is_some(),
        })),
        (_, directory) => {
            dialogs::save_checking(
                app,
                Save {
                    title: title.into(),
                    prompt: Some("Export".into()),
                    directory,
                    name: Some(name),
                    extensions: vec!["mid".into(), "midi".into()],
                    checkbox,
                },
            )
            .await
        }
    }
}

/// The tracks of the `muted` parts when the checkbox was `checked`.
fn left_out(muted: &[MutedPart], checked: bool) -> Vec<usize> {
    if checked {
        muted.iter().map(|part| part.track).collect()
    } else {
        Vec::new()
    }
}

async fn write_midi(path: PathBuf, out: Vec<u8>, left_out: usize) -> Answer<Option<ExportedMidi>> {
    tokio::fs::write(&path, out)
        .await
        .map_err(|error| format!("{} could not be written: {error}", path.display()))?;
    Ok(Some(ExportedMidi {
        file: path,
        left_out,
    }))
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[tauri::command]
pub async fn recent_list(studio: Studios<'_>) -> Answer<RecentAnswer> {
    Ok(studio.recent_list(home()).await)
}

/// Opens an entry of the recent list; only those, so the page cannot name
/// any other path. One that is gone rejects with where it was.
#[tauri::command]
pub async fn open_recent(studio: Studios<'_>, path: String) -> Answer<Opened> {
    studio.inner().open_recent(&path, home().as_deref()).await
}

#[tauri::command]
pub async fn forget_recent(studio: Studios<'_>, path: String) -> Answer<()> {
    studio.recent.remove(&path).await
}

#[tauri::command]
pub async fn clear_recent(studio: Studios<'_>) -> Answer<()> {
    studio.recent.clear().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_audio_named_is_files_beside_the_score() {
        let beside = ScoreAudio {
            audio: Some("take.wav".into()),
            stem: Some("Song - vocals.flac".into()),
            ..ScoreAudio::default()
        };
        assert_eq!(files_beside_score(&beside), Ok(()));
        for name in ["../take.wav", "/tmp/take.wav", "a\\b.wav", "..", ".", ""] {
            let elsewhere = ScoreAudio {
                backing: Some(name.into()),
                ..ScoreAudio::default()
            };
            assert!(files_beside_score(&elsewhere).is_err(), "{name:?}");
        }
    }
}
