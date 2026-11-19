import * as assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import * as os from 'node:os'
import * as path from 'node:path'
import { after, before, beforeEach, describe, test } from 'node:test'
import { quiet, replaceCellGroups, replaceCells } from '@lily/common/web/preview.js'
import { charToCharacter } from '@lily/common/span'
import { parseTextEdit } from '@lily/common/textedit'
import type { CompileEvent, CompileOutcome } from '../src/ipc'
import { previewUpdate, ScorePreview } from '../src/renderer/preview'
import { realOutcome } from './outcome'

// Runs under `node --test` from out/test/ (npm run test:unit in packages/studio/).

let scratch: string

before(async () => {
  scratch = await fs.realpath(await fs.mkdtemp(path.join(os.tmpdir(), 'lily-studio-preview-')))
})

after(async () => {
  if (scratch) await fs.rm(scratch, { recursive: true, force: true })
})

function outcome(partial: Partial<CompileOutcome>): CompileOutcome {
  return { state: 'ok', rootFile: '/s/score.ly', diagnostics: [], errorCount: 0, warningCount: 0, pages: [], svg: [], midi: [], durationMs: 1, ...partial }
}

describe('previewUpdate', () => {
  const page = ['<svg/>']

  test('the pages of a run are shown, with a note when it had errors', () => {
    assert.deepEqual(previewUpdate(outcome({ svg: page }), false), { show: true })
    assert.deepEqual(previewUpdate(outcome({ state: 'failed', svg: page }), true), {
      show: true,
      note: 'The score has errors, so this may be incomplete.',
    })
  })

  test('a run without pages keeps what is on screen and says why', () => {
    assert.deepEqual(previewUpdate(outcome({ state: 'failed' }), true), {
      show: false,
      note: 'The score has errors. Showing the last version that engraved.',
    })
    assert.deepEqual(previewUpdate(outcome({ state: 'failed' }), false), {
      show: false,
      note: 'The score has errors. Fix them to see it here.',
    })
    assert.deepEqual(previewUpdate(outcome({}), false), { show: false, note: 'LilyPond produced no pages.' })
    assert.deepEqual(previewUpdate(outcome({ state: 'no-lilypond' }), true), { show: false, note: undefined })
    assert.deepEqual(previewUpdate(outcome({ state: 'no-root' }), true), { show: false })
  })

  test("preview.js's quiet() drops LilyPond's inline styles before parsing", () => {
    const svg = '<svg><style>a{}</style><a style="color:inherit;" xlink:href="textedit:///a.ly:1:2:2"><path d="M0"/></a></svg>'
    assert.equal(quiet(svg), '<svg><a xlink:href="textedit:///a.ly:1:2:2"><path d="M0"/></a></svg>')
  })
})

// Reading the pages, and checking a clicked file against the open folder, are
// the Rust side's (crates/engrave's tests).
/** What of an element the preview touches: enough of the DOM for its cells, without a browser. */
class Fake {
  readonly children: Fake[] = []
  readonly attributes: { localName: string; value: string }[] = []
  readonly classList = { add() {}, remove() {} }
  readonly dataset: Record<string, string> = {}
  readonly properties: Record<string, string> = {}
  readonly style = { setProperty: (name: string, value: string) => void (this.properties[name] = value) }
  readonly ownerDocument = { importNode: (node: Fake) => node, adoptNode: (node: Fake) => node }
  parentNode: Fake | undefined
  hidden = false
  className = ''
  textContent = ''
  scrollTop = 0
  scrollLeft = 0
  clientHeight = 0
  clientWidth = 0
  constructor(readonly localName: string) {}
  getAttribute = (name: string) => this.attributes.find((a) => a.localName === name)?.value ?? null
  hasAttribute = (name: string) => this.getAttribute(name) !== null
  setAttribute(localName: string, value: string): void {
    const found = this.attributes.find((a) => a.localName === localName)
    if (found) found.value = value
    else this.attributes.push({ localName, value })
  }
  removeAttribute(name: string): void {
    this.attributes.splice(this.attributes.findIndex((a) => a.localName === name), 1)
  }
  removeAttributeNode(attribute: { localName: string }): void {
    this.removeAttribute(attribute.localName)
  }
  get firstElementChild() {
    return this.children[0] ?? null
  }
  get nextElementSibling() {
    const siblings = this.parentNode?.children ?? []
    return siblings[siblings.indexOf(this) + 1] ?? null
  }
  get lastElementChild(): Fake | null {
    return this.children.at(-1) ?? null
  }
  get parentElement() {
    return this.parentNode
  }
  replaceWith(node: Fake): void {
    const parent = this.parentNode!
    const next = this.nextElementSibling
    this.remove()
    parent.insertBefore(node, next)
  }
  insertBefore(node: Fake, next: Fake | null): void {
    node.remove()
    node.parentNode = this
    this.children.splice(next ? this.children.indexOf(next) : this.children.length, 0, node)
  }
  append(...nodes: Fake[]): void {
    for (const node of nodes) this.insertBefore(node, null)
  }
  replaceChildren(...nodes: Fake[]): void {
    for (const child of [...this.children]) child.remove()
    this.append(...nodes)
  }
  remove(): void {
    const siblings = this.parentNode?.children
    siblings?.splice(siblings.indexOf(this), 1)
    this.parentNode = undefined
  }
  get all(): Fake[] {
    return this.children.flatMap((child) => [child, ...child.all])
  }
  querySelectorAll(selector: string): Fake[] {
    return this.all.filter((node) => (selector === '[data-lily-cell]' ? node.hasAttribute('data-lily-cell') : node.localName === selector))
  }
  querySelector(selector: string): Fake | null {
    return this.querySelectorAll(selector)[0] ?? null
  }
  addEventListener(): void {}
  /** A page is 100 high, one under the other, in a body scrolled by `Fake.scrolled`. */
  getBoundingClientRect = () => {
    const index = this.className === 'preview-page' ? (this.parentNode?.children.indexOf(this) ?? -1) : -1
    return index < 0 ? { top: 0, height: 0 } : { top: index * 100 - Fake.scrolled, height: 100 }
  }
  static scrolled = 0
}

/** A page's or a markup's text as elements: an `<svg>` around a `<g>`, each cell a `<g>` with an `<a>` when it holds one. */
function fakePage(text: string): Fake {
  const svg = new Fake('svg')
  const group = new Fake('g')
  svg.append(group)
  for (const [, digest, y, body] of text.matchAll(/<g data-lily-cell="(\w+)" transform="translate\(0, ([^)]+)\)">\n([\s\S]*?)<\/g>\n/g)) {
    const cell = new Fake('g')
    cell.setAttribute('data-lily-cell', digest)
    cell.setAttribute('transform', `translate(0, ${y})`)
    if (body.includes('<a')) cell.append(new Fake('a'))
    group.append(cell)
  }
  return svg
}

describe('a page’s cells replaced (W8)', () => {
  const cell = (digest: string, y: string, body: string) => `<g data-lily-cell="${digest}" transform="translate(0, ${y})">\n${body}\n</g>\n`
  const page = (...cells: string[]) => `<svg>\n<style>s</style>\n<g fill="currentColor">\n${cells.join('')}</g>\n</svg>\n`
  const a = cell('aa', '1', '<rect/>')
  const b = cell('bb', '2', '<path/>')
  const c = cell('cc', '3', '<a/>')

  test('in the text: the cell given, the others kept and moved, the bytes the page’s', () => {
    assert.equal(replaceCells(page(a, b), [{ digest: 'aa', y: '1' }, { digest: 'cc', y: '3' }], c), page(a, c))
    assert.equal(
      replaceCells(page(a, b), [{ digest: 'bb', y: '7' }, { digest: 'aa', y: '1' }], ''),
      page(cell('bb', '7', '<path/>'), a),
    )
  })

  test('in the text: a cell the page lacks, or a page without cells, cannot be replaced', () => {
    assert.equal(replaceCells(page(a), [{ digest: 'zz', y: '0' }], c), undefined)
    assert.equal(replaceCells('<svg/>', [{ digest: 'aa', y: '1' }], a), undefined)
  })

  describe('on screen', () => {
    const globals = globalThis as unknown as Record<'DOMParser' | 'document' | 'ResizeObserver' | 'requestAnimationFrame', unknown>
    /** Each text parsed, as `DOMParser` is what costs (06 §6.4). */
    let parsed: string[] = []
    const { DOMParser, document, ResizeObserver, requestAnimationFrame } = globals
    /** The callbacks of the next frame. */
    let frame: (() => void)[] = []
    /** Runs the frames until none is asked for; how many ran. */
    const frames = () => {
      let count = 0
      while (frame.length > 0) {
        const callbacks = frame
        frame = []
        for (const callback of callbacks) callback()
        count++
      }
      return count
    }
    before(() => {
      globals.requestAnimationFrame = (callback: () => void) => frame.push(callback)
      globals.DOMParser = class {
        parseFromString(text: string) {
          parsed.push(text)
          const svg = fakePage(text)
          // A markup's cells are the `<svg>`'s own.
          if (!text.includes('fill=')) svg.replaceChildren(...svg.children[0].children)
          return { documentElement: svg }
        }
      }
      globals.document = { createElement: (name: string) => new Fake(name), importNode: (node: Fake) => node, adoptNode: (node: Fake) => node }
      globals.ResizeObserver = class {
        observe() {}
      }
    })
    after(() => Object.assign(globals, { DOMParser, document, ResizeObserver, requestAnimationFrame }))
    /** The tasks the preview has asked to go on in (D175). */
    let task: (() => void)[] = []
    /** Runs the tasks until none is asked for; how many ran. */
    const tasks = async () => {
      let count = 0
      for (;;) {
        // What a resolved task's continuation asks for comes after its microtasks.
        await new Promise((resolve) => setImmediate(resolve))
        const next = task.shift()
        if (!next) return count
        next()
        count++
      }
    }
    beforeEach(() => {
      parsed = []
      frame = []
      task = []
      Fake.scrolled = 0
    })

    const digests = (svg: Fake) => svg.querySelectorAll('[data-lily-cell]').map((g) => [g.getAttribute('data-lily-cell'), g.getAttribute('transform')])

    test('the cells kept stay the same elements, moved; the others leave', () => {
      const svg = fakePage(page(a, b))
      const kept = svg.querySelectorAll('[data-lily-cell]')[0]
      const cells = [{ digest: 'cc', y: '3' }, { digest: 'aa', y: '9' }]
      assert.equal(replaceCellGroups(svg as unknown as Element, cells, c), true)
      assert.deepEqual(digests(svg), [['cc', 'translate(0, 3)'], ['aa', 'translate(0, 9)']])
      assert.equal(svg.querySelectorAll('[data-lily-cell]')[1], kept)
    })

    test('a cell the page lacks changes nothing', () => {
      const svg = fakePage(page(a, b))
      assert.equal(replaceCellGroups(svg as unknown as Element, [{ digest: 'zz', y: '0' }], ''), false)
      assert.deepEqual(digests(svg).map(([digest]) => digest), ['aa', 'bb'])
    })

    function studio() {
      const body = new Fake('div')
      new Fake('div').append(body)
      let renders = 0
      const preview = new ScorePreview({
        body: body as unknown as HTMLElement,
        actions: new Fake('div') as unknown as HTMLElement,
        onReveal: () => {},
        onRender: () => renders++,
        nextTask: () => new Promise<void>((resolve) => task.push(resolve)),
        // A frame is a task here, marked so that a test can tell them apart (D176).
        nextFrame: () => new Promise<void>((resolve) => task.push(Object.assign(() => resolve(), { frame: true }))),
      })
      const pages = body.children[1]
      const finished = (svg: string[]) => {
        preview.compiled({ kind: 'finished', outcome: outcome({ svg }) } as CompileEvent)
        // The pages one per frame (D58, D174), in the tests that do not count them.
        return frames()
      }
      const started = (svg: string[]) => preview.compiled({ kind: 'finished', outcome: outcome({ svg }) } as CompileEvent)
      return { preview, body, pages, finished, started, renders: () => renders }
    }

    test('the studio’s preview: the compile that holds a page replaced parses nothing again', async () => {
      const { preview, pages, finished } = studio()
      finished([page(a, b)])
      const shown = pages.children[0]
      parsed = []
      preview.replaceCells({ type: 'cells', revision: 2, page: 0, cells: [{ digest: 'aa', y: '1' }, { digest: 'cc', y: '3' }], markup: c })
      assert.equal(parsed.length, 0, 'nothing parsed in the cells’ message (D175)')
      // The outcome follows the cells at once: it finds the page's text the cells make.
      finished([page(a, c)])
      assert.equal(parsed.length, 0, 'the page parsed whole')
      await tasks()
      assert.equal(parsed.length, 1)
      assert.equal(shown.querySelectorAll('a').length, 1, 'the new cell')
      assert.equal(pages.children[0], shown)
    })

    test('D175: a page’s new cells are parsed a part a task, then go in by frames, then its links', async () => {
      const { preview, pages, finished, renders } = studio()
      finished([page(a, b)])
      const svg = pages.children[0].querySelector('svg')!
      const long = (digest: string) => cell(digest, '0', `<a/>${'<path d="M0 0"/>'.repeat(2_200)}`)
      parsed = []
      const before = renders()
      preview.replaceCells({ type: 'cells', revision: 2, page: 0, cells: ['aa', 'x1', 'x2', 'x3'].map((digest, y) => ({ digest, y: String(y) })), markup: long('x1') + long('x2') + long('x3') })
      assert.equal(await tasks(), 3 + 3 + 1 + 1, 'three parts parsed, three frames of them in hidden, one shows them, then the links')
      assert.equal(parsed.length, 3, 'one part a task: each cell is over CELL_PART')
      assert.deepEqual(digests(svg).map(([digest]) => digest), ['aa', 'x1', 'x2', 'x3'])
      assert.equal(svg.querySelectorAll('[data-lily-cell]').filter((g) => g.hasAttribute('visibility')).length, 0, 'every cell shown')
      assert.equal(renders() - before, 1, 'the render’s callback once, after the cells')
    })

    test('D176: the new cells go in hidden, a part a frame, and the page shows the old ones until one frame shows them', async () => {
      const { preview, pages, finished } = studio()
      finished([page(a, b, c)])
      const svg = pages.children[0].querySelector('svg')!
      const [kept, left] = svg.querySelectorAll('[data-lily-cell]')
      const long = (digest: string) => cell(digest, '0', `<a/>${'<path d="M0 0"/>'.repeat(2_200)}`)
      preview.replaceCells({ type: 'cells', revision: 2, page: 0, cells: ['x1', 'aa', 'x2', 'cc'].map((digest, y) => ({ digest, y: String(y) })), markup: long('x1') + long('x2') })
      /** The cells on screen and those in hidden, in the page's order. */
      const seen = () => {
        const groups = svg.querySelectorAll('[data-lily-cell]')
        const name = (g: Fake) => g.getAttribute('data-lily-cell') ?? ''
        return [groups.filter((g) => !g.hasAttribute('visibility')).map(name), groups.filter((g) => g.hasAttribute('visibility')).map(name)]
      }
      const frames: string[][][] = []
      for (;;) {
        await new Promise((resolve) => setImmediate(resolve))
        const next = task.shift() as ((() => void) & { frame?: boolean }) | undefined
        if (!next) break
        next()
        await new Promise((resolve) => setImmediate(resolve))
        if (next.frame) frames.push(seen())
      }
      assert.deepEqual(frames, [
        [['aa', 'bb', 'cc'], ['x1']],
        [['aa', 'bb', 'cc'], ['x1', 'x2']],
        [['x1', 'aa', 'x2', 'cc'], []],
      ])
      assert.deepEqual(digests(svg), [['x1', 'translate(0, 0)'], ['aa', 'translate(0, 1)'], ['x2', 'translate(0, 2)'], ['cc', 'translate(0, 3)']])
      assert.equal(svg.querySelectorAll('[data-lily-cell]')[1], kept, 'a cell kept stays the same element')
      assert.equal(left.parentNode, undefined, 'the cell that left is out of the page')
    })

    test('D175: a page replaced while its cells wait stops them', async () => {
      const { preview, pages, finished } = studio()
      finished([page(a, b)])
      preview.replaceCells({ type: 'cells', revision: 2, page: 0, cells: [{ digest: 'aa', y: '1' }, { digest: 'cc', y: '3' }], markup: c })
      // A newer compile's page, whole, before the cells' first task.
      finished([page(b)])
      const now = pages.children[0]
      parsed = []
      await tasks()
      assert.equal(parsed.length, 0, 'the cells never parsed')
      assert.equal(pages.children[0], now)
      assert.deepEqual(digests(now.querySelector('svg')!).map(([digest]) => digest), ['bb'])
    })

    test('D175: a newer compile’s cells wait for an older’s, and go on from them', async () => {
      const { preview, pages, finished } = studio()
      finished([page(a, b)])
      const svg = pages.children[0].querySelector('svg')!
      preview.replaceCells({ type: 'cells', revision: 2, page: 0, cells: [{ digest: 'aa', y: '1' }, { digest: 'cc', y: '3' }], markup: c })
      preview.replaceCells({ type: 'cells', revision: 3, page: 0, cells: [{ digest: 'cc', y: '2' }], markup: '' })
      finished([page(cell('cc', '2', '<a/>'))])
      await tasks()
      assert.deepEqual(digests(svg), [['cc', 'translate(0, 2)']])
    })

    test('I7 (D269): a page handed over whole before its compile ends is the one its outcome leaves', () => {
      const { preview, pages, finished } = studio()
      finished([page(a), page(b)])
      const [first] = pages.children
      parsed = []
      preview.replacePage({ type: 'page', revision: 2, page: 1, svg: page(c) })
      assert.equal(parsed.length, 1, 'the page parsed in its message')
      const handed = pages.children[1]
      assert.equal(handed.querySelector('[data-lily-cell]')?.getAttribute('data-lily-cell'), 'cc')
      finished([page(a), page(c)])
      assert.equal(parsed.length, 1, 'the outcome parses nothing again')
      assert.deepEqual(pages.children, [first, handed])
      // A page not on screen yet is the outcome's to insert.
      preview.replacePage({ type: 'page', revision: 3, page: 2, svg: page(b) })
      assert.equal(pages.children.length, 2)
    })

    test('the studio’s preview: a page cells cannot replace is left to its compile', () => {
      const { preview, pages, finished } = studio()
      finished([page(a)])
      const shown = pages.children[0]
      preview.replaceCells({ type: 'cells', revision: 2, page: 0, cells: [{ digest: 'zz', y: '0' }], markup: '' })
      assert.equal(pages.children[0], shown)
      finished([page(a, b)])
      assert.notEqual(pages.children[0], shown)
    })

    test('the studio’s preview: a page unchanged by a compile stays', () => {
      const { pages, finished } = studio()
      finished([page(a), page(b)])
      const [first, second] = pages.children
      finished([page(a), page(c)])
      assert.equal(pages.children[0], first)
      assert.notEqual(pages.children[1], second)
    })

    /** Four pages, each its own text: page `i` of revision `r`. */
    const four = (r: string) => [0, 1, 2, 3].map((i) => page(cell(`${r}${i}`, String(i), '<a/>')))
    /** The revision of each page element on screen, '' for a placeholder. */
    const onScreen = (pages: Fake) => pages.children.map((p) => (p.querySelector('[data-lily-cell]')?.getAttribute('data-lily-cell') ?? '').slice(0, 1))
    /** The pages parsed, in order, as `revision` + index. */
    const parsedPages = () => parsed.map((text) => /data-lily-cell="(\w+)"/.exec(text)?.[1])

    test('D58 in the studio: the page in view at once, the others one per frame outwards, the one below first', () => {
      const { body, pages, finished, started, renders } = studio()
      assert.equal(finished(four('a')), 3, 'a new score: its first page at once, then one per frame')
      assert.deepEqual(parsedPages(), ['a0', 'a1', 'a2', 'a3'])
      // Scrolled so that the view's middle is on page 3 (index 2).
      body.clientHeight = 100
      body.scrollTop = Fake.scrolled = 200
      parsed = []
      const before = renders()
      started(four('b'))
      assert.deepEqual(parsedPages(), ['b2'], 'only the page in view in the outcome’s task')
      assert.deepEqual(onScreen(pages), ['a', 'a', 'b', 'a'], 'the old pages hold the others’ places')
      assert.equal(frames(), 3)
      assert.deepEqual(parsedPages(), ['b2', 'b3', 'b1', 'b0'])
      assert.deepEqual(onScreen(pages), ['b', 'b', 'b', 'b'])
      assert.equal(renders() - before, 4, 'each page that goes in is a render, for the playhead (D35)')
    })

    test('D58 in the studio: an added page is held by an empty one of its size until its turn', () => {
      const { pages, finished, started } = studio()
      const added = four('a').map((text) => text.replace('<svg>', '<svg viewBox="0 0 200 100">'))
      finished(added.slice(0, 1))
      started(added)
      assert.deepEqual(onScreen(pages), ['a', 'a', '', ''], 'the page in view, then the one below it at once')
      assert.equal(pages.children[2].properties['aspect-ratio'], '2')
      assert.equal(pages.children[2].className, 'preview-page')
      frames()
      assert.deepEqual(onScreen(pages), ['a', 'a', 'a', 'a'])
      started(added.slice(0, 2))
      assert.equal(pages.children.length, 2, 'fewer pages: the rest leave at once')
    })

    test('D58 in the studio: a page given a newer compile’s cells while an older one inserts is not put over', async () => {
      const { preview, pages, finished, started } = studio()
      finished(four('a'))
      started(four('b'))
      // b0 is in; a3, still on screen, takes revision c's cells before b3's turn.
      preview.replaceCells({ type: 'cells', revision: 3, page: 3, cells: [{ digest: 'a3', y: '3' }, { digest: 'cc', y: '9' }], markup: c })
      const kept = pages.children[3]
      parsed = []
      frames()
      await tasks()
      parsed = parsed.filter((text) => !text.includes('data-lily-cell="cc"'))
      assert.deepEqual(parsedPages(), ['b1', 'b2'], 'b3 not parsed')
      assert.equal(pages.children[3], kept)
      assert.equal(kept.querySelectorAll('a').length, 2, 'a3’s link and the new cell’s')
    })

    test('D58 in the studio: a newer compile stops an older one’s insertion; it reuses only what is on screen', () => {
      const { preview, pages, finished, started } = studio()
      finished(four('a'))
      parsed = []
      started(four('b'))
      // b0 is in; b1–b3 wait, held by a1–a3, when c arrives with a1 again.
      const c = four('c')
      c[1] = four('a')[1]
      started(c)
      frames()
      assert.deepEqual(parsedPages(), ['b0', 'c0', 'c2', 'c3'], 'b’s other pages never parsed, a1 kept as it is on screen')
      assert.deepEqual(onScreen(pages), ['c', 'a', 'c', 'c'])
      // A placeholder takes no cells; a cleared preview inserts nothing more.
      finished(four('d').slice(0, 1))
      started(four('d'))
      preview.replaceCells({ type: 'cells', revision: 9, page: 2, cells: [{ digest: 'zz', y: '0' }], markup: '' })
      const count = parsed.length
      preview.showScore('/s/other.ly')
      frames()
      assert.equal(parsed.length, count)
      assert.equal(pages.children.length, 0)
    })
  })
})

describe('click-to-source with lilypond', () => {
  test("a note's link leads to its place in the included file", async (t) => {
    const include = path.join(scratch, 'parts', 'tune.ily')
    await fs.mkdir(path.dirname(include), { recursive: true })
    // A tab and an astral character before the note: CHAR counts code points.
    await fs.writeFile(include, 'tune = {\n\t%{𝄞%} fis\'4 g\n}\n')
    const score = path.join(scratch, 'song.ly')
    await fs.writeFile(score, '\\version "2.24.0"\n\\include "parts/tune.ily"\n{ \\tune }\n')

    const result = await realOutcome(t, score)
    if (!result) return
    assert.equal(result.state, 'ok', result.message)
    const hrefs = [...result.svg.join('\n').matchAll(/href="(textedit:[^"]*)"/g)].map(([, href]) => href)
    assert.ok(hrefs.length > 0, 'the pages carry point-and-click links')

    // As the editor takes a click: the link's CHAR is a place in the line.
    const locations = hrefs.map((href) => parseTextEdit(href)!)
    const line = '\t%{𝄞%} fis\'4 g'
    const fis = locations.find((l) => l.file === include && l.line === 2 && line.slice(charToCharacter(line, l.char)).startsWith('fis'))
    assert.ok(fis, `a link to fis in ${JSON.stringify(locations)}`)
  })
})
