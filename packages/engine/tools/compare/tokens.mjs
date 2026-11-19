// Token sequences for MS2's parser steps (02 §5): the tokens Bison reads,
// taken from LilyPond's -ddebug-parser trace, and compared with those the
// engine's parse example records (crates/parser/examples/parse.rs --tokens).
//
// A sequence is one token a line, by its name in Bison's messages (yytnamerr:
// `\header`, `'{'`, `NOTENAME_PITCH`, `end of input`, `scheme?`). A token the
// parser pushed back and Bison reads again is marked `^`: the value token
// after `(reparsed?)` or `(backed-up?)` (MYREPARSE, MYBACKUP), and the
// lookahead pushed back with it. A parse inside the main file's (`#{ … #}`,
// ly:parse-string-expression) is indented two spaces a level, and ends where
// it is accepted or aborts. The trace
// starts at \maininput, so the sequence is the main file's and what it
// includes; it ends with the main file's parse, before init.ly's tail. A
// line `@@ begin <file>` or `@@ end <file>` of the trace is kept as it is
// (capture.mjs marks the init files with them; `byFile` splits there).
//
// The tokens the lexer gives are the sequence less the parser's pushes: the
// `^` tokens, `(reparsed?)` and `(backed-up?)` (`lexed`). The engine's parser
// reads its arguments without REPARSE and BACKUP tokens (cursor.rs), so its
// sequences are compared that way unless `full` is asked for.
//
//   node tools/compare/tokens.mjs [--full] <reference> <tokens>
//                                          compares two sequences; exits 1 if they differ
//   node tools/compare/tokens.mjs --trace <stderr>
//                                          prints the sequence of a -ddebug-parser trace
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const PUSHES = new Set(['(reparsed?)', '(backed-up?)'])

/** A token's name in Bison's messages, from the name its trace prints (yytnamerr). */
export function tokenName(raw) {
  if (!/^".*"$/s.test(raw)) return raw
  let name = ''
  for (let i = 1; i < raw.length - 1; i++) {
    const c = raw[i]
    if (c === "'" || c === ',') return raw
    if (c === '\\') {
      if (raw[i + 1] !== '\\') return raw
      i++
    }
    name += c
  }
  return name
}

const MARKS = [
  ['start', /Starting parse$/],
  ['read', /Reading a token$/],
  ['token', /Next token is token (.*) \([^()]*\)$/],
  ['eof', /Now at end of input\.$/],
  ['shift', /Shifting token (.*) \([^()]*\)$/],
  ['drop', /Error: discarding token (.*) \([^()]*\)$/],
  // A parse that aborts discards its lookahead: it ends there, a template's
  // as the main one's (MS2 step 33).
  ['abort', /Cleanup: discarding lookahead token (.*) \([^()]*\)$/],
  ['end', /Cleanup: popping nterm start_symbol /],
]

/** The lines of a -ddebug-parser trace that are Bison's (LilyPond's messages may share a line's start). */
export const BISON_LINE = /(?:^|\.\.\.)(?:Starting parse|Entering state|Stack now|Reading a token|Next token is|Now at end of input|Shifting token|Reducing stack|   \$\d+ = |-> \$\$ = |Error: |Cleanup: |Return for a new token)/

/** The tokens Bison read in the main file's parse of the -ddebug-parser trace `trace`. */
export function traceTokens(trace) {
  const tokens = []
  const state = () => ({ reading: false, pending: null, pushedBack: false, owed: 0 })
  // The main parse is traced from \maininput on: its first token was being read.
  const stack = [{ ...state(), reading: true }]
  const record = (s, name) => {
    let line = name
    if (PUSHES.has(name)) {
      s.owed = 1 + (s.pushedBack ? 1 : 0)
    } else if (s.owed > 0) {
      line = `^${name}`
      s.owed--
    }
    s.pushedBack = false
    s.reading = false
    s.pending = name
    tokens.push('  '.repeat(stack.length - 1) + line)
  }
  for (const text of trace.split('\n')) {
    if (/^@@ (begin|end) /.test(text)) {
      tokens.push(text)
      continue
    }
    const found = MARKS.map(([kind, re]) => [kind, text.match(re)]).find(([, m]) => m)
    if (!found) continue
    const [kind, m] = found
    const s = stack[stack.length - 1]
    switch (kind) {
      case 'start': stack.push(state()); break
      case 'read':
        // A lookahead not shifted was pushed back by the action before.
        if (s.pending !== null) s.pushedBack = true
        s.pending = null
        s.reading = true
        break
      case 'token': if (s.reading) record(s, tokenName(m[1])); break
      case 'eof': if (s.reading) record(s, 'end of input'); break
      case 'shift': if (tokenName(m[1]) === s.pending) s.pending = null; break
      case 'drop': s.pending = null; break
      case 'abort':
      case 'end':
        if (stack.length === 1) return tokens
        stack.pop()
        break
    }
  }
  return tokens
}

/**
 * The tokens of each file marked in `tokens` (`@@ begin <file>` to `@@ end
 * <file>`), the files it includes with them: a Map from file to tokens.
 */
export function byFile(tokens) {
  const files = new Map()
  const open = []
  for (const line of tokens) {
    const mark = line.match(/^@@ (begin|end) (.+)$/)
    if (!mark) {
      for (const file of open) files.get(file).push(line)
    } else if (mark[1] === 'begin') {
      open.push(mark[2])
      files.set(mark[2], [])
    } else {
      open.splice(open.lastIndexOf(mark[2]), 1)
    }
  }
  return files
}

/** The tokens of `tokens` that the lexer gave: the parser's pushes left out. */
export function lexed(tokens) {
  return tokens.filter((t) => !PUSHES.has(t.trim()) && !t.trim().startsWith('^'))
}

/** A sequence's lines, from a file's text. */
export const readTokens = (text) => text.split('\n').filter((line) => line !== '')

/**
 * Compares the sequence `got` with `reference`, both arrays of lines, as the
 * lexer gave them unless `full`: whether they are equal, their lengths, and
 * where they first differ, with the tokens before it.
 */
export function compareTokens(reference, got, { full = false } = {}) {
  const [a, b] = full ? [reference, got] : [lexed(reference), lexed(got)]
  let at = 0
  while (at < a.length && at < b.length && a[at] === b[at]) at++
  const equal = at === a.length && at === b.length
  return {
    equal, lengths: [a.length, b.length],
    ...(equal ? {} : { at, expected: a[at] ?? null, got: b[at] ?? null, before: a.slice(Math.max(0, at - 5), at) }),
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2)
  if (argv[0] === '--trace' && argv.length === 2) {
    for (const line of traceTokens(readFileSync(argv[1], 'utf8'))) console.log(line)
  } else {
    const full = argv[0] === '--full'
    const files = full ? argv.slice(1) : argv
    if (files.length !== 2) {
      console.error('usage: tokens.mjs [--full] <reference> <tokens> | --trace <stderr>')
      process.exit(2)
    }
    const [a, b] = files.map((f) => readTokens(readFileSync(f, 'utf8')))
    const result = compareTokens(a, b, { full })
    console.log(JSON.stringify(result))
    process.exit(result.equal ? 0 : 1)
  }
}
