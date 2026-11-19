// midi.html's player, for M8 on the phones (07 §5's unconfirmed points):
// T24's or T29's .midi as LilyPond wrote it ($LILY_CORPUS/ref, served as
// /ref/<id>.midi) played through packages/common/web/midi.js's Player, as the
// extension's preview plays it. ▶ calls play() in the click, as the preview's
// button does (D24).
//
//   /midi.html?score=T29           the score (T24 is the default)
//   /midi.html?session=off         without audioSession playback
//
// Player.play() sets `navigator.audioSession.type` to `playback` (07 §5.3).
// With the box unticked the page hides `navigator.audioSession` from midi.js,
// so the session stays the browser's default: on an iPhone on silent, that is
// heard as nothing. The box reloads the page, so each way starts from a fresh
// session and AudioContext.
//
// The readout shows the context's baseLatency and outputLatency (D51, 07 §5.5),
// its state and the session's type, and Chrome's playoutStats where they
// exist. The log keeps, with the time since the page loaded, every change of
// the player's and the context's state, the page being hidden or shown, and a
// gap of over a second between the player's updates while it plays: its
// setInterval did not fire (the screen locked, the page in the background).

;(() => {
  const { parseMidi, formatTime, Player } = globalThis.LilyMidi
  const params = new URLSearchParams(location.search)
  const id = params.get('score') === 'T29' ? 'T29' : 'T24'
  const withSession = params.get('session') !== 'off'
  const session = navigator.audioSession
  if (!withSession) Object.defineProperty(navigator, 'audioSession', { value: undefined, configurable: true })

  const $ = (name) => document.getElementById(name)
  const [play, stop, time, readout, log] = ['play', 'stop', 'time', 'readout', 'log'].map($)
  const lines = []
  const say = (line) => {
    lines.push(`${(performance.now() / 1000).toFixed(1).padStart(6)} s  ${line}`)
    log.textContent = lines.slice(-14).join('\n')
  }

  let lastState
  let lastContext
  let lastChange = 0
  const player = new Player({ onChange: changed })

  function changed() {
    const now = performance.now()
    if (player.state === 'playing' && lastState === 'playing' && now - lastChange > 1000) {
      say(`no update for ${((now - lastChange) / 1000).toFixed(1)} s while playing`)
    }
    lastChange = now
    if (player.state !== lastState) {
      say(`player ${player.state} at ${player.position.toFixed(2)} s`)
      lastState = player.state
    }
    show()
  }

  const ms = (seconds) => (typeof seconds === 'number' ? `${(seconds * 1000).toFixed(1)} ms` : 'not reported')

  function show() {
    const context = player.context
    if (context && context.state !== lastContext) {
      if (lastContext === undefined) context.addEventListener('statechange', show)
      say(`context ${context.state}`)
      lastContext = context.state
    }
    play.innerHTML = player.state === 'playing' ? '&#x23F8;&#xFE0E;' : '&#x25B6;&#xFE0E;'
    play.disabled = !player.midi
    stop.disabled = player.state === 'stopped'
    time.textContent = `${formatTime(player.position)} / ${formatTime(player.duration)}`
    const rows = [
      ['score', player.midi ? `${id}, ${player.midi.notes.length} notes, ${player.midi.duration.toFixed(1)} s` : id],
      ['audioSession', session ? `${session.type}${withSession ? '' : ', hidden from midi.js'}` : 'none in this browser'],
      ['context', context ? `${context.state}, ${context.sampleRate} Hz` : 'not made before ▶'],
      ['baseLatency', context ? ms(context.baseLatency) : '—'],
      ['outputLatency', context ? ms(context.outputLatency) : '—'],
      ['player latency', ms(player.latency)],
      ['heard / clock', `${player.position.toFixed(2)} / ${player.scheduled.toFixed(2)} s`],
    ]
    const stats = context?.playoutStats
    if (stats) rows.push(['playoutStats', JSON.stringify(typeof stats.toJSON === 'function' ? stats.toJSON() : stats)])
    readout.replaceChildren(...rows.flatMap(([name, value]) => {
      const dt = document.createElement('dt')
      const dd = document.createElement('dd')
      dt.textContent = name
      dd.textContent = value
      return [dt, dd]
    }))
  }

  play.addEventListener('click', () => {
    if (player.state === 'playing') return player.pause()
    // Called in the click: the context may only start in a gesture.
    player.play().then((playing) => playing || say('play() refused: no sound before a gesture, says the browser'))
  })
  stop.addEventListener('click', () => player.stop())

  const select = $('score')
  select.value = id
  select.addEventListener('change', () => {
    params.set('score', select.value)
    location.search = params
  })
  const box = $('session')
  box.checked = withSession
  box.addEventListener('change', () => {
    if (box.checked) params.delete('session')
    else params.set('session', 'off')
    location.search = params
  })

  document.addEventListener('visibilitychange', () => say(`page ${document.visibilityState}`))
  // The tick runs while playing; this one keeps the readout current otherwise.
  setInterval(show, 500)
  show()

  fetch(`ref/${id}.midi`)
    .then((response) => {
      if (!response.ok) throw new Error(`ref/${id}.midi: ${response.status}; run npm run corpus, then npm run capture`)
      return response.arrayBuffer()
    })
    .then((bytes) => {
      player.load(parseMidi(new Uint8Array(bytes)))
      say(`${id} loaded, ${(bytes.byteLength / 1024).toFixed(1)} KB`)
    })
    .catch((error) => say(String(error.message ?? error)))
})()
