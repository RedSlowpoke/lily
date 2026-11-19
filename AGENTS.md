# AGENTS.md

Instructions for coding agents working on this repository. How to write LilyPond
scores with the compile-and-fix loop of `lily-check` is in
[packages/vscode/AGENTS.md](packages/vscode/AGENTS.md), which ships with the
extension.

## Layout

An npm workspace of four packages (DECISIONS D53, D56), and two uv Python
projects beside it:

- `packages/common` (`@lily/common`): what both apps use. `src/span.ts`,
  `src/types.ts` and `src/definition.ts` import neither `vscode` nor Node
  (Lily Studio's renderer bundles them); `src/textedit.ts` may use Node. `web/preview.js` and
  `web/midi.js` are plain scripts with `.d.ts` files beside them. Also the
  TextMate grammar, the language configuration and lilypond's `runtime/`
  files. No build step: the apps bundle or copy what they import.
- `packages/vscode` (`lily`): the VS Code extension and `lily-check`. Read
  `packages/vscode/docs/ARCHITECTURE.md` §3 for its layout.
- `packages/studio` (`lily-studio`): Lily Studio, a Tauri app whose Rust side
  is a Cargo workspace in that directory (D42), and `lily-studio-server`, the
  same studio on the web (D55).
- `packages/engine` (`lily-engine`): our LilyPond engine in WebAssembly (D56),
  licensed GPL-3.0-or-later. Its own Cargo workspace, whose
  `rust-toolchain.toml` pins the nightly of the wasm build (D57).
  `crates/core` is its core behind chapter 08 §4.2's ABI (K2), and
  `crates/spike` chapter 09's spike, with its phone harness. The roadmap
  is `docs/research/wasm-engine/README.md`, and `progress.md` beside it says
  how far it has come.
- `packages/separate` (`karaoke-mlx`): not an npm package but a uv Python
  project, which Lily Studio runs to split a song into stems with RoFormer
  models on MLX (D112, D125). `uv run pytest -s` there runs its tests.
- `packages/transcribe` (`lily-transcribe`): not an npm package but a uv
  Python project, which Lily Studio runs to transcribe a stem into notes on
  the song's beat grid (D113). `uv run pytest` there runs its tests.

`docs/DECISIONS.md` records the decisions of all six; read it before changing
a convention and record new ones there.

## Commands

`npm install` once at the root. From the root:

```sh
npm run check-types     # tsc in every package
npm run lint            # oxlint over the repository; warnings fail
npm run test:grammar    # TextMate grammar snapshots (packages/common)
npm run build           # the extension and the studio
npm test                # grammar, then the extension's, the studio's and the engine's tests
```

In `packages/vscode`:

```sh
npm run build           # esbuild → dist/extension.js, dist/lily-check.js; copies packages/common into dist/
npm run test:unit       # node --test; tests in subdirectories of test/
npm test                # types, lint, unit, then the extension-host tests
npm run test:e2e        # packages the VSIX and runs a sample score through it
```

In `packages/studio`: `npm start`, `npm test` (Rust, renderer and smoke
tests), `npm run lint:rust`; its README has the rest.

In `packages/engine`: `npm test` and `npm run lint:rust`, which run
`cargo +stable`. A bare `cargo` there uses the pinned nightly, which rustup
installs on first use. `npm run build` builds the wasm into `dist/engine/`,
where the studio's `esbuild.mjs --site` takes it from, with the pinned nightly
and Binaryen's `wasm-opt` (on the PATH or as `WASM_OPT`), and prints the size
report; `npm run sizes` prints it again.

## Rules

- Something belongs in `packages/common` only if both apps use it. Import it
  as `@lily/common/…`, never by a relative path into another package.
- Nothing under `packages/common`, `src/compile/`, `src/intellisense/` (except
  `provider.ts`), `src/diagnostics/parse.ts` or `tools/` of the extension may
  import `vscode`.
- A VSIX holds only `packages/vscode`: a file the extension needs at run time
  is copied into `dist/` by its `esbuild.mjs` if it is in `packages/common`,
  and let into the VSIX in `.vscodeignore` either way; only
  `npm run test:e2e` notices when it is not.
- The engine is GPL-3.0-or-later and the apps are MIT: no app imports or
  links the engine's code. They reach it only through the worker protocol in
  `packages/studio/src/web/engine/` (D56).
- Work that is not about the engine changes nothing under `packages/engine/`,
  and adds nothing to the playback map of `packages/common/runtime/timing.ly`,
  which the engine reproduces and its differential tests hold equal to
  LilyPond's. A feature extends the engine through its interface: a `.ly` file
  compiled ahead of the score, whose Scheme performers write a file of their
  own. What that cannot do is added as a general interface, as engine work of
  its own, never as a feature in the engine (D144).
- Native builds and tests of the engine run on stable (`cargo +stable`); only
  its wasm build uses the pinned nightly (D57).
- Lily Studio's compiler and error parser are a Rust port of the extension's
  (D42); a change to one must be made in the other.
- `packages/vscode/data/completions.json` is generated (`npm run
  gen:completions`); do not edit it.
- Tests that need lilypond skip themselves when it is not installed; a run with
  skipped tests has not verified a change to compiling or parsing.
