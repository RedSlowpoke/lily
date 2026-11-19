// Which of the engine's files a score needs, and where they are kept (chapter
// 08 of docs/research/wasm-engine). Neither the DOM nor a worker: sw.ts, the
// worker and the tests all use it.
import type { EngineManifest } from './protocol'

/** The Cache Storage cache of the engine's files, shared by its worker and sw.ts. */
export const CACHE = 'lily-engine'

/** Where `esbuild.mjs --site` puts the engine's files, beside the page. */
export const ENGINE_DIR = 'engine/'

/**
 * What a source names that a module of the engine draws, writes or runs: a
 * Scheme part (chapter 03 §6), code split from the core (chapter 05 §5.10) or
 * a font pack (chapter 06 §5.2). A trigger only fetches early: a module the
 * scan misses is asked for by the engine, at the cost of a restart.
 */
const TRIGGERS: [RegExp, string[]][] = [
  [/\\(chordmode|chords)\b|\bChordNames\b/, ['chords', 'chord-names']],
  [/\b(TabStaff|TabVoice|FretBoards)\b|\\(fret-diagram|fret-diagram-terse|fret-diagram-verbose)\b/, ['fretted', 'tablature']],
  [/\\(woodwind-diagram|harp-pedal|discant|stdBass|freeBass)\b/, ['diagrams']],
  [/\\(partCombine|autoChange)\b/, ['partcombine']],
  [/\\(modalTranspose|modalInversion|retrograde|inversion)\b/, ['modal']],
  // articulate.ly uses (lily display-lily).
  [/\\(displayLilyMusic|displayMusic|displayScheme)\b|\\include\s+"articulate\.ly"/, ['display']],
  [/\\qr-code\b/, ['qr']],
  [/\\(sustainOn|sostenutoOn|unaCorda)\b/, ['pedals']],
  [/\\repeat\s+percent\b/, ['percent']],
  [/\bAmbitus_engraver\b/, ['ambitus']],
  // The brace's four parts (W5), in order.
  [/\b(PianoStaff|GrandStaff|SystemStartBrace)\b/, ['emmentaler-brace-0', 'emmentaler-brace-1', 'emmentaler-brace-2', 'emmentaler-brace-3']],
  // Grace and cue notes and small staves are drawn at font size -3 or so: Emmentaler-14 at staff size 20.
  [/\\(grace|acciaccatura|appoggiatura|slashedGrace|afterGrace|small|teeny|tiny|cueDuring)\b/, ['emmentaler-14']],
  // OPEN E22 and C5 (D263): the fonts of text grobs, which the core's prediction skips (their font is their
  // markup's), and the outlines a skyline needs of a text face (D262), each else a stop that pays the
  // translation again. Figures and fingerings at font size -5 (Emmentaler-11), a volta's number at -4 (-13);
  // a lyric, a text script or a markup on a note in Roman; a tempo's text in Bold; cresc. and dim. text, a
  // clef's 8 and a tab's numbers in Italic; an ottava's 8va in Bold Italic, whose face no pack preloads.
  // Measured on the 42 and the fixtures (MS7 step 7's predict.mjs).
  [/\\(figuremode|figures)\b|\bFiguredBass\b/, ['emmentaler-11']],
  [/[a-gs](?:is|es)*[',]*\d*\.*-[1-5]\b|\\finger\b/, ['emmentaler-11']],
  [/\\(alternative|volta)\b/, ['emmentaler-13']],
  [/\\(lyricmode|addlyrics|lyricsto)\b|\bLyrics\b|[_^-]\s*("|\\markup\b)/, ['C059-Roman.outlines']],
  [/\\tempo\s*("|\\markup\b)/, ['C059-Bold.outlines']],
  [/\\(cresc|decresc|dim)\b|\\clef\s*"?[a-zA-Z]+[_^]|\b(TabStaff|TabVoice)\b/, ['C059-Italic.outlines']],
  [/\\ottava\b/, ['C059-BdIta', 'C059-BdIta.outlines']],
  // A markup's own faces: bold, italic, both somewhere (bold italic, likely), sans and sans bold.
  [/\\bold\b/, ['C059-Bold.outlines']],
  [/\\italic\b/, ['C059-Italic.outlines']],
  [/^(?=[\s\S]*\\bold\b)(?=[\s\S]*\\italic\b)/, ['C059-BdIta', 'C059-BdIta.outlines']],
  [/\\sans\b/, ['NimbusSans-Regular', 'NimbusSans-Regular.outlines']],
  [/^(?=[\s\S]*\\sans\b)(?=[\s\S]*\\bold\b)/, ['NimbusSans-Bold', 'NimbusSans-Bold.outlines']],
  [/\\clef\s*"?tab"?(\s|$)/, ['NimbusSans-Bold', 'NimbusSans-Bold.outlines']],
  // "New Century Schoolbook" is LilyPond's TeX Gyre Schola, fontconfig's metric alias (OPEN E23, D271).
  [/New Century Schoolbook|TeX Gyre Schola/i, ['TeXGyreSchola-Regular', 'TeXGyreSchola-Regular.outlines']],
]

/** Emmentaler's optical sizes (chapter 06 §5.1). */
const OPTICAL = [11, 13, 14, 16, 18, 20, 23, 26]

/** The pack LilyPond would draw a staff of `size` points with: the nearest optical size. */
export function packFor(size: number): string {
  const nearest = OPTICAL.reduce((a, b) => (Math.abs(b - size) < Math.abs(a - size) ? b : a))
  return `emmentaler-${nearest}`
}

/** The modules of `manifest` that `source` names, in the order of the triggers. */
export function modulesFor(source: string, manifest: EngineManifest): string[] {
  const names = new Set<string>()
  for (const [pattern, modules] of TRIGGERS) if (pattern.test(source)) modules.forEach((name) => names.add(name))
  for (const match of source.matchAll(/set-global-staff-size\s+(\d+(?:\.\d+)?)/g)) names.add(packFor(Number(match[1])))
  // LilyPond's own ly/ files are modules of the engine; a score's own includes are asked of the page.
  for (const match of source.matchAll(/\\include\s+"([^"]+)"/g)) names.add(`ly/${match[1]}`)
  return [...names].filter((name) => name in manifest.modules && !manifest.core.preload.includes(name))
}

/** Every file of `manifest`, relative to the engine's directory: what the cache may keep. */
export function manifestFiles(manifest: EngineManifest): string[] {
  const debug = manifest.core.debug ? [manifest.core.debug] : []
  return [manifest.core.wasm, manifest.core.image, ...debug, ...Object.values(manifest.modules).map((module) => module.file)]
}
