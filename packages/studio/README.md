# Lily Studio

A small Mac app for LilyPond scores aimed at people who do not program: the
files of a folder on the left, the text in the middle, the engraved score on the
right, and nothing to configure. It is a Tauri app
with a Rust port of the [VS Code extension](../vscode)'s compiler and error
parser, shares its grammar, preview and player (`packages/common`), and needs
LilyPond installed in the same way.

- **First launch.** A welcome screen offers a sample score (Ode to Joy, saved
  in *Documents › Lily Studio*), new scores from templates, and opening files.
  If LilyPond cannot be found, a setup guide opens: download it, move it to
  Applications, and show the studio where it is. *Help › Set Up LilyPond…*
  brings it back.
- **Editing.** Open a score and it is engraved right away; switch files and the
  preview switches with them. Save with <kbd>⌘S</kbd>, or just type: the score
  follows as you type (*Live preview* in the status line). Click a note to find it in the
  text. The SVG/PDF switch shows the printable PDF, and *Export PDF* saves it
  next to the score. ▶ plays the music and marks the notes as they sound;
  *Parts* mutes a part or plays it on another instrument, and *File › Export
  MIDI…* saves the music, with or without the muted parts. *Add audio…* in
  *Parts* plays a recording along with the music, as a track of its own with
  its own volume, starting where *Starts at* says (in seconds, below 0 already
  playing) and ending the music where the MIDI or the recording ends;
  *Tempo* plays the MIDI at another ♩ a minute to keep with it. *From the
  recording*, beside it, finds both: the recording's tempo and the start
  that puts the score's bar 1 on the recording's (it needs uv, as stems
  do). The recording, the start and the tempo are lines of the score:
  `% Audio:`, `% Audio offset:` and `% BPM:`. *Visual* shows it
  falling onto a keyboard where the source is, or below the score, as piano
  videos do; *Tracks* picks the parts whose notes fall. Its *Karaoke* style
  runs the notes past a line with the words below them, coloured as they are
  sung, each singer in a colour of their own, as bars or on a staff; *Karaoke
  and keyboard* shows the keyboard below it. *Min text* sets the words' least
  size: a longer line breaks onto two. *Export Video* saves it as a
  1080p MP4 with the parts and recordings you tick; it needs
  `ffmpeg` (`brew install ffmpeg`).
- **Guitar Pro.** *File › Import Guitar Pro…* (or the welcome screen's
  button) takes a `.gp3`, `.gp4`, `.gp5` or `.gpx` file, asks where to save
  its score, `<song>.ly` in the open folder or beside the file, and opens
  it: each guitar or bass a staff with its tab in the track's tuning, drums
  a drum staff (DECISIONS D166). Dropping a Guitar Pro file on the window
  does the same. On the web the file is uploaded, 2 MB at most, and its
  score saved in My Scores. A file that cannot be imported is refused, with
  the reason, before the save panel. Lyrics, chord diagrams and the tremolo
  bar are left out, and a `.gpx`'s drums come in as rests. Guitar Pro 7 and
  8's `.gp` is not read yet.
- **Mistakes** are underlined, and a note above the score explains the first
  one in plain words, for example *“\stacato” is not a LilyPond command*.
  Click it to go there.
- **Stems.** The *Scores | Stems* switch at the top of the sidebar opens the
  Stems workspace, which splits a song into stems on Apple Silicon with
  RoFormer models (`packages/separate`). Drop a song on the window, or click to choose one. Then pick a
  model: *Karaoke* gives lead vocals and instrumental, and *All instruments*
  gives vocals, drums, bass, guitar, piano and other. Choose Fast, Balanced or
  Best and WAV, FLAC or MP3, and press *Make karaoke* or *Separate
  instruments*. Each stem gets a player. Ticked stems export as one mix, and
  *Show in Finder* opens the stems' folder, `<song> (stems)` or
  `<song> (karaoke)` next to the song. Songs separated before are listed
  under *Recent*, below the settings: a click shows their stems again
  without separating, and *Open…* takes a folder of stems made earlier, by
  this app or RoFormer MLX's own. A stem's *Score… › New score for
  bass* transcribes it: `bass.ly`, beside the stems, has the bass line's
  notes in the song's key, tempo and meter, from the first bar to the last,
  and opens in Scores. Vocals get their tune and the words heard, a syllable
  a note, and a karaoke separation's instrumental as their backing track; guitar, piano and the rest a
  rough draft of their chords (*Score… (draft)*); drums their kick, snare
  and hi-hat. The stem then plays above the preview's transport, alone or
  *With score*, in time with the score's MIDI, so what the transcription got
  wrong is heard and fixed. *Empty score for bass* writes four empty bars to
  fill in by ear instead.

## Build and install

It needs Node and a Rust toolchain (`rustup`). Run `npm install` once at the
repository's root (an npm workspace), then in `packages/studio`:

```sh
npm start              # run from the sources
npm test               # Rust and renderer unit tests, then the window's smoke test
npm run dist           # release/Lily Studio-<version>-<arch>.dmg, signed and notarized
npm run dist:x64       # the same for Intel Macs (rustup target add x86_64-apple-darwin once)
npm run dist:local     # the same DMG, ad-hoc signed, for a Mac without the certificate
npm run test:e2e       # build the DMG, install the app from it, launch it
```

Open the DMG and drag Lily Studio to Applications. The app is signed with a
Developer ID and notarized by Apple, so it opens without a warning. A
`dist:local` build is not: on another Mac, open it the first time with
right-click › Open.

`npm start` and an installed Lily Studio are one app to macOS: the same name,
bundle identifier and data folder. Only one runs at a time, so `npm start`
while Lily Studio is open only brings the open one forward. Quit it first.
Either one keeps its settings, recent files and chats in *Application Support ›
Lily Studio*, and installing a new DMG leaves them there.

`npm run dist` needs the Developer ID Application certificate in the login
keychain and a notarization profile named `lily-notary`, stored once with
`xcrun notarytool store-credentials lily-notary --apple-id <id> --team-id <team>`.

## Updates

An installed Lily Studio checks `https://lily.everyday.fail/dist/latest.json`
at launch and from *Lily Studio › Check for Updates…*, and offers a newer
version (DECISIONS D124). `npm run dist` makes the update beside the DMG:
`release/Lily Studio-<version>-<arch>.app.tar.gz`, signed with the updater's
key, and `release/latest.json`. Run `npm run dist:x64` after it for both
architectures in one `latest.json`. To publish, bump the version in
`package.json`, `Cargo.toml` and `src-tauri/tauri.conf.json`, write the notes
into `latest.json` if wanted, and run `npm run upload`. It sends the DMGs and
the `.tar.gz` files of the version in `latest.json` to `/dist/` on the site,
then `latest.json`, and reads them back through the public URLs. It needs
rsync and SSH access to the site's server, named by `LILY_SITE_HOST` as ssh
names it (`user@host`, or a `Host` of `~/.ssh/config`), which
`npm run upload:site` takes too. The server keeps the old versions.

The site's home page is `website/site/`, and `npm run upload:site` publishes
it. Its download link follows `latest.json`, so a release does not change it.
The other files in `website/` are the drafts it was chosen from.

The private key is `~/.tauri/lily-studio.key` (`TAURI_SIGNING_PRIVATE_KEY_PATH`
names another). Keep a copy somewhere safe: an installed app takes only
updates signed with it, so without it no installed copy can be updated.

## Stems

Stems splits a song with this repository's `packages/separate`
(`karaoke-mlx`, DECISIONS D112, D125) and transcribes a stem with
`packages/transcribe` (D113). The studio runs both with
[uv](https://docs.astral.sh/uv/). To set them up:

1. Use a Mac with Apple Silicon. Install [Homebrew](https://brew.sh), then
   uv and ffmpeg: `brew install uv ffmpeg`.
2. Clone this repository and prepare both projects once, so the first
   separation does not wait for them:

   ```sh
   git clone https://github.com/velet5/lily && cd lily
   (cd packages/separate && uv sync)
   (cd packages/transcribe && uv sync)
   ```

3. Start the studio. Built from this clone (`npm install`, then `npm start`
   here; it needs [Rust](https://rustup.rs)), it finds both projects by
   itself. An installed Lily Studio looks for them where it was built, so
   quit it (⌘Q) and open it from the clone's folder with their paths:

   ```sh
   open -a "Lily Studio" \
     --env KARAOKE_MLX_ROOT="$PWD/packages/separate" \
     --env LILY_TRANSCRIBE_ROOT="$PWD/packages/transcribe"
   ```

4. Choose *Stems* at the top of the sidebar, drop a song, pick a *Karaoke*
   model and press *Make karaoke*. Each model downloads on its first
   separation (0.7 to 1.7 GB, into `~/.cache/karaoke-mlx`), and the first
   transcription downloads Beat This's model (77 MB).

When Stems finds no project or no uv, it says which, with *Check again*. A
folder that `KARAOKE_MLX_ROOT` or `LILY_TRANSCRIBE_ROOT` names must hold the
project's `pyproject.toml`. Transcribing uses Spotify's Basic Pitch for the
notes and CPJKU's Beat This! for the beats, with Python 3.11 and torch.

The web has no Stems: separating needs the Mac app.

## On the web

`lily-studio-server` serves the same studio to browsers, for many users. It
has no agents. Each user signs in and works in their own *My Scores*. Scores
are uploaded and downloaded through the menu bar, and engraved on the
server. Each user's work runs in its own [bubblewrap](https://github.com/containers/bubblewrap)
sandbox, because a score can run programs (DECISIONS D55).

```sh
npm run serve          # on a Mac, for development: no sandbox, sign-up on, http://127.0.0.1:8080
npm run build:site     # dist/site/ and a release build of lily-studio-server
```

On a Linux server, with `lilypond` and `bubblewrap` installed:

```sh
lily-studio-server user add ada --data /srv/lily     # asks for a password
lily-studio-server serve --data /srv/lily --site dist/site --runtime dist/runtime \
  --listen 127.0.0.1:8080 --secure-cookies
```

Put a proxy that terminates HTTPS in front of it, and forward the Host
header and WebSocket upgrades. `--signup` lets anyone create an account.
`lily-studio-server serve --help` lists the rest. The `Dockerfile` builds an
image with LilyPond and bubblewrap, and its first lines say how to run it.

## Development

`npm run lint:rust` runs `cargo fmt` and `clippy`. The Rust side is a Cargo
workspace here: `crates/engrave` (compiling, live preview, files, settings),
`crates/agents` (Claude Code and Codex), `crates/session` (the main process
without a window, shared by the app and the server), `crates/server` (the
web), `crates/separate` (running karaoke-mlx and lily-transcribe, for the
app alone), `crates/guitarpro` (Guitar Pro files read and written as
LilyPond, D166) and `src-tauri` (the app). The page is `renderer/` and `src/renderer/`;
`src/web/` is what the web adds to it. `docs/DECISIONS.md` at the repository's root
records the studio's decisions from D28 on.

The smoke test separates a song too when it is given one. It drops the song
on the window, splits it into six stems with BS-RoFormer SW at Fast, plays
each, exports a mix, and transcribes the bass into a score, whose tempo,
notes and bars it checks. That takes about a minute and a half on the development Mac:

```sh
LILY_SMOKE_SONG=~/Music/song.mp3 npm run test:smoke   # a copy of the song, in a scratch folder
LILY_E2E_SONG=~/Music/song.mp3 npm run test:e2e       # the same, in the installed app
```

## License

MIT; the text is in the `LICENSE` file at the repository's root.
