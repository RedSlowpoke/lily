// Writes fixtures/parse-stderr.json: what the extension's own parseStderr
// (packages/vscode/src/diagnostics/parse.ts) gives on each .err of the error
// corpus. test/c0.test.mjs holds stderr.mjs, the engine's copy, to it (D63).
// Run it again after a change to the extension's parser:
//
//   node --no-warnings tools/compare/gen-stderr-fixture.mjs   or `npm run gen:stderr-fixture`
//
// The extension's parser is TypeScript, loaded through the `lily` package by
// Node's type stripping (Node 24); the corpus through @lily/common's exports.
import { readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { ERRORS, errorCases } from './stderr.mjs'

export const FIXTURE = fileURLToPath(new URL('fixtures/parse-stderr.json', import.meta.url))

/** A case's diagnostics as `parse` gives them, its paths relative to the corpus's directory. */
export function diagnose(parse, name) {
  const stderr = readFileSync(path.join(ERRORS, `${name}.err`), 'utf8')
  return parse(stderr, { rootFile: path.join(ERRORS, `${name}.ly`), cwd: ERRORS })
    .map((d) => ({ ...d, file: path.relative(ERRORS, d.file) }))
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { parseStderr } = await import('lily/src/diagnostics/parse.ts')
  const cases = Object.fromEntries(errorCases().map((name) => [name, diagnose(parseStderr, name)]))
  writeFileSync(FIXTURE, JSON.stringify({ parser: 'packages/vscode/src/diagnostics/parse.ts', cases }, null, 2) + '\n')
  console.log(`${Object.keys(cases).length} cases → ${path.relative(process.cwd(), FIXTURE)}`)
}
