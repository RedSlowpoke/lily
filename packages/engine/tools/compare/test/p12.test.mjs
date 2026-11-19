// MS8 step 10 (P12 and OPEN F's R9 gaps): inputs of LilyPond syntax and Scheme that no regression score of the
// corpus has, each compiled natively through the core (crates/core/examples/compile.rs, one instance for them all)
// and by LilyPond 2.26.0, their stderr equal byte for byte, LilyPond's data directory written `<datadir>` as the
// engine names it. What a regression score has is the corpus's: \lyricsto Type = id (Y79), number lists (Y80),
// expect-error with ly:parser-clear-error (Y81), the tagged \change of \autoChange (Y82, Y83). Each score is
// performed too, as the engine performs it for the playback map (D103) and capture.ly has LilyPond do (PERFORM):
// a translation's warning is then given twice by both, as OPEN F's `skipping zero-duration score` was. Skipped, and
// so unverified, without LilyPond 2.26.0 or cargo.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { lilypondVersion } from '../capture.mjs'
import { stderrDiffering } from '../inputs.mjs'

const lilypond = lilypondVersion() === '2.26.0' ? false : 'LilyPond 2.26.0 is not installed'
const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'

/** Each input's name and its text after `\version "2.26.0"`. */
export const INPUTS = [
  // \sourcefilename and \sourcefileline: the parser's messages and the translation's after them.
  ['sourcefile', '\\sourcefilename "renamed.ly"\n\\sourcefileline 40\n{ c\'4 \\foo d\'2. | e\'1 c\'2 | }'],
  ['sourcefile-errors', '\\sourcefilename 5\n\\sourcefileline x\n{ c\'1 }'],
  ['maininput', '\\maininput\n{ c\'1 }'],
  // An assignment's value that ends in a `#` form is defined before the next token is lexed.
  ['assign-override', 'x = \\override Stem.color = #red\n{ \\x c\'4 }'],
  ['assign-set', 'x = \\set Staff.x = #1\n{ \\x c\'4 }'],
  ['assign-set-bool', 'x = \\set x = ##t\n{ \\x c\'4 }'],
  ['assign-number', 'x = \\override Stem.length = 5\n{ \\x c\'4 }'],
  ['assign-string', 'x = \\set x = "a"\n{ \\x c\'4 }'],
  ['music-property', '#(ly:warning "~a" (ly:music-property (make-music \'NoteEvent) \'articulations \'default))\n'
    + '#(ly:warning "~a" (ly:music-property (make-music \'NoteEvent \'articulations \'()) \'articulations \'default))\n'
    + '#(ly:warning "~a" (ly:music-property (make-music \'NoteEvent \'articulations \'()) \'articulations))\n{ c\'1 }'],
  ['syntax-constructors', '#(ly:warning "~a" (filter defined? \'(music-function partial-music-function '
    + 'music-function-call-error argument-error property-set property-override repeat composed-markup-list '
    + 'partial-markup partial-text-script create-script-function sequential-music)))\n{ c\'1 }'],
  ['clear-error', '#(ly:parser-error "boom")\n#(ly:parser-clear-error)\n{ c\'1 }'],
  ['expect-error-none', 'expect-error = ##t\n{ c\'1 }'],
  ['expect-error-some', 'expect-error = ##t\n{ c\'1 \\foo }'],
  // Templates cut short, \etc after a property path.
  ['template-direction', 'fn = ##{ ^\\etc #}\n{ c\'1 \\fn "x" }'],
  ['template-minus', 'fn = ##{ -\\tweak color #red \\etc #}\n{ c\'1 \\fn -. }'],
  ['etc-path', 'fn = \\override Staff.TimeSignature.color.foo.bar = \\etc\n{ \\fn #red c\'1 }'],
  ['etc-wrong-path', 'fn = \\override #(list 3 4) = \\etc\n{ \\fn #red c\'1 }'],
  // A clone given no location: a message at its music prints its excerpt.
  ['clone', 'n = #(ly:parse-string-expression (ly:parser-clone) "{ c4 d4 }")\n'
    + '#(ly:music-warning (car (ly:music-property n \'elements)) "hi")\n{ \\n }'],
  // A \score whose music is a template's error.
  ['score-template-error', '\\score { { ##{ \\foo #} } }'],
  ['score-template-empty', '\\score { ##{ #} }'],
  ['score-no-music', '\\score { ##{ \\foo #} }'],
  // embedded_lilypond_number, *#scheme multipliers, \markuplist identifiers.
  ['template-number', 'x = ##{ 5 #}\n#(ly:warning "~a" x)\ny = ##{ -2.5\\cm #}\n#(ly:warning "~a" y)\n{ c\'1 }'],
  ['multipliers', '{ c\'4*#(/ 1 2) d\'2*#3/4 e\'4*#2 f\'8 }'],
  ['markuplist-identifier', 'ml = \\markuplist { a b }\n\\markuplist \\ml\n\\markup \\line \\ml'],
]

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-p12-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

test("the engine's stderr of P12's and OPEN F's inputs equals LilyPond's", { skip: lilypond || cargo, timeout: 900_000 }, () => {
  const failed = stderrDiffering(INPUTS, scratch)
  assert.deepEqual(failed, [], failed.join('\n'))
})
