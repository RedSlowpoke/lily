import * as assert from 'node:assert/strict'
import { afterEach, beforeEach, describe, test } from 'node:test'
import { cellsOf, Driver, framed, outcomeOf, performanceOf } from '../src/web/engine/driver'
import { engineByDefault, engineMode, startEngine } from '../src/web/engine/engine'
import { EngineHost, type EngineWorker, withPerformance } from '../src/web/engine/host'
import { manifestFiles, modulesFor, packFor } from '../src/web/engine/modules'
import { type EngineManifest, type FromEngine, Need, type PageCells, type PageUpdate, Step, type ToEngine } from '../src/web/engine/protocol'

// Runs under `node --test` from out/test/ (npm run test:unit in packages/studio/).
// The engine's host on the web (chapter 08 of docs/research/wasm-engine), with
// a scripted engine in place of the wasm, which does not exist yet.

const MANIFEST: EngineManifest = {
  version: 1,
  core: { wasm: 'core.1.wasm', image: 'core.1.image', preload: ['emmentaler-20'] },
  source: { repository: 'https://example.org/lily', commit: 'abc123', dirty: false },
  licenses: { notice: 'NOTICE.txt', engine: 'LICENSE.txt' },
  modules: {
    'emmentaler-20': { kind: 'pack', file: 'emmentaler-20.1.pack' },
    'emmentaler-14': { kind: 'pack', file: 'emmentaler-14.1.pack' },
    'emmentaler-16': { kind: 'pack', file: 'emmentaler-16.1.pack' },
    chords: { kind: 'part', file: 'chords.1.part' },
    'chord-names': { kind: 'split', file: 'chord-names.1.wasm' },
    tablature: { kind: 'split', file: 'tablature.1.wasm' },
    'ly/articulate.ly': { kind: 'file', file: 'ly-articulate.1.ly' },
    pdf: { kind: 'wasm', file: 'pdf.1.wasm' },
    'C059-Roman.otf': { kind: 'font', file: 'C059-Roman.1.otf' },
  },
}

describe('modulesFor', () => {
  test('fetches with the core what a score names', () => {
    const source = '\\include "articulate.ly"\n\\new ChordNames \\chordmode { c1 }\n\\new TabStaff { \\grace c8 d1 }'
    assert.deepEqual(modulesFor(source, MANIFEST), ['chords', 'chord-names', 'tablature', 'emmentaler-14', 'ly/articulate.ly'])
  })

  test('the pack of a staff size, but not one the core has, nor a module the build lacks', () => {
    assert.deepEqual(modulesFor('#(set-global-staff-size 15.87)', MANIFEST), ['emmentaler-16'])
    assert.deepEqual(modulesFor('#(set-global-staff-size 20)', MANIFEST), [])
    assert.deepEqual(modulesFor('\\partCombine \\a \\b \\include "my.ily"', MANIFEST), [])
  })

  test('the fonts and outlines of a score’s text, which the core asks for only once it meets them (OPEN C5, E22)', () => {
    const names = ['emmentaler-11', 'emmentaler-13', 'emmentaler-brace-0', 'emmentaler-brace-1', 'emmentaler-brace-2', 'emmentaler-brace-3',
      'C059-Roman.outlines', 'C059-Bold.outlines', 'C059-Italic.outlines', 'C059-BdIta', 'C059-BdIta.outlines',
      'NimbusSans-Regular', 'NimbusSans-Regular.outlines', 'NimbusSans-Bold', 'NimbusSans-Bold.outlines', 'display']
    const manifest: EngineManifest = { ...MANIFEST, modules: { ...MANIFEST.modules, ...Object.fromEntries(names.map((name) => [name, { kind: 'pack', file: `${name}.1.pack` }])) } }
    const of = (source: string) => modulesFor(source, manifest)
    assert.deepEqual(of('\\new PianoStaff << { c } >>'), ['emmentaler-brace-0', 'emmentaler-brace-1', 'emmentaler-brace-2', 'emmentaler-brace-3'])
    assert.deepEqual(of('\\addlyrics { la }'), ['C059-Roman.outlines'])
    assert.deepEqual(of('c4^"dolce" d-1'), ['emmentaler-11', 'C059-Roman.outlines'])
    assert.deepEqual(of('\\tempo "Allegro" 4 = 120'), ['C059-Bold.outlines'])
    assert.deepEqual(of('c\\cresc d\\!'), ['C059-Italic.outlines'])
    assert.deepEqual(of('\\ottava #1 c'), ['C059-BdIta', 'C059-BdIta.outlines'])
    assert.deepEqual(of('\\repeat volta 2 { c } \\alternative { { d } { e } }'), ['emmentaler-13'])
    assert.deepEqual(of('\\markup \\sans \\bold "A"'), ['C059-Bold.outlines', 'NimbusSans-Regular', 'NimbusSans-Regular.outlines', 'NimbusSans-Bold', 'NimbusSans-Bold.outlines'])
    assert.deepEqual(of('\\include "articulate.ly"'), ['display', 'ly/articulate.ly'])
    assert.deepEqual(of('{ c4 d e f }'), [], 'nothing for music alone')
  })

  test('packFor takes the nearest optical size', () => {
    assert.equal(packFor(10), 'emmentaler-11')
    assert.equal(packFor(17.82), 'emmentaler-18')
    assert.equal(packFor(30), 'emmentaler-26')
  })

  test('manifestFiles lists every file the cache may keep', () => {
    assert.equal(manifestFiles(MANIFEST).length, 2 + Object.keys(MANIFEST.modules).length)
    const debug = { ...MANIFEST, core: { ...MANIFEST.core, debug: 'core-debug.1.wasm' } }
    assert.ok(manifestFiles(debug).includes('core-debug.1.wasm'))
  })
})

const bytes = (text: string) => new TextEncoder().encode(text)

describe('outcomeOf', () => {
  test('the outputs as the renderer takes a compile', () => {
    const diagnostics = [{ file: '/s/score.ly', line: 3, column: 5, severity: 'error', message: 'syntax error' }]
    const outputs = new Map([
      ['stderr', bytes('score.ly:3:5: error: syntax error\n')],
      ['diagnostics.json', bytes(JSON.stringify(diagnostics))],
      ['score-1.svg', bytes('<svg>1</svg>')],
      ['score-2.svg', bytes('<svg>2</svg>')],
      ['score.midi', new Uint8Array([0x4d, 0x54])],
      ['score.timing.json', bytes('[{"events":[],"bars":[],"staves":[]}]')],
    ])
    const outcome = outcomeOf('/s/score.ly', outputs, 12)
    assert.equal(outcome.state, 'failed')
    assert.deepEqual(outcome.diagnostics, diagnostics)
    assert.equal(outcome.errorCount, 1)
    assert.deepEqual(outcome.svg, ['<svg>1</svg>', '<svg>2</svg>'])
    assert.deepEqual(outcome.midi, ['score.midi'])
    assert.deepEqual([...(outcome.midiData ?? [])], [0x4d, 0x54])
    assert.deepEqual(outcome.timing, { events: [], bars: [] })
  })

  test('a page sent by its cells: by its place among the pages, and no page of the outcome', () => {
    const outputs = new Map([
      ['score-1.svg', bytes('<svg>1</svg>')],
      ['score-2.svg', bytes('<svg>2</svg>')],
      ['score-2.cells', bytes('00ff:1.5 aa11:-.25\n<g data-lily-cell="aa11" transform="translate(0, -.25)">\n</g>\n')],
    ])
    assert.deepEqual(cellsOf(7, outputs), [
      {
        type: 'cells',
        revision: 7,
        page: 1,
        cells: [
          { digest: '00ff', y: '1.5' },
          { digest: 'aa11', y: '-.25' },
        ],
        markup: '<g data-lily-cell="aa11" transform="translate(0, -.25)">\n</g>\n',
      },
    ])
    assert.deepEqual(outcomeOf('/s/score.ly', outputs, 1).pages, ['score-1.svg', 'score-2.svg'])
  })

  test('the playback map: entry 0, as the Rust side reads it (D103)', () => {
    const timingOf = (json: string) =>
      outcomeOf('/s/score.ly', new Map([['score.midi', bytes('MThd')], ['score.timing.json', bytes(json)]]), 1).timing
    const first = { events: [{ href: 'textedit:///s/score.ly:2:3:4', at: 0, grace: 0, length: 0.25 }], bars: [{ at: 0, number: 1 }], staves: [{ id: '' }] }
    assert.deepEqual(timingOf(JSON.stringify([first, { events: [], bars: [] }])), first)
    assert.equal(timingOf('[]'), undefined)
    assert.equal(timingOf('[{"events":[]}]'), undefined)
    assert.equal(timingOf('{"events":[],"bars":[]}'), undefined)
    assert.equal(timingOf('[{"events":'), undefined)
  })

  test('the words: entry 0 of the file beside the map, as the Rust side reads them (D133)', () => {
    const map = '[{"events":[],"bars":[]}]'
    const timingOf = (words: string, name = 'score.words.json', timing = map) =>
      outcomeOf(
        '/s/score.ly',
        new Map([
          ['score.midi', bytes('MThd')],
          ['score.timing.json', bytes(timing)],
          [name, bytes(words)],
        ]),
        1,
      ).timing
    const lyric = { at: 0, text: 'la', hyphen: false, extender: false, line: 0, voice: 'v', italic: false }
    assert.deepEqual(timingOf(JSON.stringify([[lyric], []])), { events: [], bars: [], lyrics: [lyric] })
    // Words that cannot be read cost only the words; the map keeps the playhead.
    for (const words of ['[[]]', '[]', '{"at":0}', '[{"at":0}]', '[[{"at":']) assert.deepEqual(timingOf(words), { events: [], bars: [] }, words)
    // Another score's words are not this one's.
    assert.deepEqual(timingOf(JSON.stringify([[lyric]]), 'other.words.json'), { events: [], bars: [] })
    // The words ride on the map's entry, so a map that cannot be read has none.
    assert.equal(timingOf(JSON.stringify([[lyric]]), 'score.words.json', '[]'), undefined)
  })

  test('no diagnostics: ok', () => {
    assert.equal(outcomeOf('/s/score.ly', new Map([['score.svg', bytes('<svg/>')]]), 1).state, 'ok')
  })

  test('a map with no MIDI file gives no timing, as read_playback (G1, D222)', () => {
    const map = bytes('[{"events":[],"bars":[]}]')
    assert.equal(outcomeOf('/s/score.ly', new Map([['score.timing.json', map]]), 1).timing, undefined)
    assert.equal(performanceOf(new Map([['score.timing.json', map]]), 1).timing, undefined)
    assert.deepEqual(performanceOf(new Map([['score.midi', bytes('MThd')], ['score.timing.json', map]]), 1).timing, { events: [], bars: [] })
  })

  test('a page with its performance: the music, and the performers’ messages after the page’s (D264)', () => {
    const parse = { file: '/s/score.ly', line: 1, column: 1, severity: 'warning' as const, message: 'no \\version statement found' }
    const performer = { file: '/s/score.ly', line: 2, column: 3, severity: 'warning' as const, message: 'no such MIDI instrument: `no such\'' }
    const page = { ...outcomeOf('/s/score.ly', new Map([['score.svg', bytes('<svg/>')], ['diagnostics.json', bytes(JSON.stringify([parse]))]]), 9), performing: true }
    const performed = performanceOf(
      new Map([
        ['diagnostics.json', bytes(JSON.stringify([performer]))],
        ['score.midi', bytes('MThd')],
        ['score.timing.json', bytes('[{"events":[],"bars":[{"at":0,"number":1}]}]')],
      ]),
      3,
    )
    const whole = withPerformance(page, performed)
    assert.equal(whole.performing, undefined)
    assert.deepEqual(whole.diagnostics, [parse, performer])
    assert.deepEqual([whole.state, whole.errorCount, whole.warningCount, whole.durationMs], ['ok', 0, 2, 9])
    assert.deepEqual([whole.midi, [...(whole.midiData ?? [])], whole.timing], [['score.midi'], [...bytes('MThd')], { events: [], bars: [{ at: 0, number: 1 }] }])
    assert.deepEqual(whole.svg, ['<svg/>'])
    // A performer's error makes the outcome failed.
    const failed = withPerformance(page, { ...performed, diagnostics: [{ ...performer, severity: 'error' }] })
    assert.deepEqual([failed.state, failed.errorCount], ['failed', 1])
  })
})

/**
 * An engine in JavaScript behind the ABI of protocol.ts: each compile runs
 * `script`, one entry a step. A step returns Step.More, asks for a file or a
 * split module, traps, or ends with a page that names what it was given.
 */
type Action =
  | 'more'
  | 'trap'
  | 'panic'
  | { file: string }
  | { module: string }
  | { placeholder: string }
  /** Step.More with the visible page handed over (I7, D269): the outputs until the next step. */
  | { hand: [string, Uint8Array][] }
  | 'done'
let script: Action[] = []
/** The bytes each file was given to the newest instance with, as lily_put read them. */
let given = new Map<string, number[] | null>()
/** The modules given to the newest instance: their length, or null for none. */
let modulesGiven = new Map<string, number | null>()
/** Each lily_compile of every instance: its revision and root. */
let starts: [number, string][] = []
let instances = 0
let splitInstances = 0
/** The outputs a compile adds to its page. */
let extra: [string, Uint8Array][] = []
/** Each performance's steps (D264), as `script` a compile's, and what it writes. */
let performScript: Action[] = ['done']
const PERFORMED: [string, Uint8Array][] = [
  ['stderr', bytes('')],
  ['diagnostics.json', bytes('[]')],
  ['score.midi', bytes('MThd')],
  ['score.timing.json', bytes('[{"events":[],"bars":[{"at":0,"number":1}]}]')],
]

function fakeEngine(imports: WebAssembly.Imports) {
  instances++
  const memory = new WebAssembly.Memory({ initial: 4 })
  let next = 8
  const view = () => new Uint8Array(memory.buffer)
  // Four-byte aligned, as the out-parameters are u32.
  const alloc = (len: number) => {
    const at = next
    next = (next + len + 3) & ~3
    return at
  }
  const string = (at: number, len: number) => new TextDecoder().decode(view().subarray(at, at + len))
  const files = new Map<string, string | null>()
  let step = 0
  let performing = false
  let pdf = false
  let need = { kind: 0, name: '' }
  let outputs: [string, Uint8Array][] = []
  const write = (at: number, ...words: number[]) => new Uint32Array(memory.buffer, at, words.length).set(words)
  const place = (data: Uint8Array) => {
    const at = alloc(data.length)
    view().set(data, at)
    return at
  }
  return {
    memory,
    lily_alloc: alloc,
    lily_free: () => {},
    lily_start: () => 0,
    lily_put: (kind: number, name: number, nameLen: number, data: number, dataLen: number) => {
      if (kind === Need.Module) modulesGiven.set(string(name, nameLen), data === 0 ? null : dataLen)
      if (kind !== Need.File) return
      files.set(string(name, nameLen), data === 0 ? null : string(data, dataLen))
      given.set(string(name, nameLen), data === 0 ? null : [...view().subarray(data, data + dataLen)])
    },
    lily_pdf: (on: number) => {
      pdf = on !== 0
    },
    lily_compile: (revision: number, root: number, rootLen: number) => {
      starts.push([revision, string(root, rootLen)])
      step = 0
      performing = false
    },
    lily_perform: (revision: number, root: number, rootLen: number) => {
      starts.push([revision, `perform ${string(root, rootLen)}`])
      step = 0
      performing = true
    },
    lily_step: () => {
      const action = (performing ? performScript : script)[step]
      // A page handed over is the outputs until the next call.
      outputs = []
      if (action === 'more') return (step++, Step.More)
      if (typeof action === 'object' && 'hand' in action) return ((outputs = action.hand), step++, Step.More)
      if (action === 'trap') throw new WebAssembly.RuntimeError('unreachable')
      if (action === 'panic') {
        // The debug core: the message to env.lily_panic, then the trap (08 §1.3).
        const message = bytes('panicked at crates/layout/src/x.rs:1:2:\nindex out of bounds')
        ;(imports.env as Record<string, (at: number, len: number) => void>).lily_panic(place(message), message.length)
        throw new WebAssembly.RuntimeError('unreachable')
      }
      if (action === 'done') {
        outputs = performing ? PERFORMED : [['score.svg', bytes(`<svg>${[...files].map(([k, v]) => `${k}=${v}`).join(' ')}</svg>`)], ...extra]
        // Export PDF's display list (D271).
        if (pdf && !performing) outputs = [...outputs, ['score.pdf.list', bytes('LIST')]]
        return Step.Done
      }
      if ('file' in action) {
        if (files.has(action.file)) return (step++, Step.More)
        need = { kind: Need.File, name: action.file }
        return Step.Needs
      }
      if ('module' in action) {
        if (modulesGiven.has(action.module)) return (step++, Step.More)
        need = { kind: Need.Module, name: action.module }
        return Step.Needs
      }
      const placeholder = (imports[`placeholder.${action.placeholder}`] as Record<string, () => void> | undefined)?.['0']
      if (placeholder && !splitLoaded) placeholder()
      return (step++, Step.More)
    },
    lily_needs: (out: number) => {
      const name = bytes(need.name)
      write(out, need.kind, place(name), name.length)
    },
    lily_outputs: () => outputs.length,
    lily_output: (i: number, out: number) => {
      const [name, data] = outputs[i]
      const encoded = bytes(name)
      write(out, place(encoded), encoded.length, place(data), data.length)
    },
  }
}
let splitLoaded = false

/**
 * pdf.wasm (D270) in JavaScript: the fonts of a list `LIST` are C059-Roman and
 * NoSuch; its PDF names what it was given, and warns of a font it was not.
 */
let pdfInstances = 0
function fakePdf() {
  pdfInstances++
  const memory = new WebAssembly.Memory({ initial: 1 })
  let input = new Uint8Array()
  let output = new Uint8Array()
  let warnings = new Uint8Array()
  const put = (data: Uint8Array, at: number) => (new Uint8Array(memory.buffer).set(data, at), at)
  return {
    memory,
    lily_pdf_input: (len: number) => ((input = new Uint8Array(memory.buffer, 1024, len)), 1024),
    lily_pdf_fonts: () => (new TextDecoder().decode(input) === 'LIST' ? ((output = bytes('C059-Roman\nNoSuch')), 0) : 1),
    lily_pdf_write: () => {
      const data = input.slice()
      const view = new DataView(data.buffer)
      const pieces: string[] = []
      let at = 0
      const piece = () => {
        const len = view.getUint32(at, true)
        pieces.push(new TextDecoder().decode(data.subarray(at + 4, at + 4 + len)))
        at += 4 + len
      }
      piece()
      const count = view.getUint32(at, true)
      at += 4
      for (let i = 0; i < 2 * count; i++) piece()
      if (at !== data.length) return 1
      output = bytes(`%PDF ${pieces.join(' ')}`)
      warnings = bytes(count < 2 ? 'a run in NoSuch is left out' : '')
      return 0
    },
    lily_pdf_output: () => put(output, 32768),
    lily_pdf_output_len: () => output.length,
    lily_pdf_warnings: () => put(warnings, 49152),
    lily_pdf_warnings_len: () => warnings.length,
  }
}

const CORE = { core: true } as unknown as WebAssembly.Module
const PDF = { pdf: true } as unknown as WebAssembly.Module
/** The debug core: unsplit, so it imports no placeholder. */
const DEBUG = { debug: true } as unknown as WebAssembly.Module
const SPLIT = { split: true } as unknown as WebAssembly.Module
const real = { instantiate: WebAssembly.instantiate, imports: WebAssembly.Module.imports }

function driver(pause: () => Promise<void> = async () => {}, unreachable: string[] = [], debug = false) {
  const posted: FromEngine[] = []
  const manifest: EngineManifest = { ...MANIFEST, core: { ...MANIFEST.core, debug: 'core-debug.1.wasm' } }
  const loader = {
    module: async (file: string) => {
      if (unreachable.includes(file)) throw new Error(`${file}: 503 Service Unavailable`)
      return file === MANIFEST.core.wasm ? CORE : file === manifest.core.debug ? DEBUG : file === MANIFEST.modules.pdf.file ? PDF : SPLIT
    },
    bytes: async (file: string) => {
      if (unreachable.includes(file)) throw new Error(`${file}: 503 Service Unavailable`)
      return file.endsWith('.otf') ? bytes('OTF') : new Uint8Array(4)
    },
  }
  return { posted, driver: new Driver(manifest, loader, (message) => posted.push(message), pause, debug) }
}

const request = (revision: number, files: Record<string, string | Uint8Array | null> = { '/s/score.ly': '{ c }' }) =>
  ({
    type: 'compile',
    request: { revision, rootFile: '/s/score.ly', files: Object.fromEntries(Object.entries(files).map(([path, file]) => [path, typeof file === 'string' ? bytes(file) : file])), visiblePage: 1 },
  }) as ToEngine

/** Until the driver has nothing left to do. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 10))

describe('Driver', () => {
  beforeEach(() => {
    instances = 0
    performScript = ['done']
    given = new Map()
    modulesGiven = new Map()
    starts = []
    splitInstances = 0
    pdfInstances = 0
    splitLoaded = false
    WebAssembly.Module.imports = (module) =>
      module === CORE ? [{ module: 'placeholder.tablature', name: '0', kind: 'function' as const }] : module === DEBUG ? [] : real.imports(module)
    WebAssembly.instantiate = (async (module: WebAssembly.Module, imports: WebAssembly.Imports) => {
      if (module === SPLIT) return (splitInstances++, (splitLoaded = true), {})
      if (module === PDF) return { exports: fakePdf() }
      return { exports: fakeEngine(imports) }
    }) as unknown as typeof WebAssembly.instantiate
  })
  afterEach(() => {
    WebAssembly.instantiate = real.instantiate
    WebAssembly.Module.imports = real.imports
  })

  test('starts from the image, and runs a compile in steps to its outcome', async () => {
    script = ['more', 'more', 'done']
    const { driver: engine, posted } = driver()
    await engine.start()
    assert.equal(posted[0].type, 'ready')
    // The manifest's source and licences, which the page links (D56, D171).
    assert.deepEqual(posted[0].type === 'ready' && posted[0].about, { source: MANIFEST.source, licenses: MANIFEST.licenses })
    engine.receive(request(1))
    await settle()
    const finished = posted[1]
    assert.equal(finished.type === 'finished' && finished.revision, 1)
    assert.deepEqual(finished.type === 'finished' && finished.outcome.svg, ['<svg>/s/score.ly={ c }</svg>'])
  })

  test('posts a page’s cells before the compile that holds it, which is the same otherwise', async () => {
    script = ['done']
    extra = [['score.cells', bytes('aa:1\n<g/>')]]
    const { driver: engine, posted } = driver()
    await engine.start()
    engine.receive(request(1))
    await settle()
    extra = []
    assert.deepEqual(posted.slice(1, 2), [{ type: 'cells', revision: 1, page: 0, cells: [{ digest: 'aa', y: '1' }], markup: '<g/>' }])
    const finished = posted[2]
    assert.deepEqual(finished.type === 'finished' && finished.outcome.pages, ['score.svg'])
  })

  test('posts the visible page the core hands over before the end, whole, and the outcome after it (I7, D269)', async () => {
    script = ['more', { hand: [['score.svg', bytes('<svg>early</svg>')]] }, 'more', 'done']
    const { driver: engine, posted } = driver()
    await engine.start()
    engine.receive(request(1))
    await settle()
    assert.deepEqual(posted[1], { type: 'page', revision: 1, page: 0, svg: '<svg>early</svg>' })
    assert.equal(posted[2].type, 'finished')
    assert.equal(posted.length, 3)
  })

  test('posts the visible page handed over by its cells, and not its cells again at the end (I7, D269)', async () => {
    const cells = bytes('aa:1\n<g/>')
    script = [{ hand: [['score.svg', bytes('<svg/>')], ['score.cells', cells]] }, 'more', 'done']
    extra = [['score.cells', cells]]
    const { driver: engine, posted } = driver()
    await engine.start()
    engine.receive(request(1))
    await settle()
    extra = []
    assert.deepEqual(
      posted.slice(1).map((message) => message.type),
      ['cells', 'finished'],
    )
    assert.deepEqual(posted[1], { type: 'cells', revision: 1, page: 0, cells: [{ digest: 'aa', y: '1' }], markup: '<g/>' })
  })

  test('Export PDF: the compile gives its display list, which the PDF module writes with the fonts it names (D271)', async () => {
    script = ['done']
    const { driver: engine, posted } = driver()
    await engine.start()
    const pdfRequest = request(1)
    if (pdfRequest.type === 'compile') pdfRequest.request.pdf = true
    engine.receive(pdfRequest)
    await settle()
    const finished = posted[1]
    assert.equal(finished.type, 'finished')
    if (finished.type !== 'finished') return
    assert.deepEqual(finished.outcome.pages, ['score.svg'], 'the list is no page')
    assert.deepEqual(
      finished.pdf?.files.map(({ name, data }) => [name, new TextDecoder().decode(data)]),
      [['score.pdf', '%PDF LIST C059-Roman OTF']],
      'the font the build has; NoSuch, which it has not, left out',
    )
    assert.deepEqual(finished.pdf?.warnings, ['a run in NoSuch is left out'])
    assert.equal(pdfInstances, 1)
    // A compile for the preview after it gives no list, and no PDF.
    engine.receive(request(2))
    await settle()
    const next = posted[2]
    assert.equal(next.type === 'finished' && next.pdf, undefined)
    assert.equal(next.type === 'finished' && next.outcome.pages.length, 1)
  })

  test('Export PDF without its module, or with a font that cannot be fetched: no PDF, and why (D271)', async () => {
    script = ['done']
    const asked = (revision: number) => {
      const message = request(revision)
      if (message.type === 'compile') message.request.pdf = true
      return message
    }
    const { driver: engine, posted } = driver(async () => {}, ['C059-Roman.1.otf'])
    await engine.start()
    engine.receive(asked(1))
    await settle()
    const finished = posted[1]
    assert.equal(finished.type === 'finished' && finished.outcome.state, 'ok', 'the compile is shown all the same')
    assert.deepEqual(finished.type === 'finished' && finished.pdf, {
      files: [],
      warnings: [],
      message: 'The engine could not write the PDF: C059-Roman.1.otf: 503 Service Unavailable',
    })
  })

  test('framed: the list, the count, then each font’s name and OTF, each a u32 little-endian length first', () => {
    assert.deepEqual([...framed([bytes('L'), bytes('F'), bytes('OT')], 1)], [1, 0, 0, 0, 76, 1, 0, 0, 0, 1, 0, 0, 0, 70, 2, 0, 0, 0, 79, 84])
  })

  test('a newer request supersedes the running compile between steps', async () => {
    script = ['more', 'more', 'done']
    let pauses = 0
    const { driver: engine, posted } = driver(async () => {
      if (++pauses === 1) engine.receive(request(2, { '/s/score.ly': '{ d }' }))
    })
    await engine.start()
    engine.receive(request(1))
    await settle()
    assert.deepEqual(
      posted.slice(1).map((m) => [m.type, 'revision' in m && m.revision]),
      [
        ['superseded', 1],
        ['finished', 2],
      ],
    )
  })

  test('asks the page for a file it lacks, and compiles again with it; null is no such file', async () => {
    script = [{ file: '/s/a.ily' }, { file: '/s/b.ily' }, 'done']
    const { driver: engine, posted } = driver()
    await engine.start()
    engine.receive(request(1))
    await settle()
    assert.deepEqual(posted[1], { type: 'needs-file', revision: 1, path: '/s/a.ily' })
    engine.receive({ type: 'file', path: '/s/a.ily', bytes: bytes('e') })
    await settle()
    engine.receive({ type: 'file', path: '/s/b.ily', bytes: null })
    await settle()
    const finished = posted.at(-1)
    assert.deepEqual(finished?.type === 'finished' && finished.outcome.svg, ['<svg>/s/score.ly={ c } /s/a.ily=e /s/b.ily=null</svg>'])
    // Started again after each answer with the request's revision and root (D89).
    assert.deepEqual(starts, Array(3).fill([1, '/s/score.ly']))
  })

  test('gives the engine the page’s bytes unchanged, not UTF-8 of them (D102)', async () => {
    script = [{ file: '/s/a.ily' }, 'done']
    const { driver: engine } = driver()
    await engine.start()
    // Latin-1: `{ é }` and `è`, neither of them UTF-8.
    engine.receive(request(1, { '/s/score.ly': new Uint8Array([0x7b, 0x20, 0xe9, 0x20, 0x7d]) }))
    await settle()
    engine.receive({ type: 'file', path: '/s/a.ily', bytes: new Uint8Array([0xe8]) })
    await settle()
    assert.deepEqual(given.get('/s/score.ly'), [0x7b, 0x20, 0xe9, 0x20, 0x7d])
    assert.deepEqual(given.get('/s/a.ily'), [0xe8])
  })

  test('a placeholder: a new instance, the split module in it, and the compile again', async () => {
    script = [{ placeholder: 'tablature' }, 'done']
    const { driver: engine, posted } = driver()
    await engine.start()
    engine.receive(request(1))
    await settle()
    assert.equal(instances, 2)
    assert.equal(splitInstances, 1)
    assert.equal(posted.at(-1)?.type, 'finished')
    assert.deepEqual(starts, Array(2).fill([1, '/s/score.ly']))
  })

  test('a split module that cannot be fetched is a compile that did not run, and says which', async () => {
    script = [{ placeholder: 'tablature' }, 'done']
    const { driver: engine, posted } = driver(undefined, ['tablature.1.wasm'])
    await engine.start()
    engine.receive(request(1))
    await settle()
    const last = posted.at(-1)
    assert.equal(last?.type === 'finished' && last.revision, 1)
    assert.equal(last?.type === 'finished' && last.outcome.state, 'error')
    assert.match(last?.type === 'finished' ? (last.outcome.message ?? '') : '', /could not compile the score: tablature\.1\.wasm: 503/)
    script = ['done']
    engine.receive(request(2))
    await settle()
    const next = posted.at(-1)
    assert.equal(next?.type === 'finished' && next.outcome.state, 'ok')
  })

  test('a pack or part that cannot be fetched is given as none, for the core to say so; a trigger’s is only not fetched (I4)', async () => {
    script = [{ module: 'emmentaler-14' }, 'done']
    const { driver: engine, posted } = driver(undefined, ['chords.1.part', 'emmentaler-14.1.pack'])
    await engine.start()
    engine.receive(request(1, { '/s/score.ly': '\\chordmode { c }' }))
    await settle()
    const last = posted.at(-1)
    assert.equal(last?.type === 'finished' && last.outcome.state, 'ok', 'the compile ran')
    assert.equal(modulesGiven.get('emmentaler-14'), null, 'none: the core’s diagnostic says it could not be fetched')
    assert.equal(modulesGiven.has('chords'), false, 'a trigger that failed gives nothing; the core asks if it needs it')
  })

  test('LilyPond’s own files are the manifest’s, never the page’s; one it has not is none (I2, D263)', async () => {
    script = [{ file: '<datadir>/ly/articulate.ly' }, { file: '<datadir>/ly/mine.ily' }, { file: '/s/mine.ily' }, 'done']
    const { driver: engine, posted } = driver()
    await engine.start()
    engine.receive(request(1))
    await settle()
    assert.deepEqual(
      posted.filter((m) => m.type === 'needs-file'),
      [{ type: 'needs-file', revision: 1, path: '/s/mine.ily' }],
      'the page is asked only for the score’s own file',
    )
    assert.deepEqual(given.get('<datadir>/ly/articulate.ly'), [0, 0, 0, 0])
    assert.equal(given.get('<datadir>/ly/mine.ily'), null)
  })

  test('an include of LilyPond’s named in a score is fetched with the core, and given again to a new instance', async () => {
    script = [{ placeholder: 'tablature' }, 'done']
    const { driver: engine, posted } = driver()
    await engine.start()
    engine.receive(request(1, { '/s/score.ly': '\\include "articulate.ly"' }))
    await settle()
    assert.equal(instances, 2)
    const finished = posted.at(-1)
    // The page names the files of the instance that compiled it: the placeholder's new one.
    assert.match(finished?.type === 'finished' ? finished.outcome.svg[0] : '', /<datadir>\/ly\/articulate\.ly=\0{4}/)
  })

  test('a file that is gone is given as none (D172)', async () => {
    script = ['done']
    const { driver: engine } = driver()
    await engine.start()
    engine.receive(request(1, { '/s/score.ly': '{ c }', '/s/part.ily': null }))
    await settle()
    assert.equal(given.get('/s/part.ily'), null)
  })

  test('a compile’s outcome is its pages; its music comes when the page asks, in steps, once (M7, D264)', async () => {
    script = ['done']
    performScript = ['more', 'more', 'done']
    let pauses = 0
    const { driver: engine, posted } = driver(async () => void pauses++)
    await engine.start()
    engine.receive(request(1))
    await settle()
    const finished = posted[1]
    assert.ok(finished.type === 'finished')
    assert.equal(finished.outcome.performing, true)
    assert.equal(finished.outcome.midiData, undefined)
    assert.deepEqual(starts, [[1, '/s/score.ly']], 'nothing performed until asked')
    pauses = 0
    engine.receive({ type: 'perform', revision: 1 })
    await settle()
    assert.equal(pauses, 2, 'in steps')
    const performed = posted[2]
    assert.ok(performed.type === 'performed' && performed.performance)
    assert.deepEqual(starts.at(-1), [1, 'perform /s/score.ly'])
    assert.deepEqual(performed.performance.midi, ['score.midi'])
    assert.deepEqual([...(performed.performance.midiData ?? [])], [...bytes('MThd')])
    assert.deepEqual(performed.performance.timing, { events: [], bars: [{ at: 0, number: 1 }] })
    // Asked again: the same, not performed again.
    engine.receive({ type: 'perform', revision: 1 })
    await settle()
    assert.deepEqual(posted[3], performed)
    assert.equal(starts.filter(([, root]) => root.startsWith('perform')).length, 1)
  })

  test('a newer compile stops a performance, and one of another compile is none (D264)', async () => {
    script = ['done']
    performScript = ['more', 'more', 'done']
    let stop = false
    const { driver: engine, posted } = driver(async () => {
      if (!stop) return
      stop = false
      engine.receive(request(2, { '/s/score.ly': '{ d }' }))
    })
    await engine.start()
    engine.receive(request(1))
    await settle()
    stop = true
    engine.receive({ type: 'perform', revision: 1 })
    await settle()
    assert.deepEqual(
      posted.slice(2).map((m) => [m.type, 'revision' in m && m.revision, m.type === 'performed' && m.performance !== undefined]),
      [
        ['performed', 1, false],
        ['finished', 2, false],
      ],
    )
    engine.receive({ type: 'perform', revision: 1 })
    await settle()
    assert.deepEqual(posted.at(-1), { type: 'performed', revision: 1 })
    // Asked for, then a newer compile before it began: none, and the compile runs.
    engine.receive({ type: 'perform', revision: 2 })
    engine.receive(request(3))
    await settle()
    assert.deepEqual(
      posted.slice(-2).map((m) => [m.type, 'revision' in m && m.revision]),
      [
        ['performed', 2],
        ['finished', 3],
      ],
    )
  })

  test('a performance asks for what it lacks as a compile does, given the request’s files again (D264)', async () => {
    script = ['done']
    performScript = [{ file: '/s/a.ily' }, 'done']
    const { driver: engine, posted } = driver()
    await engine.start()
    engine.receive(request(1))
    await settle()
    engine.receive({ type: 'perform', revision: 1 })
    await settle()
    const asked = posted[2]
    assert.deepEqual(asked, { type: 'needs-file', revision: 1, path: '/s/a.ily' })
    engine.receive({ type: 'file', path: '/s/a.ily', bytes: bytes('c4') })
    await settle()
    assert.equal(posted[3].type === 'performed' && posted[3].performance?.midi[0], 'score.midi')
    assert.deepEqual(given.get('/s/score.ly'), [...bytes('{ c }')])
  })

  test('a performance that traps did not run, and says why; the new instance has no compile to perform', async () => {
    script = ['done']
    performScript = ['trap']
    const { driver: engine, posted } = driver()
    await engine.start()
    engine.receive(request(1))
    await settle()
    engine.receive({ type: 'perform', revision: 1 })
    await settle()
    const performed = posted[2]
    assert.ok(performed.type === 'performed')
    assert.match(performed.performance?.message ?? '', /^The engine stopped: /)
    engine.receive({ type: 'perform', revision: 1 })
    await settle()
    assert.deepEqual(posted[3], { type: 'performed', revision: 1 })
  })

  test('a trap is a compile that did not run, and the next one starts afresh', async () => {
    script = ['trap']
    const { driver: engine, posted } = driver()
    await engine.start()
    engine.receive(request(1))
    await settle()
    const failed = posted.at(-1)
    assert.equal(failed?.type === 'finished' && failed.outcome.state, 'error')
    assert.match(failed?.type === 'finished' ? (failed.outcome.message ?? '') : '', /The engine stopped: unreachable/)
    script = ['done']
    engine.receive(request(2))
    await settle()
    assert.equal(instances, 2)
    const last = posted.at(-1)
    assert.equal(last?.type === 'finished' && last.outcome.state, 'ok')
  })
})

describe('Driver with the debug core', () => {
  beforeEach(() => {
    instances = 0
    splitInstances = 0
    splitLoaded = false
    WebAssembly.Module.imports = (module) => (module === DEBUG ? [] : [{ module: 'placeholder.tablature', name: '0', kind: 'function' as const }])
    WebAssembly.instantiate = (async (module: WebAssembly.Module, imports: WebAssembly.Imports) => {
      if (module === SPLIT) return (splitInstances++, (splitLoaded = true), {})
      assert.equal(module, DEBUG)
      return { exports: fakeEngine(imports) }
    }) as unknown as typeof WebAssembly.instantiate
  })
  afterEach(() => {
    WebAssembly.instantiate = real.instantiate
    WebAssembly.Module.imports = real.imports
  })

  test('runs the debug core when asked, and a panic says why it stopped (08 §1.3)', async () => {
    script = ['panic']
    const { driver: engine, posted } = driver(undefined, [], true)
    await engine.start()
    engine.receive(request(1))
    await settle()
    const failed = posted.at(-1)
    assert.match(failed?.type === 'finished' ? (failed.outcome.message ?? '') : '', /The engine stopped: panicked at crates\/layout\/src\/x\.rs:1:2:\nindex out of bounds/)
    // The next trap is not that panic's.
    script = ['trap']
    engine.receive(request(2))
    await settle()
    const next = posted.at(-1)
    assert.match(next?.type === 'finished' ? (next.outcome.message ?? '') : '', /The engine stopped: unreachable/)
  })

  test('the debug core is not split: a split module a score names is not fetched into it', async () => {
    script = ['done']
    const { driver: engine, posted } = driver(undefined, [], true)
    await engine.start()
    engine.receive(request(1, { '/s/score.ly': '\\new TabStaff { c }' }))
    await settle()
    assert.equal(splitInstances, 0)
    assert.equal(posted.at(-1)?.type === 'finished' && (posted.at(-1) as { outcome: { state: string } }).outcome.state, 'ok')
  })
})

describe('Driver at the memory maximum', () => {
  test('a trap with the memory full says why', async () => {
    script = ['trap']
    // The engine's memory has grown to the maximum when it traps.
    WebAssembly.instantiate = (async (_: WebAssembly.Module, imports: WebAssembly.Imports) => {
      const engine = fakeEngine(imports)
      const exports = { ...engine }
      exports.lily_step = () => {
        exports.memory = { buffer: { byteLength: 256 * 2 ** 20 } } as unknown as WebAssembly.Memory
        return engine.lily_step()
      }
      return { exports }
    }) as unknown as typeof WebAssembly.instantiate
    WebAssembly.Module.imports = () => []
    try {
      const { driver: engine, posted } = driver()
      await engine.start()
      engine.receive(request(1))
      await settle()
      const last = posted.at(-1)
      assert.match(last?.type === 'finished' ? (last.outcome.message ?? '') : '', /more memory than the engine may use \(256 MiB\)/)
    } finally {
      WebAssembly.instantiate = real.instantiate
      WebAssembly.Module.imports = real.imports
    }
  })
})

describe('EngineHost', () => {
  function host(readFile: (path: string) => Promise<Uint8Array | null> = async () => null, onCells?: (cells: PageUpdate) => void) {
    const sent: ToEngine[] = []
    const worker: EngineWorker = { onmessage: null, postMessage: (message) => sent.push(message), terminate: () => {} }
    const reply = (data: FromEngine) => worker.onmessage?.({ data } as MessageEvent<FromEngine>)
    return { host: new EngineHost(worker, readFile, onCells, storage), sent, reply, worker }
  }
  /** navigator.storage's fake: what was asked, and whether the origin is kept already. */
  let asked: string[] = []
  let kept = false
  const storage = {
    persisted: async () => (asked.push('persisted'), kept),
    persist: async () => (asked.push('persist'), true),
  }
  beforeEach(() => {
    asked = []
    kept = false
  })

  test('asks the browser to keep the engine’s files once it has compiled, and only once (08 §5.4)', async () => {
    const { host: engine, reply } = host()
    const first = engine.compile('/s/score.ly', {})
    reply({ type: 'failed', message: 'no wasm' })
    await first
    await settle()
    assert.deepEqual(asked, [])
    const { host: used, reply: answer } = host()
    const superseded = used.compile('/s/score.ly', {})
    answer({ type: 'superseded', revision: 1 })
    await superseded
    await settle()
    assert.deepEqual(asked, [], 'not before a compile finished')
    for (const revision of [2, 3]) {
      const pending = used.compile('/s/score.ly', {})
      answer({ type: 'finished', revision, outcome: outcomeOf('/s/score.ly', new Map(), 1) })
      assert.equal((await pending)?.state, 'ok')
    }
    await settle()
    assert.deepEqual(asked, ['persisted', 'persist'])
  })

  test('does not ask again where the origin is kept already', async () => {
    kept = true
    const { host: engine, reply } = host()
    const pending = engine.compile('/s/score.ly', {})
    reply({ type: 'finished', revision: 1, outcome: outcomeOf('/s/score.ly', new Map(), 1) })
    await pending
    await settle()
    assert.deepEqual(asked, ['persisted'])
  })

  test('passes on the cells of the compile it waits for, and only those', async () => {
    const passed: PageUpdate[] = []
    const { host: engine, reply } = host(undefined, (cells) => passed.push(cells))
    const cells = (revision: number): PageCells => ({ type: 'cells', revision, page: 0, cells: [], markup: '' })
    const page = (revision: number): PageUpdate => ({ type: 'page', revision, page: 0, svg: '<svg/>' })
    void engine.compile('/s/score.ly', {})
    const second = engine.compile('/s/score.ly', {})
    reply(cells(1))
    reply(page(1))
    reply(page(2))
    reply(cells(2))
    reply({ type: 'finished', revision: 2, outcome: outcomeOf('/s/score.ly', new Map(), 1) })
    await second
    reply(cells(2))
    reply(page(2))
    assert.deepEqual(passed, [page(2), cells(2)])
  })

  test('performs the newest compile once, however often asked; a newer compile stops it (D264)', async () => {
    const { host: engine, sent, reply } = host()
    assert.equal(await engine.perform(), undefined, 'nothing compiled')
    void engine.compile('/s/score.ly', {})
    const first = engine.perform()
    const again = engine.perform()
    assert.deepEqual(sent.slice(1), [{ type: 'perform', revision: 1 }])
    const performance = { midi: ['score.midi'], midiData: bytes('MThd'), diagnostics: [], durationMs: 2 }
    reply({ type: 'performed', revision: 1, performance })
    assert.deepEqual([await first, await again], [performance, performance])
    const stopped = engine.perform()
    void engine.compile('/s/score.ly', {})
    assert.equal(await stopped, undefined)
  })

  test('only the newest compile is answered', async () => {
    const { host: engine, reply } = host()
    const first = engine.compile('/s/score.ly', {})
    const second = engine.compile('/s/score.ly', {})
    assert.equal(await first, undefined)
    reply({ type: 'finished', revision: 2, outcome: outcomeOf('/s/score.ly', new Map(), 1) })
    assert.equal((await second)?.state, 'ok')
  })

  test('answers the engine’s questions for files with readFile', async () => {
    const { reply, sent } = host(async (path) => (path === '/s/a.ily' ? new Uint8Array([0xe9]) : null))
    reply({ type: 'needs-file', revision: 1, path: '/s/a.ily' })
    reply({ type: 'needs-file', revision: 1, path: '/s/none.ily' })
    await settle()
    assert.deepEqual(sent, [
      { type: 'file', path: '/s/a.ily', bytes: new Uint8Array([0xe9]) },
      { type: 'file', path: '/s/none.ily', bytes: null },
    ])
  })

  test('sends an editor’s text as UTF-8 and a file’s bytes as read (D102)', () => {
    const { host: engine, sent } = host()
    void engine.compile('/s/score.ly', { '/s/score.ly': '{ é }', '/s/a.ily': new Uint8Array([0xe9]) }, 2)
    assert.deepEqual(sent, [
      {
        type: 'compile',
        request: { revision: 1, rootFile: '/s/score.ly', files: { '/s/score.ly': bytes('{ é }'), '/s/a.ily': new Uint8Array([0xe9]) }, visiblePage: 2 },
      },
    ])
    assert.deepEqual(sent[0].type === 'compile' && [...sent[0].request.files['/s/score.ly']], [0x7b, 0x20, 0xc3, 0xa9, 0x20, 0x7d])
  })

  test('says what the engine is, once started (D171)', async () => {
    const { host: engine, reply } = host()
    assert.equal(engine.about, undefined)
    const about = { source: MANIFEST.source, licenses: MANIFEST.licenses }
    reply({ type: 'ready', ms: 5, about })
    assert.equal(await engine.ready, 5)
    assert.deepEqual(engine.about, about)
  })

  test('a worker that stops, or whose script did not load, is an engine that failed (D171)', async () => {
    const { host: engine, worker } = host()
    const pending = engine.compile('/s/score.ly', {})
    worker.onerror?.({} as ErrorEvent)
    assert.equal(await pending, undefined)
    assert.equal(engine.failed, true)
    await assert.rejects(engine.ready, /stopped/)
    assert.equal(await engine.compile('/s/score.ly', {}), undefined)
  })

  test('an engine that failed answers nothing, and says so', async () => {
    const { host: engine, reply } = host()
    const pending = engine.compile('/s/score.ly', {})
    reply({ type: 'failed', message: 'no wasm' })
    assert.equal(await pending, undefined)
    assert.equal(engine.failed, true)
    await assert.rejects(engine.ready, /no wasm/)
  })
})

describe('engine.js', () => {
  test('starts the engine for ?engine=1, the debug core for ?engine=debug, and nothing otherwise', () => {
    assert.equal(engineMode('?engine=1', false), 'on')
    assert.equal(engineMode('?x=2&engine=debug', false), 'debug')
    for (const search of ['', '?engine=0', '?engine', '?engine=true', '?debug']) assert.equal(engineMode(search, false), 'off', search)
  })

  test('the site’s default starts it for any page but one that refuses it with ?engine=0 (D174)', () => {
    for (const search of ['', '?engine', '?engine=true', '?debug', '?engine=1']) assert.equal(engineMode(search, true), 'on', search)
    assert.equal(engineMode('?engine=debug', true), 'debug')
    assert.equal(engineMode('?engine=0', true), 'off')
    assert.equal(engineMode('?x=2&engine=0', true), 'off')
  })

  test('the default waits for esbuild’s --engine-default, which the tests’ build does not give (D174)', () => {
    assert.equal(engineByDefault, false)
  })

  test('the files the engine asks for wait for the page, and its cells go to the page once given', async () => {
    let readFile: (path: string) => Promise<Uint8Array | null> = async () => null
    let onCells: ((cells: PageUpdate) => void) | undefined
    const started: (boolean | undefined)[] = []
    const engine = startEngine(true, (read, cells, debug) => {
      readFile = read
      onCells = cells
      started.push(debug)
      return {} as EngineHost
    })
    assert.deepEqual(started, [true])
    const cells: PageCells = { type: 'cells', revision: 1, page: 0, cells: [], markup: '' }
    onCells?.(cells)
    let read: Uint8Array | null | undefined
    void readFile('/s/a.ily').then((bytes) => (read = bytes))
    await settle()
    assert.equal(read, undefined, 'not before the page is attached')
    const passed: PageUpdate[] = []
    engine.attach({ readFile: async (path) => (path === '/s/a.ily' ? new Uint8Array([1]) : null), onCells: (given) => passed.push(given) })
    await settle()
    assert.deepEqual(read, new Uint8Array([1]))
    onCells?.(cells)
    assert.deepEqual(passed, [cells], 'cells before the page are dropped: its outcome follows')
  })
})
