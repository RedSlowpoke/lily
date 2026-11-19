// B8 in core.wasm (MS6 step 11): the core returns between its steps and keeps
// to STEP_MS but for the steps it cannot divide, and a newer edit stops a
// compile as the worker sees it, between steps, whenever it arrives.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { coreSkip } from '../../compare/core.mjs'
import { corpus } from '../../corpus/fetch.mjs'
import { coreWasm } from '../run.mjs'
import { STEP_MS, instance, summary, superseded, whole } from '../b8.mjs'

const file = () => corpus().find((row) => row.id === 'T02')?.path
const skip = coreSkip() || (file() ? false : 'no corpus')

test('a compile in core.wasm returns between its steps, and a newer edit stops it', { skip, timeout: 300_000 }, async () => {
  const wasm = await WebAssembly.compile(readFileSync(coreWasm))
  const text = readFileSync(file())
  const edited = Buffer.from(text.toString().replace(/([a-g])'/, "$1''"))
  assert.ok(!edited.equals(text), 'the edit changed the text')

  const run = await whole(wasm, 'score.ly', text)
  assert.ok(run.done, 'it ran to its end')
  assert.ok(run.steps > 1, `it returned: ${run.steps} steps at ${STEP_MS} ms`)

  const trial = await superseded(wasm, 'score.ly', text, edited, 5)
  assert.ok(trial.stopped, 'stopped before its end')
  assert.ok(trial.done, 'the newer compile ran to its end')
  assert.ok(trial.waited >= 0 && trial.waited <= run.longest + 1, `it waited for the step it was in: ${trial.waited} of ${run.longest}`)

  // Stopped, then compiled in the same instance: what a fresh one writes.
  const outputs = (e) => {
    const out = e.lily_alloc(16)
    return Object.fromEntries(Array.from({ length: e.lily_outputs() }, (_, i) => {
      e.lily_output(i, out)
      const [at, length, data, size] = new Uint32Array(e.memory.buffer, out, 4)
      return [Buffer.from(e.memory.buffer, at, length).toString(), Buffer.from(e.memory.buffer, data, size).toString('latin1')]
    }))
  }
  const fresh = await instance(wasm)
  assert.ok(fresh.compile('score.ly', edited, 1).done)
  const stopped = await instance(wasm)
  assert.ok(!stopped.compile('score.ly', text, 1, (at) => at >= 5).done)
  assert.ok(stopped.compile('score.ly', edited, 2).done)
  assert.deepEqual(outputs(stopped.e), outputs(fresh.e))
  assert.ok(summary({ steps: [1, 2], done: true, ms: 3, memory: 2 ** 20 }).MiB === 1)
})

test('a score over the memory ends in a diagnostic, not a trap, and the instance goes on', { skip, timeout: 300_000 }, async () => {
  const wasm = await WebAssembly.compile(readFileSync(coreWasm))
  const outputs = (e) => {
    const out = e.lily_alloc(16)
    return Object.fromEntries(Array.from({ length: e.lily_outputs() }, (_, i) => {
      e.lily_output(i, out)
      const [at, length, data, size] = new Uint32Array(e.memory.buffer, out, 4)
      return [Buffer.from(e.memory.buffer, at, length).toString(), Buffer.from(e.memory.buffer, data, size).toString('latin1')]
    }))
  }
  const pages = (o) => Object.keys(o).filter((name) => name.endsWith('.svg')).length
  const staff = (bars) => Buffer.from(`\\version "2.26.0"\n\\score { \\new Staff { ${"c'8 d' e'4 f'8 g' a'4 | ".repeat(bars)} } \\layout { } }\n`)
  const { e, compile } = await instance(wasm)
  const run = compile('over.ly', staff(800), 1)
  assert.ok(run.done, 'it ended')
  const over = outputs(e)
  assert.match(over.stderr, /needs more memory than the engine may use \(256 MiB\)/)
  assert.equal(pages(over), 0)
  assert.equal(JSON.parse(over['diagnostics.json'])[0].severity, 'error')
  // The same instance, not a new one: a small score compiles as if nothing had happened.
  assert.ok(compile('small.ly', staff(2), 2).done)
  const small = outputs(e)
  assert.equal(small.stderr, '')
  assert.equal(pages(small), 1)
})
