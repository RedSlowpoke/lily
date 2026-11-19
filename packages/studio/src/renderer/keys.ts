// The transport's arrow keys (DECISIONS D154): ← and → move the music 5 s,
// ↑ and ↓ make the whole of it louder or softer, while it plays or Karaoke is
// shown. Keys typed into what takes arrows itself are left to it.

/** Seconds ← and → move the music. */
export const SEEK_STEP = 5
/** How much ↑ and ↓ change the volume, of 0–1. */
export const VOLUME_STEP = 0.1

export type TransportKey = { seek: number } | { volume: number }

/** What an arrow key does to the music; undefined for any other key, or one held with a modifier (⌘← and the like are the system's). */
export function transportKey(event: Pick<KeyboardEvent, 'key' | 'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey'>): TransportKey | undefined {
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return undefined
  switch (event.key) {
    case 'ArrowLeft':
      return { seek: -SEEK_STEP }
    case 'ArrowRight':
      return { seek: SEEK_STEP }
    case 'ArrowUp':
      return { volume: VOLUME_STEP }
    case 'ArrowDown':
      return { volume: -VOLUME_STEP }
    default:
      return undefined
  }
}

/** `volume` moved by `step`, kept between 0 and 1, in tenths. */
export function stepVolume(volume: number, step: number): number {
  return Math.round(Math.min(Math.max(volume + step, 0), 1) * 10) / 10
}

/** What takes arrow keys itself: text, the editor, a slider, a list, a menu, a splitter. */
const OWN_ARROWS = 'input, textarea, select, [contenteditable="true"], [contenteditable=""], .monaco-editor, [role="separator"], [role="tablist"], [role="tab"], [role="menu"], [role="menuitem"], [role="listbox"], [role="tree"]'

/** Whether arrow keys typed at `target` are its own. */
export function ownsArrows(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(OWN_ARROWS) !== null
}
