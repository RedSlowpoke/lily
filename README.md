# Lily

[LilyPond](https://lilypond.org) tools, in one repository:

- **[Lily for VS Code](packages/vscode)**: highlighting, a live SVG preview,
  MIDI playback, errors in the Problems panel, completion and hover, and
  `lily-check`, a headless checker for coding agents.
- **[Lily Studio](packages/studio)**: a small Mac app for LilyPond scores, for
  people who do not program. Its Stems workspace splits a song into stems with
  RoFormer MLX and starts a score for any of them.
- **[packages/separate](packages/separate)** and
  **[packages/transcribe](packages/transcribe)**: the Python that Lily Studio
  runs with uv to split a song into stems and to turn a stem into notes.
- **[packages/common](packages/common)**: what the two share: the grammar, the
  preview and MIDI player scripts, lilypond's runtime files, and the types and
  spans of diagnostics.

## Development

An npm workspace; each package's README has its own commands.

```sh
npm install            # once, for all packages
npm run check-types    # tsc in every package
npm run lint           # oxlint, warnings fail
npm test               # grammar snapshots, the extension's tests, the studio's tests
```

[AGENTS.md](AGENTS.md) describes the layout and its rules, and
[docs/DECISIONS.md](docs/DECISIONS.md) records why things are as they are.

## License

MIT; the text is in the [LICENSE](LICENSE) file. The exception is
[packages/engine](packages/engine), the LilyPond engine in WebAssembly, which
ports GNU LilyPond and is GPL-3.0-or-later
([packages/engine/LICENSE](packages/engine/LICENSE)). The apps do not link it;
they reach it only through a worker protocol.
