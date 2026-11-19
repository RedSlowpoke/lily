// C0, the diagnostics (chapter 00 §5.2, D63, D64): stderr.mjs gives what the
// extension's parseStderr gave on the error corpus (the fixture
// gen-stderr-fixture.mjs wrote); each .err scores against itself and fails
// perturbed in a message, a location or a count; --parse-only leaves out only
// the processing warnings of cases 17, 18 and 30. T03's stderr is scored
// against itself when the corpus has been captured, and since MS2 step 40
// the engine's against it, through the core natively and in core.wasm: its
// parser's errors, then what its \layout and \midi translations say (D99),
// then failed files, LilyPond's byte for byte. So is the stderr of
// fixtures/scores.ly, two scores translated in the file's order, against
// LilyPond's with capture.ly. Those tests skip themselves, and so verify
// nothing, without LilyPond 2.26.0, cargo or the corpus's capture, and the
// wasm test without a core.wasm as new as the sources.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, describe, test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { captureOne, lilypondVersion } from '../capture.mjs'
import { compileInWasm, coreSkip } from '../core.mjs'
import { FIXTURE, diagnose } from '../gen-stderr-fixture.mjs'
import { PROCESSING, compareErrors, compareStderr, diagnostics, key } from '../score.mjs'
import { ERRORS, displayWidth, errorCases, parseStderr } from '../stderr.mjs'

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-c0-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

const cases = errorCases()
const err = (id) => readFileSync(path.join(ERRORS, `${id}.err`), 'utf8')
// A header line as parseStderr reads one, and the failed-files summary it drops beside errors.
const HEADER = /^(?:(.+?):(\d+)(?::(\d+))?: )?(fatal error|programming error|error|warning): (.*)$/
const SUMMARY = /^fatal error: failed files: /

// The stderr's lines, with the indices of the headers C0 counts.
function headers(text) {
  const lines = text.split('\n')
  return { lines, at: lines.flatMap((l, i) => HEADER.test(l) && !SUMMARY.test(l) ? [i] : []) }
}

describe('the error corpus', () => {
  test('is read through @lily/common\'s exports', () => {
    assert.equal(cases.length, 30)
    assert.ok(ERRORS.endsWith(path.join('packages', 'common', 'test', 'parser', 'errors')), ERRORS)
    for (const id of cases) assert.ok(existsSync(path.join(ERRORS, `${id}.ly`)), id)
  })

  test('no tool reaches into another package by a relative path', () => {
    const tools = path.dirname(path.dirname(path.dirname(fileURLToPath(import.meta.url))))
    const scripts = readdirSync(tools, { recursive: true }).filter((f) => f.endsWith('.mjs') && !f.includes('node_modules'))
    for (const f of scripts) {
      assert.doesNotMatch(readFileSync(path.join(tools, f), 'utf8'), /['"`][^'"`\n]*\.\.\/(?:\.\.\/)*(?:packages\/)?(?:common|vscode|studio)\//, f)
    }
  })
})

describe('stderr.mjs against the extension\'s parseStderr', () => {
  const fixture = JSON.parse(readFileSync(FIXTURE, 'utf8'))

  test('the fixture holds every case', () => {
    assert.deepEqual(Object.keys(fixture.cases), cases)
  })

  test('gives what the extension gave on each .err', () => {
    for (const id of cases) assert.deepEqual(diagnose(parseStderr, id), fixture.cases[id], id)
  })

  test('keeps what the corpus exercises', () => {
    const all = Object.values(fixture.cases).flat()
    assert.equal(all.length, 49)
    // Guile's text after the excerpt (22), the search path before it (07), a string's origin (25).
    assert.match(fixture.cases['22-scheme-eval'][0].message, /\nIn procedure car: Wrong type \(expecting pair\): \(\)$/)
    assert.match(fixture.cases['07-include-missing'][0].message, /\n\(search path: /)
    assert.deepEqual(fixture.cases['25-include-string'].map((d) => [d.line, d.column, d.message.split('\n')[1]]),
      [[1, undefined, '(in <included string>, line 1, column 6)'], [1, undefined, '(in <included string>, line 1, column 6)']])
    // The failed-files summary is dropped beside errors and never stands alone in the corpus.
    assert.ok(all.every((d) => !d.message.startsWith('failed files: ')))
    // A tab in the excerpt (26) and UTF-8 before the column (27) are cut as the extension cuts them.
    assert.equal(fixture.cases['26-tab-column'][0].message, 'unknown command: `\\foo\'')
    assert.equal(fixture.cases['27-utf8-column'][0].message, 'unknown command: `\\foo\'')
  })
})

describe('C0 on the error corpus', () => {
  const write = (dir, id, text) => writeFileSync(path.join(dir, `${id}.err`), text)
  const copies = (name, perturb) => {
    const dir = mkdtempSync(path.join(scratch, `${name}-`))
    for (const id of cases) write(dir, id, perturb(err(id), id))
    return dir
  }
  const failing = (dir, options) => compareErrors(dir, options).filter((r) => r.pass.C0).map((r) => r.id)

  test('each .err against itself passes, with and without --parse-only', () => {
    const dir = copies('self', (text) => text)
    for (const parseOnly of [false, true]) {
      const results = compareErrors(dir, { parseOnly })
      assert.equal(results.length, 30)
      for (const r of results) assert.deepEqual([r.pass.C0, r.all], [true, 1], r.id)
    }
  })

  test('each fails with its first message changed', () => {
    const dir = copies('message', (text) => {
      const { lines, at } = headers(text)
      lines[at[0]] += ' (changed)'
      return lines.join('\n')
    })
    assert.deepEqual(failing(dir), [])
  })

  test('each fails with its first line or column moved', () => {
    for (const field of [2, 3]) {
      const dir = copies(`location-${field}`, (text) => {
        const { lines, at } = headers(text)
        const m = HEADER.exec(lines[at[0]])
        // A header with no column (25 in the corpus) moves its line instead.
        const f = m[field] === undefined ? 2 : field
        lines[at[0]] = lines[at[0]].replace(`:${m[2]}${m[3] === undefined ? '' : `:${m[3]}`}: `,
          f === 2 ? `:${+m[2] + 1}${m[3] === undefined ? '' : `:${m[3]}`}: ` : `:${m[2]}:${+m[3] + 1}: `)
        // A column moved moves its excerpt with it, so that only the column differs.
        const indent = +m[3] - 1
        const end = at[1] ?? lines.length
        for (let i = at[0] + 1; f === 3 && i + 1 < end; i++) {
          if (displayWidth(lines[i]) === indent && lines[i + 1].length >= indent && lines[i + 1].slice(0, indent).trim() === '') {
            lines[i] += ' '; lines[i + 1] = ' ' + lines[i + 1]
            break
          }
        }
        return lines.join('\n')
      })
      const results = compareErrors(dir)
      assert.deepEqual(results.filter((r) => r.pass.C0).map((r) => r.id), [], `field ${field}`)
      // Each first diagnostic still has its message, so the location alone fails it; an
      // included string's location is in its message (25), where parseStderr puts it.
      for (const r of results.filter((r) => r.first[0].column !== undefined)) {
        assert.equal(r.first[1].message, r.first[0].message, `${r.id} field ${field}`)
      }
      assert.equal(results.filter((r) => r.first[0].column === undefined).length, 1)
    }
  })

  test('each fails with one diagnostic more or one fewer', () => {
    const more = copies('more', (text) => {
      const { lines, at } = headers(text)
      return [...lines.slice(0, at.at(-1) + 1), lines[at.at(-1)] + ' again', ...lines.slice(at.at(-1) + 1)].join('\n')
    })
    assert.deepEqual(failing(more), [])
    const fewer = copies('fewer', (text) => {
      const { lines, at } = headers(text)
      const next = lines.findIndex((l, i) => i > at.at(-1) && HEADER.test(l))
      return [...lines.slice(0, at.at(-1)), ...(next < 0 ? [] : lines.slice(next))].join('\n')
    })
    assert.deepEqual(failing(fewer), [])
  })

  test('a candidate missing fails', () => {
    const r = compareErrors(path.join(scratch, 'none'), { cases: ['01-unknown-command'] })
    assert.deepEqual(r, [{ id: '01-unknown-command', missing: true, pass: { C0: false } }])
  })
})

describe('--parse-only', () => {
  // A case's stderr without the warnings of processing, as a parser alone would write it.
  const parsed = (id) => {
    const { lines } = headers(err(id))
    const out = []; let skip = false
    for (const line of lines) {
      const m = HEADER.exec(line)
      if (m) skip = m[4] === 'warning' && PROCESSING.has(m[5])
      if (!skip) out.push(line)
    }
    return out.join('\n')
  }

  test('leaves out only the processing warnings, in cases 17, 18 and 30', () => {
    const cut = cases.filter((id) => diagnostics(err(id), { parseOnly: true }).length !== diagnostics(err(id)).length)
    assert.deepEqual(cut, ['17-function-argument', '18-optional-argument', '30-unattached'])
    assert.deepEqual(diagnostics(err('30-unattached'), { parseOnly: true }).map((d) => [d.line, d.column, d.message]),
      [[2, 3, 'Unattached ArticulationEvent'], [3, 24, 'Unattached ArticulationEvent']])
  })

  test('scores a parser\'s stderr as passing only with it', () => {
    for (const id of ['17-function-argument', '18-optional-argument', '30-unattached']) {
      const rootFile = `${id}.ly`
      assert.equal(compareStderr(err(id), parsed(id), { rootFile }).pass.C0, false, id)
      assert.equal(compareStderr(err(id), parsed(id), { rootFile, parseOnly: true }).pass.C0, true, id)
    }
    // 17's error count is 1 either way; its warnings are 2 against 0 without --parse-only.
    const r = compareStderr(err('17-function-argument'), parsed('17-function-argument'))
    assert.deepEqual([r.errors, r.warnings], [[1, 1], [2, 0]])
  })

  test('still counts the parser\'s own warnings', () => {
    const { lines, at } = headers(parsed('30-unattached'))
    const one = [...lines.slice(0, at[1]), ...lines.slice(at[1] + 3)].join('\n')
    const r = compareStderr(err('30-unattached'), one, { parseOnly: true })
    assert.deepEqual([r.pass.C0, r.warnings], [false, [2, 1]])
  })
})

const t03 = path.join(corpusDir(), 'ref', 'T03', 'stderr.txt')
describe('C0 on T03', { skip: !existsSync(t03) && 'the corpus has not been captured' }, () => {
  test('its stderr against itself passes, and its errors are counted', () => {
    const text = readFileSync(t03, 'utf8')
    const r = compareStderr(text, text)
    assert.ok(r.pass.C0 && r.errors[0] > 0, JSON.stringify(r))
    const { lines, at } = headers(text)
    lines[at[0]] = lines[at[0]].replace(': error: ', ': warning: ')
    assert.equal(compareStderr(text, lines.join('\n')).pass.C0, false)
  })
})

const lilypond = lilypondVersion() === '2.26.0' ? false : 'LilyPond 2.26.0 is not installed'
const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'

/** The core's compile example on `file`, from its directory: its outputs' directory. */
function compileNatively(file) {
  const out = mkdtempSync(path.join(scratch, 'engine-'))
  const run = runExample('compile', ['--out', out, file], { cwd: path.dirname(file), encoding: 'utf8' })
  // 1: the file failed, as T03 does in LilyPond.
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  return out
}

describe('C0 on T03 through the core', () => {
  const skip = lilypond || cargo || (!existsSync(t03) && 'the corpus has not been captured')
  const file = () => corpus().find((row) => row.id === 'T03').path
  const reference = () => readFileSync(t03, 'utf8')

  test('its stderr natively is LilyPond\'s, the translations\' warnings after the parser\'s errors', { skip, timeout: 600_000 }, () => {
    const out = compileNatively(file())
    const stderr = readFileSync(path.join(out, 'broken.err'), 'utf8')
    assert.equal(stderr, reference())
    const r = compareStderr(reference(), stderr)
    assert.ok(r.pass.C0, JSON.stringify(r))
    // The bar check that fails in its \layout translation and again in its \midi.
    assert.equal(stderr.split('warning: bar check failed at: 3/4').length - 1, 2)
  })

  test('core.wasm gives the same stderr', { skip: skip || coreSkip(), timeout: 600_000 }, async () => {
    const [result] = await compileInWasm([file()])
    assert.equal(result.outputs.stderr.toString(), reference())
  })
})

test('two scores are translated in the file\'s order, and what they say is LilyPond\'s', { skip: lilypond || cargo, timeout: 600_000 }, async () => {
  const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../fixtures/scores.ly')
  const ly = path.join(scratch, 'scores-ly')
  const result = await captureOne(process.env.LILYPOND ?? 'lilypond', { id: 'scores', path: file }, ly)
  assert.equal(result.exit, 0)
  const out = compileNatively(file)
  const stderr = readFileSync(path.join(out, 'scores.err'), 'utf8')
  assert.equal(stderr, readFileSync(path.join(ly, 'stderr.txt'), 'utf8'))
  // The first bar check met by the expected warning; the \midi's that
  // cannot create Solo.
  assert.equal(stderr.split('warning: bar check failed').length - 1, 3)
  assert.match(stderr, /warning: cannot create context: Solo\n/)
  // The performances in the file's order. timing.ly reads each staff's
  // voices when it writes the file, after the last performance cleared
  // them, so only the bars and events of the first are compared.
  const maps = [ly, out].map((dir) => JSON.parse(readFileSync(path.join(dir, 'scores.timing.json'), 'utf8')))
  const played = (map) => map.map((p) => ({ bars: p.bars, events: p.events.map((e) => ({ ...e, href: key(e.href) })) }))
  assert.deepEqual(played(maps[1]), played(maps[0]))
  assert.deepEqual(maps[1][1].staves, maps[0][1].staves)
  assert.deepEqual(maps[1].map((p) => p.events[0].href.split(':').at(-3)), ['10', '14'])
})
