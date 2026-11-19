import * as assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { describe, test } from 'node:test'
import { droppedGuitarPro, GUITAR_PRO_ACCEPT, GUITAR_PRO_EXTENSIONS, isGuitarPro, scoreName } from '../src/renderer/guitarPro'

// Runs under `node --test` from out/test/ (npm run test:unit in packages/studio/).
// Import Guitar Pro… (DECISIONS D166); its conversion is crates/session's guitar_pro.rs.

const studioDir = path.resolve(__dirname, '..', '..')
const read = (file: string) => readFileSync(path.join(studioDir, file), 'utf8')

describe('Import Guitar Pro', () => {
  test('takes Guitar Pro files by their extension, in any case', () => {
    for (const file of ['/a/song.gp3', '/a/song.GP4', '/a/song.gp5', '/a/Song.gpx', '/a/new.gp', '/a/old.gtp']) assert.ok(isGuitarPro(file), file)
    for (const file of ['/a/song.mid', '/a/song.ly', '/a/gp5', '/a/.gp5', '/a.gp5/song']) assert.ok(!isGuitarPro(file), file)
  })

  test('a drop takes the first Guitar Pro file', () => {
    assert.equal(droppedGuitarPro(['/a/take.wav', '/a/riff.gp5', '/a/b.gpx']), '/a/riff.gp5')
    assert.equal(droppedGuitarPro(['/a/take.wav']), undefined)
  })

  test('the score is named for the file', () => {
    assert.equal(scoreName('/Music/band-song.gp5'), 'band-song.ly')
    assert.equal(scoreName('Song v1.2.gpx'), 'Song v1.2.ly')
    assert.equal(GUITAR_PRO_ACCEPT, '.gp3,.gp4,.gp5,.gpx')
  })

  test('the extensions are the session crate’s', () => {
    const rust = read('crates/session/src/guitar_pro.rs').match(/pub const EXTENSIONS: &\[&str\] = &\[([^\]]*)\]/)?.[1] ?? ''
    assert.deepEqual([...rust.matchAll(/"(\w+)"/g)].map((match) => match[1]), GUITAR_PRO_EXTENSIONS)
  })
})

describe('the app menu (src-tauri/src/menu.rs)', () => {
  test('its renderer commands are the ones of Command in src/ipc.ts', () => {
    const menu = read('src-tauri/src/menu.rs').match(/pub const COMMANDS: &\[&str\] = &\[([^\]]*)\]/)?.[1] ?? ''
    const commands = [...menu.matchAll(/"([\w-]+)"/g)].map((match) => match[1])
    const union = read('src/ipc.ts').match(/export type Command =([^]*?)\n\n/)?.[1] ?? ''
    const declared = [...union.matchAll(/\|\s*'([\w-]+)'/g)].map((match) => match[1])
    assert.ok(commands.includes('import-guitar-pro'))
    // upload and download come only from the web's menu bar (D55).
    assert.deepEqual(commands.sort(), declared.filter((command) => command !== 'upload' && command !== 'download').sort())
  })
})
