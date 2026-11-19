// engine.js (chapter 08 §7 of docs/research/wasm-engine, D171): loaded by the
// web's page before app.js, so that the engine's worker fetches its files
// while app.js loads. It starts the engine for `?engine=1`, for
// `?engine=debug` with the debug core (08 §1.3), and, in a site built with
// `--engine-default`, for a page that asks nothing (D174); `?engine=0` refuses
// it, and the page compiles on the server, as before. The web transport, in app.js, finds the
// engine as `window.lilyEngine` and attaches what the engine asks of the
// page: the files it reads, and where a page's cells go.
import { EngineHost } from './host'
import type { PageUpdate } from './protocol'

/** What the page asks of the engine: on, with the core as shipped or the debug one, or off. */
export type EngineMode = 'on' | 'debug' | 'off'

/**
 * `?engine=1` or `?engine=debug` in the page's query; `?engine=0` refuses the
 * engine; any other query is the site's default (D174).
 */
export function engineMode(search: string, byDefault: boolean): EngineMode {
  const asked = new URLSearchParams(search).get('engine')
  if (asked === '1') return 'on'
  if (asked === 'debug') return 'debug'
  if (asked === '0') return 'off'
  return byDefault ? 'on' : 'off'
}

/**
 * Whether the site runs the engine for a page that asks nothing: esbuild's
 * `--engine-default` (D174), which waits for the engine's source to be public
 * (D56, OPEN I5).
 */
declare const LILY_ENGINE_DEFAULT: boolean | undefined
export const engineByDefault = typeof LILY_ENGINE_DEFAULT === 'boolean' && LILY_ENGINE_DEFAULT

/** What the web's page gives the engine, once app.js has its transport. */
export interface EnginePage {
  /** A file's bytes as read, undecoded; null when there is none (D102). */
  readFile(path: string): Promise<Uint8Array | null>
  /** What changed of a page on screen, or the page shown whole, before its compile's outcome (W8, I7). */
  onCells(cells: PageUpdate): void
}

/** The engine of the page, as engine.js left it for app.js. */
export interface PageEngine {
  host: EngineHost
  /** Gives the engine the page; files it asks for before then wait for it. */
  attach(page: EnginePage): void
}

declare global {
  interface Window {
    lilyEngine?: PageEngine
  }
}

/** Starts the engine with a page that comes later. */
export function startEngine(debug: boolean, start = EngineHost.start): PageEngine {
  let attached: (page: EnginePage) => void = () => {}
  const page = new Promise<EnginePage>((resolve) => (attached = resolve))
  let cells: EnginePage['onCells'] = () => {}
  const host = start(
    async (path) => (await page).readFile(path),
    (changed) => cells(changed),
    debug,
  )
  return {
    host,
    attach: (given) => {
      cells = (changed) => given.onCells(changed)
      attached(given)
    },
  }
}

const mode = typeof location === 'undefined' ? 'off' : engineMode(location.search, engineByDefault)
if (mode !== 'off') window.lilyEngine = startEngine(mode === 'debug')
