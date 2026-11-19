// How bridge.ts reaches the Rust side on the web (DECISIONS D55):
// `node esbuild.mjs --site` bundles this module in place of
// src/renderer/transport.ts. Calls go over the socket to the user's worker
// (connection.ts); the commands the app answers with a native panel or
// alert ask with the page's own dialogs first (dialogs.ts), then pass the
// worker what was chosen. A file the app writes for the user — a PDF, a MIDI
// file — is downloaded too; a Guitar Pro file to import (D166) is uploaded.
// The menu bar takes the app menu's place.
// Separating stems needs the Mac app: separation.ts answers those calls.
// With `?engine=1`, the engine that engine.js started compiles the live
// edits in the page (live.ts, D171), and the server what it cannot (D172);
// while the connection is down the engine goes on, and the page with it.
// The engine's music comes after its pages (D264): Export MIDI waits for it,
// and while the music plays it is performed at once. Export PDF is the
// engine's when it runs (W7, D271), its PDFs downloaded from the page; the
// server exports what the engine cannot.
import type { ExportedMidi, FileChange, MutedPart, Opened, RecentList, Transport } from '../ipc'
import { Connection } from './connection'
import { ask, Browser, exportQuestion, type Listing } from './dialogs'
import type { PageEngine } from './engine/engine'
import { aboutLinks, base64, EngineLive, pageAt, refusedSearch } from './live'
import { MenuBar } from './menubar'
import { GUITAR_PRO_ACCEPT, scoreName } from '../renderer/guitarPro'
import { separation, separationStub } from './separation'
import './web.css'

type Args = Record<string, unknown>

const connection = new Connection()
const call = <T>(command: string, args?: Args) => connection.call<T>(command, args)
const browser = new Browser(
  (dir) => call<Listing>('browse', dir ? { dir } : {}),
  (parent, name) => call<string>('create_folder', { parent, name }),
)

/** The folder the page shows; given again to a worker that does not have it (D55). */
let folder: string | undefined
let live: boolean | undefined
let dirty = false
let emit: (event: unknown) => void = () => {}

function remember<T>(result: T): T {
  const opened = result as Partial<Opened> | null
  if (opened?.listing?.folder) folder = opened.listing.folder
  return result
}

const name = (path: string) => path.split('/').pop() ?? path
/** What the worker takes as one upload (worker/commands.rs's MAX_UPLOAD). */
const MAX_UPLOAD = 2 * 1024 * 1024

function bytes(base64: string): Uint8Array<ArrayBuffer> {
  const binary = atob(base64)
  const data = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) data[i] = binary.charCodeAt(i)
  return data
}

/** Hands a file to the browser to save, as a download: its bytes, or as base64. */
function save(file: { name: string; data: string | Uint8Array }): void {
  const type = file.name.endsWith('.pdf') ? 'application/pdf' : file.name.match(/\.midi?$/) ? 'audio/midi' : 'text/plain'
  const data = typeof file.data === 'string' ? bytes(file.data) : file.data
  const url = URL.createObjectURL(new Blob([data as Uint8Array<ArrayBuffer>], { type }))
  const link = document.createElement('a')
  link.href = url
  link.download = file.name
  link.click()
  setTimeout(() => URL.revokeObjectURL(url), 60_000)
}

/** Replaces a file only when the user says so. */
async function confirmReplace(file: string): Promise<boolean> {
  const answer = await ask({
    title: `Replace “${file}”?`,
    text: 'A file with that name is already in the folder. Replacing it overwrites what it holds.',
    buttons: ['Cancel', 'Replace'],
  })
  return answer?.button === 1
}

/** The app's panels and alerts, asked on the page. */
const dialogs: Record<string, (args: Args) => Promise<unknown>> = {
  open_folder: async () => {
    const chosen = await browser.chooseFolder({ title: 'Open Folder', prompt: 'Open', start: folder })
    return chosen ? remember(await call<Opened>('open_folder', { path: chosen })) : null
  },
  open_file: async () => {
    const chosen = await browser.chooseScore({ title: 'Open Score', prompt: 'Open', start: folder })
    return chosen ? remember(await call<Opened>('open_file', { path: chosen })) : null
  },
  new_score: ({ template }) =>
    saveScoreAs('New Score', 'Create', 'Untitled.ly', (path, replace) => call('new_score', { template, path, ...(replace ? { replace } : {}) })),
  // File › Import Guitar Pro… (D166): the file is uploaded and converted
  // first, so one that cannot be imported is refused before asking where to
  // save its score. A drop is the app's: no `source` comes on the web.
  import_guitar_pro: async () => {
    const [file] = await pickFiles(GUITAR_PRO_ACCEPT, false)
    if (!file) return null
    if (file.size > MAX_UPLOAD) throw new Error(`${file.name} is too large to upload (${MAX_UPLOAD / 1024 / 1024} MB at most).`)
    const data = base64(new Uint8Array(await file.arrayBuffer()))
    await call('import_guitar_pro', { name: file.name, data })
    return saveScoreAs('Import Guitar Pro', 'Import', scoreName(file.name), (path, replace) =>
      call('import_guitar_pro', { name: file.name, data, path, ...(replace ? { replace } : {}) }),
    )
  },
  confirm_reload: async ({ file }) => {
    const answer = await ask({
      title: `${name(String(file))} was changed on the server.`,
      text: 'Reload it and lose the changes you have not saved, or keep your changes? Saving them will replace the other version.',
      buttons: ['Keep My Changes', 'Reload'],
    })
    return answer?.button === 1
  },
  // The server's LilyPond is its operator's; agents are the app's.
  choose_lilypond: async () => null,
  choose_agent: async () => null,
  open_link: async ({ link }) => {
    window.open(await call<string>('open_link', { link }), '_blank', 'noopener,noreferrer')
  },
  export_pdf: async ({ rootFile }) => {
    // The engine's PDFs, written in the page and downloaded (D271): their names alone, as nothing is written beside
    // the score. A score with errors rejects, as the server's export does; what the engine cannot export, the server does.
    const made = await engineLive?.exportPdf(String(rootFile))
    if (made) {
      for (const file of made.files) save(file)
      return made.files.map((file) => file.name)
    }
    const written = await call<string[]>('export_pdf', { rootFile })
    for (const file of written) save(await call('download', { file }))
    return written
  },
  export_midi: (args) => exportMidi('export_midi', 'Export MIDI', args),
  export_garageband: (args) => exportMidi('export_garageband', 'Export for GarageBand', args),
}

/**
 * Asks where to save a new score, `suggested` in the open folder at first,
 * and has the worker write it there with `create`. A file there is replaced
 * only when the user says so; else the panel asks again. Null when cancelled.
 */
async function saveScoreAs(
  title: string,
  prompt: string,
  suggested: string,
  create: (path: string, replace: boolean) => Promise<Opened | { exists: true }>,
): Promise<Opened | null> {
  let start = folder
  for (;;) {
    const chosen = await browser.saveAs({ title, prompt, start, name: suggested })
    if (!chosen) return null
    const created = await create(chosen, false)
    if (!('exists' in created)) return remember(created)
    const file = name(chosen).match(/\.(ly|ily|lyi)$/i) ? name(chosen) : `${name(chosen)}.ly`
    if (await confirmReplace(file)) return remember((await create(chosen, true)) as Opened)
    start = chosen.slice(0, chosen.lastIndexOf('/'))
    suggested = name(chosen)
  }
}

/**
 * Export MIDI (D54) or Export for GarageBand (D162): asks whether to leave
 * out the muted parts, if any are, then has the worker write the file and
 * downloads it. Null when cancelled.
 */
async function exportMidi(command: string, title: string, args: Args): Promise<ExportedMidi | null> {
  const muted = args.muted as MutedPart[]
  const question = exportQuestion(title, String(args.rootFile), muted.map((part) => part.name))
  let leaveOut = false
  if (question) {
    const answer = await ask(question)
    if (answer?.button !== 0) return null
    leaveOut = answer.checked
  }
  const exported = await call<{ name: string; data: string; leftOut: number }>(command, { ...args, leaveOut })
  save(exported)
  return { file: exported.name, leftOut: exported.leftOut }
}

/** Calls that change what the worker has open, kept for a worker that comes back without it. */
const REMEMBERED = new Set(['open_sample', 'open_recent'])

const menuBar = new MenuBar({
  command: (command) => emit({ type: 'command', command }),
  openRecent: (path) => emit({ type: 'openRecent', path }),
  recentList: () => call<RecentList>('recent_list'),
  clearRecent: () => void call('clear_recent').catch(console.error),
  learn: () => void dialogs.open_link({ link: 'learn' }).catch(console.error),
})

/** The page the preview shows, from 1, for the engine to lay out first (chapter 05 §7.2). */
function visiblePage(): number {
  const body = document.querySelector<HTMLElement>('[data-pane="preview"] [data-view="svg"] .pane-body')
  if (!body) return 1
  const { top, bottom } = body.getBoundingClientRect()
  return pageAt([...body.querySelectorAll('.preview-page')].map((page) => page.getBoundingClientRect()), (top + bottom) / 2)
}

/**
 * A file's bytes as the server has them (D102); null when there is no such
 * file. Rejects when the server refuses it, or at once without a connection,
 * where a call would wait for one.
 */
async function readBytes(file: string): Promise<Uint8Array | null> {
  if (!connection.online) throw new Error('No connection to the server.')
  try {
    return bytes((await call<{ data: string }>('download', { file })).data)
  } catch (error) {
    // `download`'s error for a file that is not there (commands.rs: the name, then the OS's).
    if (/No such file or directory|cannot find the file/i.test(String(error))) return null
    throw error
  }
}

// The engine, when engine.js started one (`?engine=1`, D171, or the site's default, D174).
const pageEngine: PageEngine | undefined = window.lilyEngine
const engineLive = pageEngine
  ? new EngineLive({
      engine: pageEngine.host,
      serverLive: (on) => void call('set_live', { on }).catch((error: unknown) => console.error('set_live:', error)),
      serverEdited: (file, text) => void call('edited', { file, text }).catch((error: unknown) => console.error('edited:', error)),
      readFile: readBytes,
      serverCompileSaved: (file) => void call('show_score', { file }).catch((error: unknown) => console.error('show_score:', error)),
      online: () => connection.online,
      emit: (event) => emit(event),
      visiblePage,
    })
  : undefined
if (pageEngine && engineLive) {
  pageEngine.attach({ readFile: (path) => engineLive.read(path), onCells: (cells) => engineLive.cells(cells) })
  // Without a connection the engine goes on: the page says so, and is not covered.
  void pageEngine.host.ready.then(
    () => (connection.workOffline = () => !pageEngine.host.failed),
    () => {},
  )
  // Its source and licences, in the Help menu (D56).
  void pageEngine.host.ready.then(
    () => {
      const links = aboutLinks(pageEngine.host.about, new URL('engine/', location.href).href)
      const open = links.map(({ label, url }) => ({ label, run: () => void window.open(url, '_blank', 'noopener,noreferrer') }))
      // The way to refuse the engine (D174): the page again, compiling on the server; unsaved edits are asked about first.
      menuBar.addHelp([...open, { label: 'Compile on the Server', run: () => (location.search = refusedSearch(location.search)) }])
    },
    (error: unknown) => console.warn('The engine did not start; the server compiles.', error),
  )
}

// A worker that comes back — restarted, or used by another tab — gets the
// folder and the live preview switch of this page first (D55); the switch
// off while the engine has the live compiles.
connection.onHello = async (hello, again) => {
  menuBar.setUser(hello.user)
  if (!again) return
  if (live !== undefined) await connection.callFirst('set_live', { on: engineLive ? engineLive.serverOn : live })
  if (folder) await connection.callFirst('open_folder', { path: folder })
  engineLive?.reconnected()
}

// Unsaved changes: the browser asks before the tab closes.
window.addEventListener('beforeunload', (event) => {
  if (dirty) event.preventDefault()
})

export const transport: Transport = {
  hosted: true,
  separation,
  call: async <T>(command: string, args: Args = {}): Promise<T> => {
    const stub = separationStub(command)
    if (stub) return stub
    const dialog = dialogs[command]
    if (dialog) return (await dialog(args)) as T
    const result = await call<T>(command, args)
    if (command === 'show_score') engineLive?.showScore((result as { rootFile?: string } | null)?.rootFile)
    if (REMEMBERED.has(command)) remember(result)
    if (command === 'list_folder' && result) folder = (result as unknown as { folder: string }).folder
    return result
  },
  send: (command, args = {}) => {
    if (command === 'set_dirty') {
      dirty = Boolean(args.dirty)
      return
    }
    if (command === 'set_live') {
      live = Boolean(args.on)
      // With the engine, live.ts decides the server's switch.
      if (engineLive) return engineLive.setLive(live)
    }
    if (command === 'edited') engineLive?.edited(String(args.file), typeof args.text === 'string' ? args.text : null)
    call(command, args).catch((error: unknown) => console.error(`${command}:`, error))
  },
  subscribe: (onEvent) => {
    emit = onEvent
    connection.onEvent = (event) => {
      const studio = event as { type?: string; changes?: FileChange[]; event?: { kind: 'started' | 'finished'; rootFile?: string; outcome?: { rootFile: string } } }
      if (studio.type === 'filesChanged' && studio.changes) engineLive?.changed(studio.changes.map((change) => change.file))
      const compile = studio.type === 'compile' ? studio.event : undefined
      if (compile && engineLive && !engineLive.serverCompile(compile.kind, compile.rootFile ?? compile.outcome?.rootFile ?? '')) return
      onEvent(event)
    }
    return connection.ready()
  },
  upload: async (into) => {
    const files = await pickFiles('.ly,.ily,.lyi', true)
    if (files.length === 0) return undefined
    let written: string[] = []
    let dir = into
    for (const file of files) {
      const data = base64(new Uint8Array(await file.arrayBuffer()))
      let answer = await call<{ path: string } | { exists: true }>('upload', { dir, name: file.name, data })
      if ('exists' in answer) {
        if (!(await confirmReplace(file.name))) continue
        answer = await call<{ path: string }>('upload', { dir, name: file.name, data, replace: true })
      }
      if ('path' in answer) written = [...written, answer.path]
    }
    if (written.length === 0) return undefined
    dir = written[0].slice(0, written[0].lastIndexOf('/'))
    // One score is opened; several show in their folder.
    const opened =
      written.length === 1 ? await call<Opened>('open_file', { path: written[0] }) : await call<Opened>('open_folder', { path: dir })
    return remember(opened)
  },
  download: async (file) => save(await call('download', { file })),
  music: async (rootFile) => engineLive?.music(rootFile),
  playing: (on) => engineLive?.playing(on),
}

/** The files the user picks in the browser's own file panel, of the extensions `accept` lists. */
function pickFiles(accept: string, multiple: boolean): Promise<File[]> {
  return new Promise((resolve) => {
    const input = document.createElement('input')
    input.type = 'file'
    input.multiple = multiple
    input.accept = accept
    input.addEventListener('change', () => resolve([...(input.files ?? [])]))
    input.addEventListener('cancel', () => resolve([]))
    input.click()
  })
}
