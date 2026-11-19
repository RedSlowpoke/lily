// B6 in core.wasm (MS6 step 12): an edit's phases, as b6.html reads them on a
// device, and edits in one instance keep to what the first few made.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { coreSkip } from '../../compare/core.mjs'
import { corpus } from '../../corpus/fetch.mjs'
import { phases, measure } from '../b6.mjs'
import { coreWasm } from '../run.mjs'

const skip = coreSkip() || (corpus().find((row) => row.id === 'T02') ? false : 'no corpus')

test("lsb_times' values are README §4.3's phases, a translation's layout taken out of it", () => {
  assert.deepEqual(phases([1, 10, 1, 9, 4, 2, 3, 99, 0, 5, 0, 0]), { parse: 1, translation: 6, layout: 6, svg: 3, midi: 5 })
})

test('E1–E3 of T02 in one instance, by phase', { skip, timeout: 300_000 }, async () => {
  const wasm = await WebAssembly.compile(readFileSync(coreWasm))
  const { rows, MiB } = await measure(wasm, 'T02', 2)
  assert.deepEqual(rows.map((row) => row.edit), ['E1', 'E2', 'E3'])
  for (const row of rows) {
    assert.ok(row.engine >= row.parse + row.translation, `${row.edit}: the engine's ms hold its phases`)
    assert.ok(row.layout > 0 && row.svg > 0, `${row.edit}: laid out and written`)
  }
  assert.ok(MiB < 64, `T02 within B4's Must: ${MiB} MiB`)
})
