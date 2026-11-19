// The engine's examples the tools' tests run (MS3 step 1): each built once
// with cargo +stable and run from target/ as a binary, where the tests ran
// `cargo +stable run` each time and concurrent runs now and then killed or
// hung an example (MS2 steps 28, 33–35 and 37–42). A binary older than a file
// of its dep-info (target/<profile>/examples/<name>.d: its crates' sources),
// or than the manifests, is built again before it runs, never run stale.
//
//   node tools/examples.mjs     builds the six the tests run (BUILT), as
//                               npm run test:tools does before node --test
import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync, statSync, utimesSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const engine = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const target = path.resolve(engine, process.env.CARGO_TARGET_DIR ?? 'target')
/** Each example's crate. */
const CRATES = { compile: 'lily-engine-core', bench: 'lily-engine-core', parse: 'lily-engine-parser', write: 'lily-engine-svg' }
/** The examples the tests run, and whether in release. */
export const BUILT = [['compile', false], ['parse', false], ['bench', true], ['parse', true], ['write', false], ['compile', true]]

/** `name`'s binary under target/. */
function binary(name, release) {
  return path.join(target, release ? 'release' : 'debug', 'examples', name + (process.platform === 'win32' ? '.exe' : ''))
}

/** The files `name`'s binary is built from: its dep-info's, the workspace's manifests and lock; undefined without a dep-info. */
function sources(name, release) {
  const info = path.join(target, release ? 'release' : 'debug', 'examples', `${name}.d`)
  if (!existsSync(info)) return undefined
  // `<binary>: <file> <file> …`, a space in a name escaped.
  const files = readFileSync(info, 'utf8').split('\n')[0].replace(/^.*?[^\\]: /, '')
    .split(/(?<!\\) /).filter(Boolean).map((file) => file.replaceAll('\\ ', ' '))
  const crates = new Set(files.map((file) => path.relative(engine, file).match(/^crates\/[^/]+/)?.[0]).filter(Boolean))
  return [...files, 'Cargo.toml', 'Cargo.lock', ...[...crates].map((crate) => `${crate}/Cargo.toml`)]
    .map((file) => path.resolve(engine, file))
}

/** Why `name`'s binary must be built: none, or a source newer than it; else false. */
function stale(name, release) {
  const file = binary(name, release)
  const files = existsSync(file) && sources(name, release)
  if (!files) return `no ${path.relative(engine, file)}`
  const built = statSync(file).mtimeMs
  const newer = files.find((source) => !existsSync(source) || statSync(source).mtimeMs > built)
  return newer ? `${path.relative(engine, newer)} is newer than ${path.relative(engine, file)}` : false
}

const checked = new Set()
/** `name`'s binary, built first if it is stale (once a process). */
export function example(name, release = false) {
  const file = binary(name, release)
  if (checked.has(file)) return file
  if (stale(name, release)) {
    const build = spawnSync('cargo', ['+stable', 'build', '-q', '--manifest-path', path.join(engine, 'Cargo.toml'),
      '-p', CRATES[name], '--example', name, ...(release ? ['--release'] : [])], { stdio: ['ignore', 'inherit', 'inherit'] })
    if (build.status !== 0) throw new Error(`cargo +stable build of the ${name} example failed`)
    // Still older than a manifest, cargo found it fresh: the time it was found so.
    if (stale(name, release)) utimesSync(file, new Date(), new Date())
  }
  checked.add(file)
  return file
}

/**
 * Runs the example `name` with `args` as `cargo +stable run -q [--release] -p <crate> --example <name> -- …args` ran
 * it, through spawnSync with `options`, `options.release` choosing the release build.
 */
export function runExample(name, args, { release = false, ...options } = {}) {
  return spawnSync(example(name, release), args, options)
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  if (spawnSync('cargo', ['+stable', '--version']).status !== 0) {
    console.log('cargo +stable is not installed: the examples are not built, and the tests that run them skip')
  } else {
    for (const [name, release] of BUILT) example(name, release)
  }
}
