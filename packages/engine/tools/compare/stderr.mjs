// The extension's parseStderr (packages/vscode/src/diagnostics/parse.ts, D6,
// D16) in plain JavaScript, for C0 (DECISIONS D63). It is the third copy, beside
// the extension's and Lily Studio's Rust port (D42): a change to the
// extension's parser must be made here too, and gen-stderr-fixture.mjs then
// rewrites the fixture that test/c0.test.mjs holds this copy to.
//
// Also where the engine's tools find the error corpus (chapter 02 §7): through
// @lily/common's exports, never by a relative path into that package.
import { readdirSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

/** The error corpus's directory, packages/common/test/parser/errors, resolved by its first case. */
export const ERRORS = path.dirname(fileURLToPath(import.meta.resolve('@lily/common/test/parser/errors/01-unknown-command.ly')))

/** The corpus's cases by name (`01-unknown-command`, …), in order; each has a .ly and its .err. */
export const errorCases = () => readdirSync(ERRORS).filter((f) => f.endsWith('.err')).map((f) => f.slice(0, -'.err'.length)).sort()

// @lily/common/span's displayWidth: a tab advances to the next multiple of 8.
const TAB_WIDTH = 8
export function displayWidth(text) {
  let width = 0
  for (const char of text) width = char === '\t' ? width + TAB_WIDTH - (width % TAB_WIDTH) : width + 1
  return width
}

const KEYWORDS = 'fatal error|programming error|error|warning'
// Tried first, so that a bare message quoting a location is not read as located.
const BARE = new RegExp(`^(${KEYWORDS}): (.*)$`)
// The lazy path stops at the first `:LINE[:COL]: keyword:`; the colon of a
// Windows drive letter is not followed by digits and is skipped.
const LOCATED = new RegExp(`^(.+?):(\\d+)(?::(\\d+))?: (${KEYWORDS}): (.*)$`)

/**
 * Parses the stderr of a run made with English messages (`LANGUAGE=en`, D15) into
 * `{file, line, column?, severity, message}`s, as the extension does. Order of first
 * appearance is kept; identical entries are reported once. `rootFile` receives the
 * messages that carry no location; `cwd`, the base of relative paths, defaults to its directory.
 */
export function parseStderr(stderr, { rootFile, cwd }) {
  rootFile = path.resolve(rootFile)
  cwd ??= path.dirname(rootFile)
  const lines = stderr.split(/\r?\n/)

  const diagnostics = []
  const seen = new Set()
  for (let index = 0; index < lines.length;) {
    const header = parseHeader(lines[index++])
    // Anything before the first message (Guile notes, backtraces) stays in the
    // raw output only.
    if (!header) continue

    const block = []
    while (index < lines.length && !parseHeader(lines[index])) block.push(lines[index++])

    // Text parsed by Scheme is located in `<included string>` or `<string>`,
    // which is no file: the message goes to the root and names where it was.
    const inString = header.file !== undefined && /^<.*>$/.test(header.file)
    const origin = `(in ${header.file}, line ${header.line}, column ${header.column})`
    const message = [header.message, ...continuation(block, header.column), inString ? origin : '']
      .map((line) => line.trim())
      // "continuing, cross fingers" follows every programming error.
      .filter((line) => line !== '' && line !== 'continuing, cross fingers')
      .join('\n')
    const located = header.file !== undefined && !inString
    const diagnostic = {
      file: located ? path.resolve(cwd, header.file) : rootFile,
      line: located ? Math.max(1, header.line ?? 1) : 1,
      ...(located && header.column !== undefined ? { column: Math.max(1, header.column) } : {}),
      severity: header.keyword === 'error' || header.keyword === 'fatal error' ? 'error' : 'warning',
      // A programming error is a lilypond bug the run survived; say so.
      message: header.keyword === 'programming error' ? `programming error: ${message}` : message,
    }

    const key = JSON.stringify(diagnostic)
    if (!seen.has(key)) {
      seen.add(key)
      diagnostics.push(diagnostic)
    }
  }

  // `fatal error: failed files: "…"` closes every failed run. Next to the errors
  // that caused it, it is noise; alone, it is the only sign of the failure.
  const isSummary = (d) => d.column === undefined && d.message.startsWith('failed files: ')
  const hasCause = diagnostics.some((d) => d.severity === 'error' && !isSummary(d))
  return hasCause ? diagnostics.filter((d) => !isSummary(d)) : diagnostics
}

function parseHeader(line) {
  const bare = BARE.exec(line)
  if (bare) return { keyword: bare[1], message: bare[2] }
  const located = LOCATED.exec(line)
  if (!located) return undefined
  return {
    file: located[1],
    line: Number(located[2]),
    column: located[3] === undefined ? undefined : Number(located[3]),
    keyword: located[4],
    message: located[5],
  }
}

/**
 * The lines of a message block that belong to the message. A message with a
 * column is followed by two context lines: the source line up to the column,
 * then the rest of it indented to the column. Message text can come before them
 * (`(search path: …)`) and after them (Guile's `In procedure car: …`).
 */
function continuation(block, column) {
  if (column === undefined) return [...block]
  const indent = column - 1
  for (let index = 0; index + 1 < block.length; index++) {
    const rest = block[index + 1]
    if (displayWidth(block[index]) === indent && rest.length >= indent && rest.slice(0, indent).trim() === '') {
      return [...block.slice(0, index), ...block.slice(index + 2)]
    }
  }
  return [...block]
}
