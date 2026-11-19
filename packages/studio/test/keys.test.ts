import * as assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { stepVolume, transportKey } from '../src/renderer/keys'

// Runs under `node --test` from out/test/ (npm run test:unit).

const key = (name: string, extra: Partial<KeyboardEvent> = {}) => ({ key: name, altKey: false, ctrlKey: false, metaKey: false, shiftKey: false, ...extra })

describe('the transport’s arrow keys (D154)', () => {
  test('← and → move the music 5 s, ↑ and ↓ its volume a tenth', () => {
    assert.deepEqual(transportKey(key('ArrowLeft')), { seek: -5 })
    assert.deepEqual(transportKey(key('ArrowRight')), { seek: 5 })
    assert.deepEqual(transportKey(key('ArrowUp')), { volume: 0.1 })
    assert.deepEqual(transportKey(key('ArrowDown')), { volume: -0.1 })
  })

  test('another key, or an arrow with a modifier, is not the transport’s', () => {
    assert.equal(transportKey(key(' ')), undefined)
    assert.equal(transportKey(key('ArrowLeft', { metaKey: true })), undefined)
    assert.equal(transportKey(key('ArrowUp', { shiftKey: true })), undefined)
    assert.equal(transportKey(key('ArrowRight', { altKey: true })), undefined)
  })

  test('the volume stays between 0 and 1, in tenths', () => {
    assert.equal(stepVolume(1, 0.1), 1)
    assert.equal(stepVolume(0.7, 0.1), 0.8)
    assert.equal(stepVolume(0.1, -0.1), 0)
    assert.equal(stepVolume(0, -0.1), 0)
    assert.equal(stepVolume(0.30000000000000004, 0.1), 0.4)
  })
})
