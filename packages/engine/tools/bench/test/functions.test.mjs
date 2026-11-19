// MS2 step 5's function probe (N2, 04 §13): each of the 159 functions of
// LilyPond 2.26.0's ly/music-functions-init.ly applied to small arguments by
// tools/bench/probe-functions.ly, one section a function. Each section is
// run as a file of its own, all of them by one lilypond and by one run of
// the engine's runner (crates/parser/examples/parse.rs --display), so that a
// call that fails touches no other. A section's output, \displayMusic's of
// the call's result, must be LilyPond's, but for the functions in RECORDED,
// whose output differs for a reason of its own; \clef's since LilyPond's
// regex functions (MS2 step 20), those whose bodies are templates since
// step 33 (P11), and the partcombine part's since its GOOPS generics and
// global contexts (MS5 step 6a).
// A function of a part on demand loads the part from source (D74). Those must still
// differ, so that the list is kept true. Procedures print with Guile's
// addresses and files, the engine's with its own: they are compared as
// `#<procedure>`, and a section that holds one with its whitespace
// collapsed, since the length of what pretty-print writes breaks its lines.
// Skipped, and so unverified, without LilyPond 2.26.0 or cargo.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { runExample } from '../../examples.mjs'
import { benchDir, lilyScm } from '../run.mjs'

const lily = lilyScm()
const skip = !lily
  ? 'LilyPond 2.26.0 is not installed'
  : spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-functions-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/**
 * The functions whose output is not LilyPond's, each with why; progress.md,
 * MS2 step 13, lists what they needed then. Those whose bodies are templates
 * give LilyPond's since MS2 step 33 (P11), and those of the partcombine part
 * since MS5 step 6a.
 */
const RECORDED = {
  withRelativeDir: 'lilypond -o names its files absolute; the core names them as the host put them, here "./probe.ly", as lilypond prints without -o (MS2 step 11)',
}

/** probe-functions.ly's header and its sections: [name, text] in order. */
function sections() {
  const text = readFileSync(path.join(benchDir, 'probe-functions.ly'), 'utf8')
  const parts = text.split(/^(?=#\(probe ")/m)
  const header = parts.shift()
  const all = parts.map((part) => [JSON.parse(/^#\(probe ("(?:[^"\\]|\\.)*")\)/.exec(part)[1]), part])
  assert.deepEqual(all.at(-1)?.[0], 'end', 'the probe ends with its end section')
  return { header, sections: all.slice(0, -1) }
}

/** Guile's and the engine's procedures alike, and their lines if a section has one. */
function normalize(output) {
  // A block's last newline is the one the next block's marker begins with.
  const out = output.replace(/#<(?:procedure|program)[^<>]*(?:<[^<>]*>[^<>]*)*>/g, '#<procedure>').trimEnd()
  return out.includes('#<procedure>') ? out.replace(/\s+/g, ' ') : out
}

test('the probe applies each function music-functions-init.ly defines', { skip }, () => {
  const init = readFileSync(path.join(lily, '../../ly/music-functions-init.ly'), 'utf8')
  const defined = [...init.matchAll(/^(?:([A-Za-z][-A-Za-z]*)|"((?:[^"\\]|\\.)*)") *=/gm)]
    .map((m) => m[1] ?? JSON.parse(`"${m[2]}"`).replace(/^\\/, ''))
  const names = sections().sections.map(([name]) => name)
  assert.equal(names.length, FUNCTIONS)
  assert.deepEqual([...names].sort(), [...new Set(defined)].filter((n) => !NOT_FUNCTIONS.includes(n)).sort())
  for (const name of Object.keys(RECORDED)) assert.ok(names.includes(name), `${name} is recorded and probed`)
})

/** What music-functions-init.ly assigns that is no function, and the functions it probes. */
const NOT_FUNCTIONS = ['A', 'B', 'C', 'afterGraceFraction', 'instrument-definitions', 'previous-pitchnames']
const FUNCTIONS = 159

test('each function gives LilyPond 2.26.0\'s \\displayMusic, but those recorded with what they need',
  { skip, timeout: 600_000 }, () => {
    const { header, sections: all } = sections()
    const dir = path.join(scratch, 'functions')
    mkdirSync(dir)
    const files = all.map(([, text], i) => {
      const file = `f${String(i).padStart(3, '0')}.ly`
      writeFileSync(path.join(dir, file), header + text)
      return file
    })

    // One lilypond for all; each file prints its own `=== name` first.
    const lilypond = spawnSync('lilypond', ['-s', '-o', dir, ...files], { cwd: dir, encoding: 'utf8', maxBuffer: 1 << 28 })
    assert.ok(lilypond.status === 0, lilypond.stderr)
    const reference = new Map()
    for (const block of lilypond.stdout.split(/^(?==== )/m).slice(1)) {
      const newline = block.indexOf('\n')
      reference.set(block.slice(4, newline), block.slice(newline + 1))
    }
    assert.equal(reference.size, FUNCTIONS, 'one block a function from lilypond')

    const out = path.join(scratch, 'engine')
    mkdirSync(out)
    const run = runExample('parse', ['--lily', lily, '--display', out, '--out', out, ...files],
    { release: true, cwd: dir, encoding: 'utf8', maxBuffer: 1 << 28 })
    // 1: a file failed, as recorded ones may; 2: the runner could not run.
    assert.ok(run.status === 0 || run.status === 1, run.stderr)

    const wrong = []
    const still = []
    all.forEach(([name], i) => {
      const stem = path.join(out, files[i].replace(/\.ly$/, ''))
      const shown = existsSync(`${stem}.out`) ? readFileSync(`${stem}.out`, 'utf8') : ''
      const errors = existsSync(`${stem}.err`) ? readFileSync(`${stem}.err`, 'utf8') : ''
      const engineBlock = shown.replace(/^\n?=== .*\n/, '')
      const same = normalize(engineBlock) === normalize(reference.get(name) ?? '') && !/error/.test(errors)
      if (RECORDED[name] && same) still.push(name)
      if (!RECORDED[name] && !same) wrong.push(`${name}: ${errors.trim().split('\n').slice(0, 4).join(' | ') || 'output differs'}`)
    })
    if (process.env.PROBE_DUMP) writeFileSync(process.env.PROBE_DUMP, JSON.stringify({ wrong, still }, null, 1))
    assert.deepEqual(wrong, [], 'functions whose output is not LilyPond\'s')
    assert.deepEqual(still, [], 'recorded functions that now give LilyPond\'s output: take them off RECORDED')
  })

test('\\relative\'s octave checks warn as LilyPond 2.26.0 warns', { skip, timeout: 600_000 }, () => {
  // `=` checks the note's own octave (`generic_to_relative_octave`), and
  // \octaveCheck the pitch it gives on (`Relative_octave_check`); both
  // warn at the music's origin.
  const dir = path.join(scratch, 'checks')
  mkdirSync(dir)
  writeFileSync(path.join(dir, 'checks.ly'), `\\version "2.26.0"
x = \\relative { c' d =,4 e }
y = \\relative { c' \\octaveCheck d, e }
`)
  const lilypond = spawnSync('lilypond', ['--loglevel=WARNING', '-o', dir, 'checks.ly'], { cwd: dir, encoding: 'utf8' })
  assert.equal(lilypond.status, 0, lilypond.stderr)
  assert.equal(lilypond.stderr.split('warning:').length, 3, 'two warnings')
  const run = runExample('parse', ['--lily', lily, 'checks.ly'], { release: true, cwd: dir, encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  // lilypond names a warning's file by its real path.
  assert.equal(run.stderr, lilypond.stderr.replace(/^.*\/(checks\.ly:)/gm, '$1'))
})
