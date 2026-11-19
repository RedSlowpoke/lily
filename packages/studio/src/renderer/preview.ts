// The preview pane (DECISIONS D32): the SVG pages of the last compile, fitted
// to the pane's width and zoomable, and a click on a note that asks for its
// place in the source. Page parsing, sanitizing, zoom steps and the scroll
// anchor are @lily/common's web/preview.js, the extension's webview script; the rest is
// the webview's render() cut down to one pane without a toolbar of its own, which
// inserts the changed pages as it does (D58, D174): the one in view at once, the
// others one per frame. A page's cells (W8) go in by parts, a few per task
// (D175), so that no task of the page's parses a whole edit; then hidden, a part
// a frame, and are shown in one more (D176), so that no frame styles and lays
// out a whole edit's cells.
import {
  captureAnchor,
  clampZoom,
  insertionOrder,
  isSourceLink,
  pageRatio,
  quiet,
  replaceCells,
  resolveAnchor,
  sanitize,
  stepZoom,
  zoomLabel,
} from '@lily/common/web/preview.js'
import type { CompileEvent, CompileOutcome } from '../ipc'
import type { PageCells, PageWhole } from '../web/engine/protocol'

/** What a finished compile does to the preview. */
export interface PreviewUpdate {
  /** Show `outcome.svg` in place of the pages on screen. */
  show: boolean
  /** A line over the pages, or the text of the empty pane when there are none. */
  note?: string
}

/**
 * Pages of any run are shown, as the extension does (ARCHITECTURE §3.3); a run
 * without pages keeps the ones on screen. `hasPages`: whether pages of the same
 * score are on screen.
 */
export function previewUpdate(outcome: CompileOutcome, hasPages: boolean): PreviewUpdate {
  const pages = outcome.svg.length > 0
  switch (outcome.state) {
    case 'ok':
      return pages ? { show: true } : { show: false, note: 'LilyPond produced no pages.' }
    case 'failed':
      if (pages) return { show: true, note: 'The score has errors, so this may be incomplete.' }
      return { show: false, note: hasPages ? 'The score has errors. Showing the last version that engraved.' : 'The score has errors. Fix them to see it here.' }
    case 'no-lilypond':
    case 'error':
      return { show: false, note: hasPages ? undefined : 'The score could not be engraved.' }
    case 'no-root':
      // Nothing ran: the preview stays as it is.
      return { show: false }
  }
}

/** The page containers are this far from the pane's edges (layout.css). */
const PADDING = 16

export interface ScorePreviewOptions {
  /** The pane's scrolling body. */
  body: HTMLElement
  /** Where the zoom buttons go, in the pane's header. */
  actions: HTMLElement
  /** A click on an element that carries a `textedit:` link. */
  onReveal(href: string): void
  /** Other pages are on screen, or none (the playhead's map is out of date, D35). */
  onRender?(): void
  /** The pages changed size: zoom, or the pane was resized. */
  onLayout?(): void
  /** Ends the task, to go on in a later one (D175); a MessageChannel's message unless a test says. */
  nextTask?(): Promise<void>
  /** Goes on in the next frame, before its style and layout (D176); `requestAnimationFrame` unless a test says. */
  nextFrame?(): Promise<void>
}

/** A part of a page's new cells is parsed in a task of its own when it is this long or longer (D175). */
const CELL_PART = 32_000

export class ScorePreview {
  private readonly pages = document.createElement('div')
  private readonly empty = document.createElement('div')
  private readonly note = document.createElement('div')
  private readonly fitButton: HTMLButtonElement
  /**
   * The score whose pages are on screen, and the text of each page element
   * on screen, or that its cells are making it (D175): none for a page held
   * by a placeholder (D174).
   */
  private rootFile: string | undefined
  private shown: (string | undefined)[] = []
  /** Counts renders and clears; an insertion of an older one stops. */
  private insertion = 0
  /** The pages the newest render has yet to insert, in their order. */
  private pending: number[] = []
  /** Cells going into a page, by page: the cells of a newer compile wait for those of an older (D175). */
  private readonly cellWork = new Map<number, Promise<void>>()
  /** Whether the links and the render's callback are owed to cells that have gone in. */
  private linksOwed = false
  private zoom = 1
  private clicked: Element | undefined
  /** The elements of the pages on screen by their `textedit:` link, in page order. */
  readonly sourceLinks = new Map<string, Element[]>()

  constructor(private readonly options: ScorePreviewOptions) {
    const { body, actions } = options
    this.pages.className = 'preview-pages'
    this.empty.className = 'preview-empty placeholder'
    this.empty.textContent = 'Save a score to see it here.'
    this.note.className = 'preview-note'
    this.note.hidden = true
    body.classList.remove('placeholder')
    body.replaceChildren(this.empty, this.pages)
    body.parentElement!.append(this.note)

    const zoomButton = (label: string, title: string, action: () => void) => {
      const button = document.createElement('button')
      button.type = 'button'
      button.textContent = label
      button.title = title
      button.addEventListener('click', action)
      return button
    }
    this.fitButton = zoomButton('Fit', 'Fit to width', () => this.setZoom(1))
    actions.append(
      zoomButton('−', 'Zoom out', () => this.setZoom(stepZoom(this.zoom, -1))),
      this.fitButton,
      zoomButton('+', 'Zoom in', () => this.setZoom(stepZoom(this.zoom, 1))),
    )

    // Links must not navigate the window; a textedit: one asks for its source.
    this.pages.addEventListener('click', (event) => {
      const link = event.target instanceof Element ? event.target.closest('a') : null
      if (!link) return
      event.preventDefault()
      const href = linkHref(link)
      if (!isSourceLink(href)) return
      this.clicked?.classList.remove('current')
      link.classList.add('current')
      this.clicked = link
      options.onReveal(href)
    })
    // Ctrl/Cmd + wheel, which is also how a trackpad pinch arrives.
    body.addEventListener(
      'wheel',
      (event) => {
        if (!event.ctrlKey && !event.metaKey) return
        event.preventDefault()
        this.setZoom(this.zoom * Math.exp(-event.deltaY * 0.0025), event.clientY - body.getBoundingClientRect().top)
      },
      { passive: false },
    )
    new ResizeObserver(() => this.relayout(0, () => {})).observe(body)
    this.layout()
  }

  /**
   * The editor shows a file of `rootFile`, or of no score (undefined): pages
   * of another score leave at once, and a note says what comes (D39).
   */
  showScore(rootFile: string | undefined): void {
    if (rootFile !== undefined && rootFile === this.rootFile) return
    if (this.rootFile !== undefined) this.clear()
    this.showNote(rootFile === undefined ? 'This file is not part of a score. Open the .ly file that \\includes it to see the music.' : 'Engraving…')
  }

  compiled(event: CompileEvent): void {
    if (event.kind === 'started') {
      if (this.pages.children.length === 0) this.empty.textContent = 'Engraving…'
      return
    }
    const { outcome } = event
    const same = outcome.rootFile === this.rootFile
    const update = previewUpdate(outcome, same && this.pages.children.length > 0)
    if (update.show) this.render(outcome.rootFile, outcome.svg, same)
    else if (outcome.state !== 'no-root' && !same) this.clear()
    if (outcome.state === 'no-root') return
    this.showNote(update.note)
  }

  /**
   * The cells of a page on screen replaced (W8): the compile's outcome then
   * finds the page as it is, and leaves it. A page that cannot take them is
   * left to that outcome. The page's text is the new one at once, so that the
   * outcome, which follows the cells, leaves the page; the cells go in after
   * (D175): their markup parsed a part a task, then moved into the page in
   * one, then the links and the render's callback in another. A page
   * replaced meanwhile, by a render or a clear, stops them.
   */
  replaceCells({ page, cells, markup }: PageCells): void {
    const element = this.pages.children[page]
    const svg = element?.querySelector('svg')
    const shown = this.shown[page]
    const text = shown === undefined ? undefined : replaceCells(shown, cells, markup)
    if (!element || !svg || text === undefined) return
    this.shown[page] = text
    // Newer than the render still inserting: that render's page must not go over it.
    this.pending = this.pending.filter((index) => index !== page)
    const current = () => this.pages.children[page] === element
    const work = (this.cellWork.get(page) ?? Promise.resolve())
      .then(() => putCells(svg, cells, markup, current, () => this.nextTask(), () => this.nextFrame()))
      .then((put) => {
        if (this.cellWork.get(page) === work) this.cellWork.delete(page)
        // Stopped: the page is another's now.
        if (put === undefined) return
        // The page on screen was not the one the engine compared with: its text, whole (rare).
        if (!put && this.shown[page] === text) element.replaceWith(toPage(text, page))
        this.owe()
      })
    this.cellWork.set(page, work)
  }

  /**
   * A page on screen replaced whole before its compile's outcome (I7, D269):
   * the page the engine wrote first, which the outcome then finds as it is,
   * and leaves. One page parsed in the task, as a render inserts the one in
   * view (D58). A page not on screen yet is left to the outcome.
   */
  replacePage({ page, svg }: PageWhole): void {
    const element = this.pages.children[page]
    if (!element || this.shown[page] === undefined || this.shown[page] === svg) return
    this.shown[page] = svg
    // Newer than a render still inserting, or cells still going in: neither goes over it.
    this.pending = this.pending.filter((index) => index !== page)
    this.cellWork.delete(page)
    element.replaceWith(toPage(svg, page))
    this.owe()
  }

  private nextTask(): Promise<void> {
    return (this.options.nextTask ?? nextTask)()
  }

  private nextFrame(): Promise<void> {
    return (this.options.nextFrame ?? nextFrame)()
  }

  /** The links and the render's callback, in a task after the cells' (D175), once for those that come together. */
  private owe(): void {
    if (this.linksOwed) return
    this.linksOwed = true
    void this.nextTask().then(() => {
      this.linksOwed = false
      this.indexLinks()
      this.options.onRender?.()
    })
  }

  /**
   * Shows the pages of a compile (D58, D174). A page whose text is the one on
   * screen at its place stays, as replaceCells may have left it. Of the
   * others, the one at the middle of the view is parsed and inserted at once
   * and the rest one per frame, outwards from it (`insertionOrder`), so that
   * no task parses more than one page; until then the old page, or an empty
   * one of its size (`pageRatio`), holds the place. A newer render or a clear
   * stops the insertion.
   */
  private render(rootFile: string, svg: string[], same: boolean): void {
    const id = ++this.insertion
    const { body } = this.options
    // Another score starts at its top; the same one keeps its place.
    const rects = same ? this.pageRects() : []
    const anchor = captureAnchor(rects, body.scrollTop)
    const visible = captureAnchor(rects, body.scrollTop + body.clientHeight / 2)?.page ?? 0
    const pages = this.pages
    if (!same) pages.replaceChildren()
    while (pages.children.length > svg.length) pages.lastElementChild!.remove()
    this.shown.length = pages.children.length
    const changed: number[] = []
    svg.forEach((text, index) => {
      if (this.shown[index] === text) return
      changed.push(index)
      if (index >= pages.children.length) pages.append(placeholder(text, index))
    })
    this.pending = insertionOrder(changed, visible)
    const insert = () => {
      const index = this.pending.shift()!
      pages.children[index].replaceWith(toPage(svg[index], index))
      this.shown[index] = svg[index]
    }
    this.rootFile = rootFile
    if (this.pending.length > 0) insert()
    this.indexLinks()
    this.layout()
    body.scrollTop = resolveAnchor(anchor, this.pageRects())
    this.options.onRender?.()
    const next = () => {
      if (id !== this.insertion || this.pending.length === 0) return
      insert()
      this.indexLinks()
      this.options.onRender?.()
      if (this.pending.length > 0) requestAnimationFrame(next)
    }
    if (this.pending.length > 0) requestAnimationFrame(next)
  }

  /** `sourceLinks` of the pages on screen, no element clicked. */
  private indexLinks(): void {
    this.clicked?.classList.remove('current')
    this.clicked = undefined
    this.sourceLinks.clear()
    for (const link of this.pages.querySelectorAll('a')) {
      const href = linkHref(link)
      if (!isSourceLink(href)) continue
      link.classList.add('source')
      const links = this.sourceLinks.get(href)
      if (links) links.push(link)
      else this.sourceLinks.set(href, [link])
    }
  }

  private clear(): void {
    this.insertion++
    this.pending = []
    this.pages.replaceChildren()
    this.shown = []
    this.sourceLinks.clear()
    this.rootFile = undefined
    this.clicked = undefined
    this.options.onRender?.()
  }

  /** The page elements on screen, in order. */
  get pageElements(): HTMLCollection {
    return this.pages.children
  }

  private showNote(note: string | undefined): void {
    const hasPages = this.pages.children.length > 0
    this.empty.hidden = hasPages
    this.empty.textContent = hasPages ? '' : (note ?? 'Open a score to see it here.')
    this.note.hidden = !hasPages || !note
    this.note.textContent = note ?? ''
  }

  /** Page boxes in the body's scroll coordinates. */
  private pageRects(): { top: number; height: number }[] {
    const { body } = this.options
    const origin = body.getBoundingClientRect().top - body.scrollTop
    return Array.from(this.pages.children, (page) => {
      const rect = page.getBoundingClientRect()
      return { top: rect.top - origin, height: rect.height }
    })
  }

  private layout(): void {
    const available = Math.max(this.options.body.clientWidth - 2 * PADDING, 50)
    this.pages.style.setProperty('--page-width', `${Math.round(available * this.zoom)}px`)
    this.fitButton.textContent = zoomLabel(this.zoom)
  }

  /** Re-lays out while the point at `y` (from the body's top) stays where it is. */
  private relayout(y: number, change: () => void): void {
    const { body } = this.options
    const anchor = captureAnchor(this.pageRects(), body.scrollTop + y)
    const x = body.scrollWidth > 0 ? (body.scrollLeft + body.clientWidth / 2) / body.scrollWidth : 0.5
    change()
    this.layout()
    body.scrollTop = resolveAnchor(anchor, this.pageRects()) - y
    body.scrollLeft = x * body.scrollWidth - body.clientWidth / 2
    this.options.onLayout?.()
  }

  private setZoom(zoom: number, y = this.options.body.clientHeight / 2): void {
    this.relayout(y, () => (this.zoom = clampZoom(zoom)))
  }
}

/** Holds a page's place, at its size, until it is inserted (D58). */
function placeholder(text: string, index: number): HTMLElement {
  const page = document.createElement('div')
  page.className = 'preview-page'
  page.dataset.page = String(index + 1)
  const ratio = pageRatio(text)
  if (ratio) page.style.setProperty('aspect-ratio', String(ratio))
  return page
}

/** `href` or `xlink:href`, whichever the page uses. */
function linkHref(link: Element): string {
  return (link as SVGAElement).href?.baseVal ?? ''
}

function toPage(text: string, index: number): HTMLElement {
  const page = document.createElement('div')
  page.className = 'preview-page'
  page.dataset.page = String(index + 1)
  const svg = new DOMParser().parseFromString(quiet(text), 'image/svg+xml').documentElement
  if (svg.localName !== 'svg' || svg.querySelector('parsererror')) {
    page.classList.add('preview-page-error')
    page.textContent = `Page ${index + 1} could not be displayed.`
    return page
  }
  sanitize(svg)
  if (svg.hasAttribute('viewBox')) {
    // Sized by .preview-page; the viewBox keeps the aspect ratio.
    svg.removeAttribute('width')
    svg.removeAttribute('height')
  }
  page.append(document.importNode(svg, true))
  return page
}

/** A task of its own, after what is waiting: a MessageChannel's message, which no timer's clamp delays. */
function nextTask(): Promise<void> {
  return new Promise((resolve) => {
    const channel = new MessageChannel()
    channel.port1.onmessage = () => {
      channel.port1.close()
      resolve()
    }
    channel.port2.postMessage(undefined)
  })
}

/** The next frame, from its callbacks: what is done then is styled and laid out in it. */
function nextFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()))
}

/** The cells of a markup, in order, in parts of about `CELL_PART` characters (a longer cell is a part alone). */
function cellParts(markup: string): string[] {
  const CELL = '<g data-lily-cell="'
  const parts: string[] = []
  let start = markup.indexOf(CELL)
  while (start >= 0) {
    let end = markup.indexOf(CELL, start + 1)
    while (end >= 0 && end - start < CELL_PART) end = markup.indexOf(CELL, end + 1)
    parts.push(markup.slice(start, end < 0 ? markup.length : end))
    start = end
  }
  return parts
}

/**
 * As preview.js' replaceCellGroups, in tasks (D175) and frames (D176): the
 * markup's cells parsed and sanitized a part a task; then, a part a frame, the
 * new ones adopted into the page, not copied, hidden, each after the cell
 * before it, so that a frame styles and lays out a part and paints none of it;
 * then, in one more frame, the page's cells that leave removed, those that stay
 * moved where they must, and the new ones shown. Until then the page on screen
 * is the old one whole. True when they went in, false when the page lacks a
 * cell (nothing changed), undefined when `current` said the page had gone
 * before.
 */
async function putCells(
  svg: Element,
  cells: PageCells['cells'],
  markup: string,
  current: () => boolean,
  next: () => Promise<void>,
  frame: () => Promise<void>,
): Promise<boolean | undefined> {
  const fresh = new Map<string, Element>()
  const parts: Element[][] = []
  for (const part of cellParts(markup)) {
    await next()
    if (!current()) return undefined
    const parsed = new DOMParser().parseFromString(
      `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">${quiet(part)}</svg>`,
      'image/svg+xml',
    ).documentElement
    if (parsed.localName !== 'svg' || parsed.querySelector('parsererror')) return false
    sanitize(parsed)
    const groups = [...parsed.children]
    for (const group of groups) fresh.set(group.getAttribute('data-lily-cell') ?? '', group)
    parts.push(groups)
  }
  await (parts.length > 0 ? frame() : next())
  if (!current()) return undefined
  const shown = new Map<string, Element>()
  for (const group of svg.querySelectorAll('[data-lily-cell]')) shown.set(group.getAttribute('data-lily-cell') ?? '', group)
  const parent = shown.values().next().value?.parentNode
  if (!parent || !cells.every(({ digest }) => fresh.has(digest) || shown.has(digest))) return false
  const place = new Map(cells.map(({ digest }, i) => [digest, i]))
  const element = (digest: string) => fresh.get(digest) ?? shown.get(digest)
  for (const [k, groups] of parts.entries()) {
    if (k > 0) await frame()
    if (!current()) return undefined
    for (const group of groups) {
      const digest = group.getAttribute('data-lily-cell') ?? ''
      const i = place.get(digest)
      if (i === undefined) continue
      const adopted = svg.ownerDocument.adoptNode(group)
      fresh.set(digest, adopted)
      adopted.setAttribute('visibility', 'hidden')
      const before = i > 0 ? element(cells[i - 1].digest) : undefined
      parent.insertBefore(adopted, before?.parentNode === parent ? before.nextElementSibling : parent.firstElementChild)
    }
  }
  if (parts.length > 0) await frame()
  if (!current()) return undefined
  const wanted = new Set(cells.map(({ digest }) => digest))
  for (const [digest, group] of shown) if (!wanted.has(digest) || fresh.has(digest)) group.remove()
  let at = parent.firstElementChild
  for (const { digest, y } of cells) {
    const group = element(digest)!
    const place = `translate(0, ${y})`
    if (group.getAttribute('transform') !== place) group.setAttribute('transform', place)
    if (group === at) at = at.nextElementSibling
    else parent.insertBefore(group, at)
    if (group.hasAttribute('visibility')) group.removeAttribute('visibility')
  }
  return true
}
