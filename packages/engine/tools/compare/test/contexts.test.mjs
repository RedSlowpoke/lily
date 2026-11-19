// N3's context definitions (MS2 step 15, 04 §7): $defaultlayout's and
// $defaultmidi's, which engraver-init.ly and performer-init.ly make inside
// declarations-init.ly's \layout and midi-init.ly's \midi, as the engine
// makes them (crates/parser/examples/parse.rs, after the whole init) against LilyPond
// 2.26.0's, both dumped by contexts.ly. Since step 16 none differs: the
// image holds what engraver-init.ly stored of the fretted part (D78).
// Skipped, and so unverified, without LilyPond 2.26.0 or cargo.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { test } from 'node:test'
import { lilypondVersion } from '../capture.mjs'
import { compareContexts, engineContexts, lilypondContexts, readContexts } from '../contexts.mjs'

const lilypond = lilypondVersion() === '2.26.0' ? false : 'LilyPond 2.26.0 is not installed'
const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'

test("the engine's context definitions equal LilyPond's", { skip: lilypond || cargo, timeout: 600_000 }, () => {
  const [reference, got] = [lilypondContexts(), engineContexts()]
  assert.equal(reference.status, 0, reference.stderr)
  assert.equal(reference.stderr, '')
  assert.equal(got.status, 0, got.stderr)
  assert.equal(got.stderr, '', "the init files' and contexts.ly's messages")
  const defs = readContexts(reference.stdout)
  // engraver-init.ly's 43 and performer-init.ly's 36.
  assert.equal([...defs.keys()].filter((head) => head.startsWith('layout ')).length, 43)
  assert.equal([...defs.keys()].filter((head) => head.startsWith('midi ')).length, 36)
  const result = compareContexts(reference.stdout, got.stdout)
  assert.deepEqual(result.definitions, [79, 79])
  assert.deepEqual(result.differ, [])
})

test('compareContexts names the definition and the lines that differ', () => {
  const a = 'layout Staff e.ly:1:1\n  context-name Staff\n  property-ops 1\n    (assign x 1)\nmidi Staff p.ly:2:1\n  context-name Staff\n'
  const b = a.replace('(assign x 1)', '(assign x 2)')
  assert.deepEqual(compareContexts(a, a), { equal: true, definitions: [2, 2], differ: [] })
  assert.deepEqual(compareContexts(a, b).differ, [{ definition: 'layout Staff', expected: '(assign x 1)', got: '(assign x 2)' }])
  const c = a.replace('midi Staff p.ly:2:1\n  context-name Staff\n', '')
  assert.deepEqual(compareContexts(a, c).differ, [
    { definition: 'midi Staff', expected: 'midi Staff p.ly:2:1', got: null },
    { definition: 'midi Staff', expected: 'context-name Staff', got: null },
  ])
})
