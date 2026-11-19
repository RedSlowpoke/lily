// K4's split (08 §2.3–§2.4, D223): which functions of the core as rustc
// linked it, with its names (the wasm-names profile, before wasm-opt), go to
// each notation module, read from the call graph as wasm-dis gives it.
// scripts/build.mjs then gives the sets to Binaryen's wasm-split
// --multi-split.
//
// A module's functions are those reachable only from its own crate (or its
// own module of a crate, SPLIT_PATHS). The core
// keeps every function reachable from its roots: its exports and the table's
// functions of no module's crate, through the direct calls (`call`,
// `return_call`) and `ref.func`. A function of a module's crate the core
// reaches so stays in the core (the crates' `install`, which `lsb_init`
// calls). A module takes the rest of its crate and every function reachable
// from it that the core does not reach; one reachable from two modules stays
// in the core. A function of a module's crate in the table is reached by the
// core only through `call_indirect`, which wasm-split's placeholder answers
// until the module is loaded: a score that calls it without its trigger is a
// restart (08 §4.4), as packages/studio/test/engine-corpus.test.ts checks
// the 30 for.
//
//   node scripts/split.mjs [core.wasm]    prints each module's count and the
//                                         functions of its crate kept in the core
import { spawnSync } from 'node:child_process'
import { pathToFileURL } from 'node:url'
import { crateOf } from './sizes.mjs'

/**
 * K4's split modules by crate (D223), sizes.mjs' MODULES but the figured
 * bass's: LilyPond's Staff consists `Figured_bass_engraver` and
 * `Figured_bass_position_engraver` (engraver-init.ly), so every score with a
 * staff makes them, and the crate stays in the core (D230).
 */
export const SPLIT = { lily_engine_tablature: 'tablature', lily_engine_chord_names: 'chord-names' }

/**
 * Split modules of a module of a crate, by the path its functions' v0 names
 * hold (D273): the scheme crate's `syntax_case`, chapter 03 §6's
 * `syntax-case` part, which the expander and Scheme reach only through
 * the function table (`expand.rs`' hooks, its primitives).
 */
export const SPLIT_PATHS = { syntax_case: { crate: 'lily_engine_scheme', module: 'syntax-case' } }

/** The module of `name` that is a path's of SPLIT_PATHS, if one is. */
function pathModule(name) {
  for (const [path, { crate, module }] of Object.entries(SPLIT_PATHS)) {
    const at = name.indexOf(`${crate.length}${crate}`)
    if (name.startsWith('_R') && at >= 0 && name.indexOf(`${path.length}${path}`, at) >= 0) return module
  }
  return undefined
}

/**
 * The module's functions of `file` by module name, each sorted, and the
 * functions of each module's crate the core keeps; `wasmDis` is Binaryen's.
 * The names are Binaryen's, which wasm-split's manifest names them by.
 */
export function splitSets(file, wasmDis = 'wasm-dis') {
  const dis = spawnSync(wasmDis, [file], { encoding: 'utf8', maxBuffer: 2 ** 30 })
  if (dis.error || dis.status !== 0) throw new Error(`${wasmDis} ${file}: ${dis.error ?? dis.stderr}`)
  const edges = new Map()
  const roots = []
  const table = []
  let calls
  for (const line of dis.stdout.split('\n')) {
    if (line.startsWith(' (func $')) {
      const end = line.indexOf(' ', 8)
      calls = []
      edges.set(line.slice(8, end < 0 ? undefined : end), calls)
      continue
    }
    if (line.startsWith(' (')) calls = undefined
    if (calls) {
      for (const m of line.matchAll(/\((?:return_call|call|ref\.func) \$([^\s)]+)/g)) calls.push(m[1])
    } else if (line.startsWith(' (export ')) {
      const m = line.match(/\(func \$([^\s)]+)\)/)
      if (m) roots.push(m[1])
    } else if (line.startsWith(' (start ')) {
      roots.push(line.match(/\$([^\s)]+)/)[1])
    } else if (line.startsWith(' (elem ')) {
      table.push(...[...line.matchAll(/ \$([^\s)]+)/g)].map((m) => m[1]))
    }
  }
  // The element segment comes before the functions; its own name is no function's.
  table.splice(0, table.length, ...table.filter((name) => edges.has(name)))
  if (!table.length) throw new Error(`${file}: no table read`)
  const moduleOf = (name) => SPLIT[crateOf(name, 'lily_engine_core').crate] ?? pathModule(name)
  const reach = (from, stop = () => false) => {
    const seen = new Set()
    for (const stack = [...from]; stack.length;) {
      const name = stack.pop()
      if (seen.has(name) || stop(name)) continue
      seen.add(name)
      stack.push(...(edges.get(name) ?? []))
    }
    return seen
  }
  const core = reach([...roots, ...table.filter((name) => !moduleOf(name))])
  const sets = {}
  const kept = {}
  for (const module of [...Object.values(SPLIT), ...Object.values(SPLIT_PATHS).map((p) => p.module)]) {
    const own = [...edges.keys()].filter((name) => moduleOf(name) === module)
    kept[module] = own.filter((name) => core.has(name)).sort()
    sets[module] = reach(own.filter((name) => !core.has(name)), (name) => core.has(name))
  }
  // Reachable from two modules: the core's.
  const count = new Map()
  for (const set of Object.values(sets)) for (const name of set) count.set(name, (count.get(name) ?? 0) + 1)
  for (const module of Object.keys(sets)) sets[module] = [...sets[module]].filter((name) => count.get(name) === 1).sort()
  return { sets, kept, functions: edges.size }
}

/** wasm-split's --manifest: each module's name, then its functions, a blank line between modules. */
export function manifestText(sets) {
  return Object.entries(sets).filter(([, names]) => names.length).map(([module, names]) => [module, ...names].join('\n')).join('\n\n') + '\n'
}

/**
 * `primary` and its `secondaries` (wasm-split's outputs, as bytes) with the
 * names the primary exports to them by `prefix` shortened alike: wasm-split
 * gives each the function's own, a mangled name of some hundred bytes, which
 * would make up a third of a module. Each export is `prefix` and a number in
 * base 36, in the order of the primary's exports; nothing else changes.
 */
export function shortenNames(primary, secondaries, prefix = '%', namespace = 'primary') {
  const names = new Map()
  const out = rewrite(primary, 7, (read, write) => {
    const name = read.name()
    if (name.startsWith(prefix) && !names.has(name)) names.set(name, `${prefix}${names.size.toString(36)}`)
    write.name(names.get(name) ?? name)
    write.raw(read.raw(1))
    write.u32(read.u32())
  })
  return { primary: out, secondaries: secondaries.map((bytes) => rewrite(bytes, 2, (read, write) => {
    const module = read.name()
    const name = read.name()
    write.name(module)
    write.name(module === namespace ? (names.get(name) ?? name) : name)
    const kind = read.raw(1)
    write.raw(kind)
    const limits = () => {
      const flags = read.raw(1)
      write.raw(flags)
      write.u32(read.u32())
      if (flags[0] & 1) write.u32(read.u32())
    }
    if (kind[0] === 0) write.u32(read.u32())
    else if (kind[0] === 1) {
      write.raw(read.raw(1))
      limits()
    } else if (kind[0] === 2) limits()
    else if (kind[0] === 3) write.raw(read.raw(2))
    else throw new Error(`an import of kind ${kind[0]}, which shortenNames does not read`)
  })) }
}

/** `bytes` with each entry of its section `id` (a vector) rewritten by `entry(read, write)`. */
function rewrite(bytes, id, entry) {
  let at = 8
  const u32 = () => {
    let value = 0
    for (let shift = 0; ; shift += 7) {
      const byte = bytes[at++]
      value += (byte & 0x7f) * 2 ** shift
      if (byte < 0x80) return value
    }
  }
  const leb = (value) => {
    const out = []
    do {
      let byte = value % 128
      value = Math.floor(value / 128)
      if (value) byte |= 0x80
      out.push(byte)
    } while (value)
    return Buffer.from(out)
  }
  const parts = [bytes.subarray(0, 8)]
  while (at < bytes.length) {
    const start = at
    const section = bytes[at++]
    const size = u32()
    const end = at + size
    if (section !== id) {
      parts.push(bytes.subarray(start, end))
      at = end
      continue
    }
    const body = []
    const read = {
      u32,
      raw: (n) => bytes.subarray(at, (at += n)),
      name: () => {
        const length = u32()
        return new TextDecoder().decode(bytes.subarray(at, (at += length)))
      },
    }
    const write = {
      u32: (value) => body.push(leb(value)),
      raw: (raw) => body.push(Buffer.from(raw)),
      name: (name) => body.push(leb(Buffer.byteLength(name)), Buffer.from(name)),
    }
    const count = u32()
    write.u32(count)
    for (let i = 0; i < count; i++) entry(read, write)
    if (at !== end) throw new Error(`section ${id} not read to its end`)
    const content = Buffer.concat(body)
    parts.push(Buffer.from([section]), leb(content.length), content)
  }
  return Buffer.concat(parts)
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const root = new URL('..', import.meta.url).pathname.replace(/\/$/, '')
  const file = process.argv[2] ?? `${root}/target/wasm32-unknown-unknown/wasm-names/lily_engine_core.wasm`
  const { sets, kept, functions } = splitSets(file)
  console.log(`${functions} functions in ${file}`)
  for (const [module, names] of Object.entries(sets)) {
    console.log(`${module}: ${names.length} functions; ${kept[module].length} of its crate kept in the core`)
    for (const name of kept[module]) console.log(`  kept ${name}`)
  }
}
