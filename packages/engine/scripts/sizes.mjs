// The engine's size report (K1, H4): each file of dist/engine/ in raw, gzip
// and brotli bytes, the interpreter's scheme.image beside them, every font pack of dist/engine/fonts/ (W1) against README
// §4.1 and §4.2 and chapter 06 §8, and core.wasm's code by crate against
// README §4.1's shares of B1 and B3, with the bytes of core::fmt. It reports
// and never fails: the budgets are not gates for now (IMPLEMENT.md, *Limits*).
//
//   node scripts/sizes.mjs     after scripts/build.mjs, which calls it too
//
// The code is attributed from target/engine/core.names.wasm, the same build
// as core.wasm through the same wasm-opt -Oz with the name section kept, as
// chapter 09 §2.1 did. A function belongs to the crate of the outermost path
// of its mangled name: the crate that defines the item, or the impl's crate;
// but a generic instance that an engine crate made, such as
// `Vec<Item>::extend`, belongs to that crate, since it costs what it does there.
// A crate's brotli bytes are those of its function bodies compressed
// together, an estimate; the whole file's are measured. The data section
// (OPEN H2) is charged to a crate by its string literals of SOURCE_STRING
// bytes or more, found in it byte for byte (the Scheme preludes most of
// all); the rest, which no name says the owner of, to none.
//
// wasm-opt inlines across crates: a function of one call site goes into its
// caller whatever rustc was told, and merge-similar-functions leaves bodies
// of no name. So the code is also given by origin (OPEN H4, D225), from the
// core as rustc wrote it (target/wasm32-unknown-unknown/wasm-names/) and both
// call graphs, read with Binaryen's wasm-dis: a function's bytes are shared
// among the rows of the engine code wasm-opt put into it, by those
// functions' sizes before wasm-opt; the runtime's code put into it stays its
// own, as a generic instance does; an unnamed body goes to its callers. A
// row's brotli bytes by origin are an estimate at its functions' ratio by name.
import { spawnSync } from 'node:child_process'
import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { dirname } from 'node:path'
import { pathToFileURL } from 'node:url'
import { brotliCompressSync, constants, gzipSync } from 'node:zlib'

const KB = 1000

/**
 * README §4.1's parts: brotli KB (B1) and raw KB of core.wasm (B3), Must and
 * Goal. A module's crate adds its name to its part when it is created. Crates
 * outside the engine (std, core, alloc, dlmalloc, …) are the Rust runtime,
 * charged with the core's crate (K2) and its exports to the first part. A
 * part's `modules` are shown below its crates, each the functions of the
 * part's crates whose mangled names name one of its Rust modules (a
 * module's identifier is its length and its name): N3's context
 * definitions and instances are modules of the music crate (D73), and so
 * are N4's iterators, N5's translators (D79), N6's `Grace_engraver` and
 * N7's `Repeat_acknowledge_engraver` (the volta iterators are N4's), the
 * lyric combine iterator's listeners (its iterator is N4's) and the
 * translators' parts that set the melisma properties. A part's `borrows`
 * are the functions of another part's crate whose mangled names name one of
 * its modules, charged to it and not to that part: the MIDI performers are
 * modules of the music crate (MS5 step 7), its audio elements, items and
 * performance with them, beside crates/smf's writer.
 */
const PARTS = [
  { name: 'Driver, ABI, diagnostics, Rust runtime', owner: '08', brotli: [30, 20], raw: [80, 50], crates: ['lily_engine_core'], runtime: true },
  { name: 'Parser', owner: '02', brotli: [80, 55], raw: [210, 145], crates: ['lily_engine_parser'] },
  { name: 'Scheme interpreter', owner: '03', brotli: [190, 140], raw: [500, 380], crates: ['lily_engine_scheme'] },
  {
    name: 'Music model', owner: '04', brotli: [160, 110], raw: [420, 290], crates: ['lily_engine_music'],
    modules: [
      { name: 'N3: context_def, context_mod, output_def', test: /11context_def|11context_mod|10output_def/ },
      { name: 'N3: context, override_, dispatcher, event_class', test: /7context\d|9override_\d|10dispatcher\d|11event_class\d/ },
      { name: 'N4: iterator, global', test: /8iterator\d|6global\d/ },
      { name: 'N5: translator, timing, records, scheme_translator, playback, marks', test: /10translator\d|6timing\d|7records\d|17scheme_translator\d|8playback\d|5marks\d/ },
      { name: 'N6: grace', test: /5grace\d/ },
      { name: 'N7: volta', test: /5volta\d/ },
      { name: 'N7: lyrics', test: /6lyrics\d/ },
      { name: 'N7: melisma', test: /7melisma\d/ },
    ],
  },
  { name: 'Layout and notation', owner: '05', brotli: [400, 260], raw: [1100, 700], crates: ['lily_engine_skyline', 'lily_engine_layout'] },
  { name: 'SVG writer and text shaping', owner: '06', brotli: [40, 25], raw: [110, 70], crates: ['lily_engine_fonts', 'lily_engine_svg'] },
  {
    name: 'MIDI performers and writer', owner: '07', brotli: [45, 30], raw: [120, 80], crates: ['lily_engine_smf'],
    borrows: {
      crate: 'lily_engine_music',
      name: 'M2–M5: audio, midi_item, performance, performers, dynamic, control and mark performers',
      test: /5audio\d|9midi_item\d|11performance\d|10performers\d|17dynamic_performer\d|18control_performers\d|15mark_performers\d/,
    },
  },
]
/**
 * K4's notation modules (08 §2.4, D223), each a crate of its own: 05 §9's
 * budget for each, brotli KB Must / Goal. Since MS4 step 20 wasm-split moves
 * the tablature's and the chord names' out of the core (split.mjs' SPLIT),
 * each measured as its manifest's file; what of their crates the core keeps
 * is reported in the core. The figured bass's stays in the core, which every
 * Staff reaches (D230), and is measured there against the module's budget.
 */
const MODULES = { lily_engine_figured_bass: 'figured-bass', lily_engine_tablature: 'tablature', lily_engine_chord_names: 'chord-names' }
const MODULE_BROTLI = [100, 40]
/** README §4.2's split modules of other budgets than the notation's, by file: `syntax-case` (D273). */
const SPLIT_BROTLI = { 'syntax-case.wasm': [40, 20] }
/** Crates of the engine that are no module of the roadmap, and so have no share. */
const UNBUDGETED = { lily_engine_spike: 'the harness\'s toy engine (09)' }

/** README §4.1's budgets for files, brotli KB, and B3 for core.wasm. */
const FILE_BUDGETS = { image: [420, 280], packs: [100, 70], sum: [1500, 1000], coreRaw: [4000, 2500], pdf: [500, 100] }

/**
 * The font packs' budgets, brotli KB, by the pack's file name: README §4.2's
 * per module (06 §8 gives the same, and 15 / 8 for each brace part; the
 * braces' Goal is 23 since MS0's step 12), and §4.1's
 * share of B1 for the fonts in the core, Emmentaler-20 and the three C059
 * styles of 06 §5.2 (06 §8 asked 120 / 80).
 */
const FONT_BUDGETS = [
  { kind: 'music', test: /^emmentaler-\d+\.pack$/, each: [60, 40], source: 'README §4.2, 06 §8' },
  { kind: 'brace part', test: /^emmentaler-brace-\d+\.pack$/, each: [15, 8], all: [25, 23], source: '06 §8 each, README §4.2 all' },
  { kind: 'text', test: /^(C059|Nimbus|TeXGyre)[^.]*\.pack$/, each: [20, 12], source: 'README §4.2, 06 §8' },
  // A text face's outlines (E14, D262), fetched when a skyline first needs one: no budget of their own.
  { kind: 'outlines', test: /^(C059|Nimbus|TeXGyre)[^.]*\.outlines\.pack$/, source: 'D262' },
]
const CORE_FONTS = { files: ['emmentaler-20.pack', 'C059-Roman.pack', 'C059-Bold.pack', 'C059-Italic.pack'], budget: [100, 70] }
/**
 * The outlines T02's first page fetches (D262, measured at MS7 step 5): B1 is
 * everything T02's first page needs (00 §2), so the download is read both
 * ways, without them and with them.
 */
const T02_OUTLINES = ['C059-Roman.outlines']

/** The shortest string literal of a crate's source looked for in the data section. */
const SOURCE_STRING = 8

const brotli = (bytes) => brotliCompressSync(bytes, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }).length
const gzip = (bytes) => gzipSync(bytes, { level: 9 }).length
const n = (value) => (value === undefined ? '' : value.toLocaleString('en-US'))
const status = (bytes, [must, goal]) => (bytes > must * KB ? 'over Must' : bytes > goal * KB ? 'over Goal' : 'ok')
const budget = (pair) => (pair ? `${n(pair[0])} / ${n(pair[1])}` : '')

function table(head, rows) {
  const widths = head.map((_, i) => Math.max(...[head, ...rows].map((row) => String(row[i] ?? '').length)))
  const line = (row) => row.map((cell, i) => (i === 0 ? String(cell ?? '').padEnd(widths[i]) : String(cell ?? '').padStart(widths[i]))).join('  ').trimEnd()
  return [line(head), ...rows.map(line)].join('\n')
}

// --- wasm ------------------------------------------------------------------

/** The function bodies of a module, by function index, with their names, and its data segments' bytes. */
export function functions(bytes) {
  let at = 8
  const u32 = () => {
    let value = 0
    for (let shift = 0; ; shift += 7) {
      const byte = bytes[at++]
      value += (byte & 0x7f) * 2 ** shift
      if (byte < 0x80) return value
    }
  }
  const name = () => {
    const length = u32()
    return new TextDecoder().decode(bytes.subarray(at, (at += length)))
  }
  let imported = 0
  const bodies = []
  const names = new Map()
  let data = 0
  let code = 0
  const segments = []
  while (at < bytes.length) {
    const id = bytes[at++]
    const size = u32()
    const end = at + size
    if (id === 2) {
      for (let count = u32(); count > 0; count--) {
        name()
        name()
        const kind = bytes[at++]
        const limits = () => (bytes[at++] & 1 ? u32() + u32() : u32())
        if (kind === 0) {
          imported++
          u32()
        } else if (kind === 1) {
          at++
          limits()
        } else if (kind === 2) limits()
        else if (kind === 3) at += 2
        else if (kind === 4) {
          at++
          u32()
        }
      }
    } else if (id === 10) {
      code = size
      for (let count = u32(); count > 0; count--) {
        const start = at
        const length = u32()
        at += length
        bodies.push(bytes.subarray(start, at))
      }
    } else if (id === 11) {
      data = size
      for (let count = u32(); count > 0; count--) {
        const flags = u32()
        if (flags === 2) u32()
        // An active segment's offset, a constant expression ending with `end`.
        if (flags !== 1) while (bytes[at++] !== 0x0b);
        const length = u32()
        segments.push(bytes.subarray(at, (at += length)))
      }
    } else if (id === 0 && name() === 'name') {
      while (at < end) {
        const sub = bytes[at++]
        const length = u32()
        const subEnd = at + length
        if (sub === 1) for (let count = u32(); count > 0; count--) names.set(u32(), name())
        at = subEnd
      }
    }
    at = end
  }
  return { code, data, segments: Buffer.concat(segments), bodies: bodies.map((body, i) => ({ body, name: names.get(imported + i) ?? '' })) }
}

// --- the data section --------------------------------------------------------

/** The string literals of SOURCE_STRING bytes or more in a crate's `src/`, raw or without an escape. */
export function sourceStrings(dir) {
  const strings = new Set()
  for (const file of readdirSync(dir, { recursive: true }).filter((file) => file.endsWith('.rs'))) {
    const text = readFileSync(`${dir}/${file}`, 'utf8')
    const raw = [...text.matchAll(/\br(#*)"([\s\S]*?)"\1/g)].map((m) => m[2])
    // The plain ones once the raw ones, the comments' lines and the quote's character literals are gone.
    const rest = text.replace(/\br(#*)"[\s\S]*?"\1/g, '').replace(/^\s*\/\/[^\n]*/gm, '').replace(/'\\?"'/g, '')
    const plain = [...rest.matchAll(/"((?:[^"\\]|\\[\s\S])*)"/g)].map((m) => m[1]).filter((string) => !string.includes('\\'))
    for (const string of [...raw, ...plain]) if (Buffer.byteLength(string) >= SOURCE_STRING) strings.add(string)
  }
  return [...strings]
}

/**
 * The data section by crate (H2): each engine crate's string literals found
 * in `segments`, the longest first, the bytes they cover (a byte charged
 * once, to the first string found there) and their brotli bytes compressed
 * together.
 */
export function dataByCrate(segments, crates) {
  const owner = new Uint8Array(segments.length)
  const found = crates.map(() => [])
  const strings = crates.flatMap(({ strings }, i) => strings.map((string) => ({ i, bytes: Buffer.from(string) })))
  for (const { i, bytes } of strings.sort((a, b) => b.bytes.length - a.bytes.length)) {
    for (let at = segments.indexOf(bytes); at >= 0; at = segments.indexOf(bytes, at + 1)) {
      let mine = 0
      for (let j = at; j < at + bytes.length; j++) {
        if (owner[j]) continue
        owner[j] = i + 1
        mine++
      }
      if (mine) found[i].push(bytes)
    }
  }
  const rows = crates.map(({ crate }, i) => ({
    crate,
    count: found[i].length,
    raw: owner.reduce((total, o) => total + (o === i + 1), 0),
    brotli: found[i].length ? brotli(Buffer.concat(found[i])) : 0,
  }))
  const rest = segments.filter((_, j) => !owner[j])
  return { rows, rest: { raw: rest.length, brotli: brotli(rest) } }
}

// --- names -----------------------------------------------------------------

/**
 * The crate of the outermost path of a v0-mangled Rust name (`_R…`), and
 * whether the name contains `core::fmt::`, as its demangled form would. Only
 * the first component of each path is followed, so no type or generic
 * argument has to be skipped; `<T as Trait>` takes T's crate when T is a path.
 */
export function crateOf(symbol, core) {
  if (!symbol.startsWith('_R')) {
    // The core's exports: chapter 08 §4.2's lily_*, chapter 03 Appendix B's lsb_* and the parser's lsp_*.
    if (/^l(ily|sb|sp)_/.test(symbol)) return { crate: core, fmt: false }
    if (/^__(rust|rdl|rg)_/.test(symbol)) return { crate: 'std', fmt: false }
    return { crate: symbol ? 'unmangled' : 'unnamed', fmt: false }
  }
  const s = symbol.slice(2).replace(/[.$].*$/, '')
  const base62 = (at) => {
    if (s[at] === '_') return { value: 0, end: at + 1 }
    let value = 0
    for (; at < s.length && s[at] !== '_'; at++) value = value * 62 + '0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ'.indexOf(s[at])
    return { value: value + 1, end: at + 1 }
  }
  const ident = (at) => {
    if (s[at] === 'u') at++
    let digits = ''
    for (; /[0-9]/.test(s[at]); at++) digits += s[at]
    if (s[at] === '_') at++
    return { text: s.slice(at, at + Number(digits)), end: at + Number(digits) }
  }
  const disambiguated = (at) => (s[at] === 's' ? base62(at + 1).end : at)
  // A crate root `C`, or a backreference to one: its name and where it ends.
  const root = (at) => {
    if (s[at] === 'B') {
      const { value, end } = base62(at + 1)
      return s[value] === 'C' ? { text: root(value).text, end } : undefined
    }
    return s[at] === 'C' ? ident(disambiguated(at + 1)) : undefined
  }
  const path = (at, depth = 0) => {
    if (depth > 64 || at >= s.length) return undefined
    switch (s[at]) {
      case 'C': return root(at).text
      case 'B': return path(base62(at + 1).value, depth + 1)
      case 'N': return path(at + 2, depth + 1)
      case 'I': return path(at + 1, depth + 1)
      case 'M': case 'X': return path(disambiguated(at + 1), depth + 1)
      case 'Y': return type(at + 1, depth + 1)
      default: return undefined
    }
  }
  const type = (at, depth) => {
    if (/[CBNIMXY]/.test(s[at])) return path(at, depth)
    if (/[RQ]/.test(s[at])) return type(s[at + 1] === 'L' ? base62(at + 2).end : at + 1, depth + 1)
    if (/[POAST]/.test(s[at])) return type(at + 1, depth + 1)
    return undefined
  }
  let fmt = false
  for (let at = s.indexOf('Nt'); at >= 0 && !fmt; at = s.indexOf('Nt', at + 1)) {
    const parent = root(at + 2)
    fmt = parent?.text === 'core' && ident(parent.end).text === 'fmt'
  }
  let crate = path(0) ?? 'other'
  // A generic instance ends with the crate that made it.
  for (let at = s.length - 1; at >= 0; at--) {
    const made = /[BC]/.test(s[at]) ? root(at) : undefined
    if (!made?.text || made.end !== s.length) continue
    if (made.text.startsWith('lily_') && !crate.startsWith('lily_')) crate = made.text
    break
  }
  return { crate, fmt }
}

// --- origins (H4, D225) -------------------------------------------------------

/**
 * The key and direct calls (`call`, `return_call`) of each function of
 * `file`, in order, read from wasm-dis's text; undefined if wasm-dis fails or
 * does not give `count` functions. The key is the name Binaryen gives the
 * function, which wasm-opt keeps: the name section's, with `_<index>` added
 * to each but the first of a name given twice, and the index for no name.
 */
export function callGraph(file, count, wasmDis) {
  const dis = spawnSync(wasmDis, [file], { encoding: 'utf8', maxBuffer: 2 ** 30 })
  if (dis.error || dis.status !== 0) return undefined
  const graph = []
  let calls
  for (const line of dis.stdout.split('\n')) {
    if (line.startsWith(' (func $')) {
      const end = line.indexOf(' ', 8)
      calls = []
      graph.push({ key: line.slice(8, end < 0 ? undefined : end), calls })
    } else if (line.startsWith(' (')) calls = undefined
    else if (calls) {
      const call = line.match(/\((?:return_)?call \$([^\s)]+)/)
      if (call) calls.push(call[1])
    }
  }
  return graph.length === count ? graph : undefined
}

/**
 * The optimised core's functions by origin: for each of `post`'s functions,
 * a Map from row to bytes. `pre` and `post` are the core before and after
 * wasm-opt, each `{ bodies, graph }`; `rowOf(name)` is a function's row by
 * name, and `engine(row)` whether the row is the engine's. A function kept
 * by name holds its own code and, by their sizes in `pre`, that of the
 * functions gone from `post` that it reaches through gone functions only —
 * those wasm-opt inlined into it — but for one whose body is a kept one's,
 * which duplicate elimination removed; a gone function's code is its row's
 * if that is the engine's, else the holder's. A function of no name is
 * shared among its callers' origins, by call site.
 */
export function origins(pre, post, rowOf, engine) {
  const before = new Map(pre.graph.map(({ key, calls }, i) => [key, { calls, body: pre.bodies[i].body, row: rowOf(pre.bodies[i].name) }]))
  const after = new Set(post.graph.map(({ key }) => key))
  const gone = (key) => before.has(key) && !after.has(key)
  const keptBodies = new Set([...before].filter(([key]) => after.has(key)).map(([, f]) => Buffer.from(f.body).toString('latin1')))
  const inlined = (key) => gone(key) && !keptBodies.has(Buffer.from(before.get(key).body).toString('latin1'))
  const add = (map, row, bytes) => map.set(row, (map.get(row) ?? 0) + bytes)
  const total = (map) => [...map.values()].reduce((sum, bytes) => sum + bytes, 0)
  const scaled = (map, size) => new Map([...map].map(([row, bytes]) => [row, (size * bytes) / total(map)]))
  const result = post.graph.map(() => undefined)
  post.graph.forEach(({ key }, i) => {
    const own = before.get(key)
    if (!post.bodies[i].name || !own) return
    const shares = new Map([[own.row, own.body.length]])
    const seen = new Set()
    for (const stack = [...own.calls]; stack.length;) {
      const call = stack.pop()
      if (seen.has(call) || !inlined(call)) continue
      seen.add(call)
      const f = before.get(call)
      add(shares, engine(f.row) ? f.row : own.row, f.body.length)
      stack.push(...f.calls)
    }
    result[i] = scaled(shares, post.bodies[i].body.length)
  })
  const callers = new Map()
  post.graph.forEach(({ calls }, i) => calls.forEach((call) => callers.set(call, [...(callers.get(call) ?? []), i])))
  post.graph.forEach(({ key }, i) => {
    if (post.bodies[i].name) return
    const shares = new Map()
    for (const caller of callers.get(key) ?? []) {
      const from = result[caller] ?? new Map([[rowOf(post.bodies[caller].name), 1]])
      for (const [row, bytes] of from) add(shares, row, bytes / total(from))
    }
    result[i] = shares.size ? scaled(shares, post.bodies[i].body.length) : undefined
  })
  return result.map((map, i) => map ?? new Map([[rowOf(post.bodies[i].name), post.bodies[i].body.length]]))
}

// --- the font packs ---------------------------------------------------------

/** Prints the font packs of `dir` (W1) against their budgets; says so when there are none. */
export function fontReport(dir) {
  const files = existsSync(dir) ? readdirSync(dir).filter((file) => file.endsWith('.pack')).sort() : []
  if (!files.length) {
    console.log(`No font packs in ${dir}: run tools/packs/build.mjs and text.mjs.\n`)
    return
  }
  const sizes = new Map(files.map((file) => {
    const bytes = readFileSync(`${dir}/${file}`)
    return [file, { raw: bytes.length, brotli: brotli(bytes) }]
  }))
  const rows = []
  for (const { kind, test, each, all, source } of FONT_BUDGETS) {
    const group = files.filter((file) => test.test(file))
    for (const file of group) {
      const { raw, brotli } = sizes.get(file)
      rows.push([file, kind, n(raw), n(brotli), budget(each), each ? status(brotli, each) : '', source])
    }
    if (all && group.length) {
      const sum = (key) => group.reduce((total, file) => total + sizes.get(file)[key], 0)
      rows.push([`all ${group.length} ${kind}s`, '', n(sum('raw')), n(sum('brotli')), budget(all), status(sum('brotli'), all), 'README §4.2'])
    }
  }
  const core = CORE_FONTS.files.filter((file) => sizes.has(file))
  const sum = (key) => core.reduce((total, file) => total + sizes.get(file)[key], 0)
  rows.push(['fonts in the core', `${core.length} of ${CORE_FONTS.files.length}`, n(sum('raw')), n(sum('brotli')), budget(CORE_FONTS.budget), status(sum('brotli'), CORE_FONTS.budget), 'README §4.1'])
  console.log(table(['dist/engine/fonts', '', 'raw', 'brotli', 'brotli KB Must / Goal', '', ''], rows))
  console.log(`The fonts in the core (06 §5.2): ${CORE_FONTS.files.join(', ')}.\n`)
}

// --- the report ------------------------------------------------------------

/** The row of a part's borrowed functions: their crate and its modules' name. */
const borrowed = (part) => `${part.borrows.crate}: ${part.borrows.name}`
/** The row of the functions whose names contain core::fmt::, the driver row's. */
const FMT = 'core::fmt (H4)'

/**
 * Prints core.wasm's code by origin (H4, D225) beside its code by name
 * (`byName`, each built part's `measure`): each part's raw bytes, and its
 * brotli bytes at its ratio by name; then the other parts' code that
 * wasm-opt put into the core's functions, and where the bodies of no name go.
 */
function originReport({ root, core, bodies, rowOf, engine, byName }) {
  // Since K4 (MS4 step 20) the core before wasm-opt is wasm-split's primary, with the trampolines it adds.
  const split = `${root}/target/engine/split/core.wasm`
  const rustc = existsSync(split) && core === 'lily_engine_core' ? split : `${root}/target/wasm32-unknown-unknown/wasm-names/${core}.wasm`
  const named = `${root}/target/engine/core.names.wasm`
  const wasmDis = process.env.WASM_OPT ? `${dirname(process.env.WASM_OPT)}/wasm-dis` : 'wasm-dis'
  if (!existsSync(rustc)) {
    console.log(`No ${rustc}: the code is not given by origin.\n`)
    return
  }
  const pre = { bodies: functions(readFileSync(rustc)).bodies }
  pre.graph = callGraph(rustc, pre.bodies.length, wasmDis)
  const post = { bodies, graph: callGraph(named, bodies.length, wasmDis) }
  if (!pre.graph || !post.graph) {
    console.log(`${wasmDis} did not read both cores: the code is not given by origin.\n`)
    return
  }
  const known = new Set(pre.graph.map(({ key }) => key))
  const strangers = post.graph.filter(({ key }, i) => bodies[i].name && !known.has(key)).length
  if (strangers) {
    console.log(`${strangers} functions of core.names.wasm are not in ${rustc}, another build: the code is not given by origin.\n`)
    return
  }
  const shares = origins(pre, post, rowOf, engine)
  const partOf = (row) => PARTS.find((part) => part.crates.includes(row) || (part.borrows && row === borrowed(part))) ?? (engine(row) ? undefined : PARTS.find((part) => part.runtime))
  const ratio = (part) => (byName.get(part)?.raw ? byName.get(part).brotli / byName.get(part).raw : 0)
  const raw = new Map(), packed = new Map()
  const add = (map, key, bytes) => map.set(key, (map.get(key) ?? 0) + bytes)
  // The other parts' code in the core's functions, by part, and in the largest holder; the bodies of no name, by part.
  const inCore = new Map(), noName = new Map()
  let holder = { name: '', size: 0, foreign: 0 }
  bodies.forEach(({ name, body }, i) => {
    const host = partOf(rowOf(name))
    let foreign = 0
    for (const [row, bytes] of shares[i]) {
      const part = partOf(row)
      if (!part) continue
      add(raw, part, bytes)
      add(packed, part, bytes * ratio(host))
      if (!name) add(noName, part, bytes)
      else if (rowOf(name) === core && part !== host) {
        add(inCore, part, bytes)
        foreign += bytes
      }
    }
    if (foreign > holder.foreign) holder = { name, size: body.length, foreign }
  })
  const rows = []
  for (const part of PARTS) {
    if (!byName.has(part)) continue
    const [r, b] = [Math.round(raw.get(part) ?? 0), Math.round(packed.get(part) ?? 0)]
    rows.push([`${part.name} (${part.owner})`, n(byName.get(part).raw), n(r), n(byName.get(part).brotli), n(b), budget(part.raw), budget(part.brotli), `raw ${status(r, part.raw)}, brotli ${status(b, part.brotli)}`])
  }
  console.log(table(['core.wasm code by origin (H4, D225)', 'raw by name', 'by origin', 'brotli ≈ by name', 'by origin', 'raw KB Must / Goal', 'brotli KB Must / Goal', 'by origin'], rows))
  const list = (map) => [...map].sort(([, a], [, b]) => b - a).map(([part, bytes]) => `${part.owner} ${n(Math.round(bytes))}`).join(', ')
  const total = (map) => Math.round([...map.values()].reduce((sum, bytes) => sum + bytes, 0))
  const unnamed = bodies.filter(({ name }) => !name)
  console.log(`Other parts' code that wasm-opt put into ${core}'s functions: ${n(total(inCore))} bytes (${list(inCore)}), ` +
    `${n(Math.round(holder.foreign))} of them in ${holder.name} (${n(holder.size)} bytes).`)
  console.log(`The ${unnamed.length} functions of no name (${n(unnamed.reduce((sum, { body }) => sum + body.length, 0))} bytes), ` +
    `by their callers: ${list(noName)}.\n`)
}

/** Prints the report for `root`, the engine's package, whose crate `core` is core.wasm. */
export function report(root, core = 'lily_engine_core') {
  const dist = `${root}/dist/engine`
  if (!existsSync(`${dist}/manifest.json`)) {
    console.log(`No engine build in ${dist}: run scripts/build.mjs.`)
    return
  }
  const manifest = JSON.parse(readFileSync(`${dist}/manifest.json`, 'utf8'))
  const packs = Object.values(manifest.modules).filter((module) => module.kind === 'pack').map((module) => module.file)
  const splits = Object.values(manifest.modules).filter((module) => module.kind === 'split').map((module) => module.file)
  const preload = manifest.core.preload.map((name) => manifest.modules[name].file)
  const rows = []
  const sum = { raw: 0, brotli: 0, packs: 0 }
  const coreRaw = readFileSync(`${dist}/${manifest.core.wasm}`).length
  for (const file of [manifest.core.wasm, manifest.core.image, ...Object.values(manifest.modules).map((module) => module.file)]) {
    const bytes = readFileSync(`${dist}/${file}`)
    const compressed = brotli(bytes)
    const first = file === manifest.core.wasm || file === manifest.core.image || preload.includes(file)
    if (first) {
      sum.raw += bytes.length
      sum.brotli += compressed
    }
    if (first && packs.includes(file)) sum.packs += compressed
    const pair = file === manifest.core.image ? FILE_BUDGETS.image : splits.includes(file) ? (SPLIT_BROTLI[file] ?? MODULE_BROTLI) : undefined
    rows.push([file + (first ? '' : ' (on demand)'), n(bytes.length), n(gzip(bytes)), n(compressed), budget(pair), pair ? status(compressed, pair) : ''])
  }
  // Not in the manifest, and so not in the download: the image of the files the interpreter loads so far.
  if (existsSync(`${dist}/scheme.image`)) {
    const bytes = readFileSync(`${dist}/scheme.image`)
    rows.push(['scheme.image (the interpreter\'s, no manifest)', n(bytes.length), n(gzip(bytes)), n(brotli(bytes)), '', ''])
  }
  // The debug build (08 §1.3, D262), which the page loads only when asked: no budget counts it.
  if (manifest.core.debug && existsSync(`${dist}/${manifest.core.debug}`)) {
    const bytes = readFileSync(`${dist}/${manifest.core.debug}`)
    rows.push([`${manifest.core.debug} (asked for, no budget)`, n(bytes.length), n(gzip(bytes)), n(brotli(bytes)), '', ''])
  }
  // W7's module (06 §7.3, D270), fetched on the first export and no part of the download: README §4.2's `pdf`, its fonts apart.
  if (existsSync(`${dist}/pdf.wasm`)) {
    const bytes = readFileSync(`${dist}/pdf.wasm`)
    const compressed = brotli(bytes)
    rows.push(['pdf.wasm (on export, without its fonts)', n(bytes.length), n(gzip(bytes)), n(compressed), budget(FILE_BUDGETS.pdf), status(compressed, FILE_BUDGETS.pdf)])
  }
  rows.push([`packs in the core's download`, '', '', n(sum.packs), budget(FILE_BUDGETS.packs), status(sum.packs, FILE_BUDGETS.packs)])
  rows.push([`core's download, without the worker`, n(sum.raw), '', n(sum.brotli), budget(FILE_BUDGETS.sum), status(sum.brotli, FILE_BUDGETS.sum)])
  const t02 = T02_OUTLINES.filter((name) => manifest.modules[name]).map((name) => readFileSync(`${dist}/${manifest.modules[name].file}`))
  if (t02.length) {
    const raw = sum.raw + t02.reduce((total, bytes) => total + bytes.length, 0)
    const compressed = sum.brotli + t02.reduce((total, bytes) => total + brotli(bytes), 0)
    rows.push([`with T02's outlines (${T02_OUTLINES.join(', ')}; B1, D262)`, n(raw), '', n(compressed), budget(FILE_BUDGETS.sum), status(compressed, FILE_BUDGETS.sum)])
  }
  console.log(table(['dist/engine', 'raw', 'gzip -9', 'brotli', 'brotli KB Must / Goal', ''], rows))
  console.log(`core.wasm against B3: ${n(coreRaw)} raw bytes, Must / Goal ${budget(FILE_BUDGETS.coreRaw)} KB, ${status(coreRaw, FILE_BUDGETS.coreRaw)}`)
  console.log(existsSync(`${dist}/scheme.image`)
    ? `scheme.image holds lily.scm's core files (03 §6), stubs of the parts on demand (D74) and the whole init, declarations-init.ly with the files it includes (D78)${core === 'lily_engine_core' ? '; core.image is it after a mark (K2)' : ''}.\n`
    : 'No scheme.image: the build had no LilyPond 2.26.0 scm/lily.\n')
  fontReport(`${dist}/fonts`)

  const named = `${root}/target/engine/core.names.wasm`
  if (!existsSync(named)) {
    console.log(`No ${named}: the code is not attributed.`)
    return
  }
  const { code, data, segments, bodies } = functions(readFileSync(named))
  // Each crate's function bodies; those whose names contain core::fmt:: apart.
  const crates = new Map()
  const fmt = []
  // Each part's modules' function bodies, by the module's row.
  const modules = new Map(PARTS.flatMap((part) => (part.modules ?? []).map((module) => [module, []])))
  // A function's row by name: its crate, a part's borrowed modules, or core::fmt.
  const rowOf = (name) => {
    const of = crateOf(name, core)
    if (of.fmt) return FMT
    const borrower = PARTS.find((part) => part.borrows?.crate === of.crate && part.borrows.test.test(name))
    return borrower ? borrowed(borrower) : of.crate
  }
  for (const { body, name } of bodies) {
    const of = crateOf(name, core)
    if (of.fmt) fmt.push(body)
    else {
      const key = rowOf(name)
      const borrower = key !== of.crate
      if (!crates.has(key)) crates.set(key, [])
      crates.get(key).push(body)
      if (borrower) continue
      for (const part of PARTS) {
        if (!part.crates.includes(of.crate)) continue
        for (const module of part.modules ?? []) if (module.test.test(name)) modules.get(module).push(body)
      }
    }
  }
  const measure = (group) => ({ count: group.length, raw: group.reduce((total, body) => total + body.length, 0), brotli: brotli(Buffer.concat(group)) })
  const engine = (crate) => crate.startsWith('lily_')
  const codeRows = []
  // Each built part's code by name, for the code by origin.
  const byName = new Map()
  for (const part of PARTS) {
    const members = [...crates.keys()].filter((crate) => part.crates.includes(crate) || (part.borrows && crate === borrowed(part)) || (part.runtime && !engine(crate)))
    const groups = members.map((crate) => [crate, measure(crates.get(crate))]).sort(([, a], [, b]) => b.raw - a.raw)
    if (part.runtime && fmt.length) groups.push([FMT, measure(fmt)])
    // A crate of the part that core.wasm does not link yet has no code in it.
    const unlinked = part.crates.filter((crate) => !crates.has(crate)).map((crate) => [`  ${crate}`, '', 'not linked'])
    if (!groups.length) {
      codeRows.push([`${part.name} (${part.owner})`, '', 'not built', '', budget(part.raw), '', budget(part.brotli), ''], ...unlinked)
      continue
    }
    const all = measure(members.flatMap((crate) => crates.get(crate)).concat(part.runtime ? fmt : []))
    // Both statuses, so that a brotli miss does not hide behind a raw one (MS2 step 42).
    const verdict = `raw ${status(all.raw, part.raw)}, brotli ${status(all.brotli, part.brotli)}`
    byName.set(part, all)
    codeRows.push([`${part.name} (${part.owner})`, n(all.count), n(all.raw), n(all.brotli), budget(part.raw), '', budget(part.brotli), verdict])
    for (const [crate, g] of groups) codeRows.push([`  ${crate}`, n(g.count), n(g.raw), n(g.brotli)])
    // The driver row split into the core's functions and the rest (MS3 part 3, *The image and the driver row*).
    if (part.runtime) {
      const rest = measure(members.filter((crate) => crate !== core).flatMap((crate) => crates.get(crate)).concat(fmt))
      codeRows.push([`  the rest: all but ${core}`, n(rest.count), n(rest.raw), n(rest.brotli)])
    }
    for (const module of part.modules ?? []) {
      const g = measure(modules.get(module))
      codeRows.push([`    ${module.name}`, n(g.count), n(g.raw), n(g.brotli)])
    }
    codeRows.push(...unlinked)
  }
  const budgeted = new Set([...PARTS.flatMap((part) => part.crates), ...PARTS.filter((part) => part.borrows).map(borrowed)])
  for (const [crate, group] of crates) {
    if (!engine(crate) || budgeted.has(crate)) continue
    const g = measure(group)
    if (MODULES[crate] && splits.includes(`${MODULES[crate]}.wasm`)) {
      codeRows.push([`${crate}: what the core keeps of the ${MODULES[crate]} module`, n(g.count), n(g.raw), n(g.brotli), '', '', '', 'split (K4)'])
      continue
    }
    if (MODULES[crate]) {
      codeRows.push([`${crate}: the ${MODULES[crate]} module (05 §9), in the core`, n(g.count), n(g.raw), n(g.brotli), '', '', budget(MODULE_BROTLI), `brotli ${status(g.brotli, MODULE_BROTLI)}`])
      continue
    }
    codeRows.push([`${crate}: ${UNBUDGETED[crate] ?? 'no part in README §4.1 yet'}`, n(g.count), n(g.raw), n(g.brotli), '', '', '', 'no share'])
  }
  console.log(table(['core.wasm code by crate', 'fns', 'raw', 'brotli ≈', 'raw KB Must / Goal', '', 'brotli KB Must / Goal', ''], codeRows))
  const h4 = measure(fmt)
  console.log(`code section ${n(code)} bytes in ${n(bodies.length)} functions; data section ${n(data)} bytes`)
  console.log(`H4, core::fmt: ${n(h4.raw)} bytes in ${n(h4.count)} functions (reported; IMPLEMENT.md allows core::fmt)\n`)
  originReport({ root, core, bodies, rowOf, engine, byName })

  // The data section by crate (OPEN H2), the crates of crates/ by their source.
  const sources = readdirSync(`${root}/crates`).filter((dir) => existsSync(`${root}/crates/${dir}/src`) && crates.has(`lily_engine_${dir}`))
  const byCrate = dataByCrate(segments, sources.map((dir) => ({ crate: `lily_engine_${dir}`, strings: sourceStrings(`${root}/crates/${dir}/src`) })))
  const dataRows = byCrate.rows.filter((row) => row.count).sort((a, b) => b.raw - a.raw).map((row) => [`strings of ${row.crate}`, n(row.count), n(row.raw), n(row.brotli)])
  dataRows.push(['in no crate\'s string of its source', '', n(byCrate.rest.raw), n(byCrate.rest.brotli)])
  console.log(table(['core.wasm data by crate (H2)', 'strings', 'raw', 'brotli ≈'], dataRows))
  console.log(`The data segments' ${n(segments.length)} bytes, charged to no part's budget: a crate's string literals of ${SOURCE_STRING} bytes or more found in them byte for byte.`)
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) report(new URL('..', import.meta.url).pathname.replace(/\/$/, ''))
