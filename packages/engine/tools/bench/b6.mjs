// B6 in core.wasm (MS6 step 12, README §4.3; 00 §6.2): T02, T12, T24 and T29,
// each edit E1–E3 from the score's own text, in one instance as a person
// edits, driven as the studio's driver drives it, for a preview, by phase.
//
//   node tools/bench/b6.mjs [--runs <n>] [<id>…]
//
// A run is the text compiled (not timed), then the edit: the first of each
// edit's runs is dropped (00 §6.1) and `--runs` are kept, 20 by default. The
// figures are medians. The phases are README §4.3's as lsb_times gives them;
// its Scheme row is inside the others (a callback is timed where it runs):
//
//   parse         the parse and re-parse
//   translation   the translations, their layout taken out
//   layout        the Paper_scores processed and broken, and the pages' stencils
//   svg           the writer
//   midi          the \midi file, in no row of §4.3 (README §4.4 has its own)
//   engine        lily_put to the compile's end, on the host's clock
//
// The page's row needs a browser: crates/spike/harness/b6.html measures all of
// them on a device.
import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'
import { EDITED, e1e3 } from '../compare/edits.mjs'
import { instance } from './b8.mjs'
import { coreWasm } from './run.mjs'

/** README §4.3's Must and Goal of each phase, ms on the Android reference; E2 twice the engine's, iOS 0.6 of each. */
export const MUST = { S: { parse: 20, translation: 25, layout: 120, svg: 20, page: 50, total: 250 },
  L: { parse: 40, translation: 50, layout: 240, svg: 40, page: 50, total: 500 } }
export const GOAL = { S: { parse: 8, translation: 8, layout: 40, svg: 7, page: 25, total: 100 },
  L: { parse: 15, translation: 20, layout: 80, svg: 15, page: 30, total: 200 } }
/** The score classes of chapter 00. */
export const CLASS = { T02: 'S', T12: 'S', T24: 'L', T29: 'L' }

/** `lsb_times`' twelve values as B6's phases. */
export function phases(values) {
  const [parse, translate, , , layOut, stencils, svg, , , midi] = values
  return { parse, translation: translate - layOut, layout: layOut + stencils, svg, midi }
}

export const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ?? 0
const round = (x) => Math.round(x * 10) / 10

/** Each edit's runs of one score in a fresh instance. */
export async function measure(wasm, id, runs) {
  const { e, compile } = await instance(wasm)
  const edits = e1e3(id)
  const file = edits[0].file
  const text = readFileSync(file)
  let revision = 1
  compile(file, text, revision++)
  const rows = []
  for (const edit of edits) {
    const kept = []
    for (let run = 0; run <= runs; run++) {
      compile(file, text, revision++)
      const { ms, done } = compile(file, Buffer.from(edit.source), revision++)
      if (!done) throw new Error(`${id} ${edit.name} did not end`)
      const times = [...new Float64Array(e.memory.buffer, e.lsb_times(), 12)]
      if (run > 0) kept.push({ engine: ms, ...phases(times) })
    }
    const row = { id, edit: edit.name }
    for (const phase of Object.keys(kept[0])) row[phase] = round(median(kept.map((k) => k[phase])))
    row.worst = round(Math.max(...kept.map((k) => k.engine)))
    rows.push(row)
  }
  return { rows, MiB: round(e.memory.buffer.byteLength / 2 ** 20) }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const args = process.argv.slice(2)
  const at = args.indexOf('--runs')
  const runs = at < 0 ? 20 : Number(args.splice(at, 2)[1])
  const wasm = await WebAssembly.compile(readFileSync(coreWasm))
  for (const id of args.length ? args : EDITED) {
    const { rows, MiB } = await measure(wasm, id, runs)
    for (const row of rows) console.log(JSON.stringify(row))
    console.log(JSON.stringify({ id, MiB }))
  }
}
