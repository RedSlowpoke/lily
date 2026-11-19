// File › Import Guitar Pro… (DECISIONS D166) on the renderer's side: which
// files it takes, for the web's file picker and for a file dropped on the
// app's window. crates/session's guitar_pro.rs converts them; these lists
// are its EXTENSIONS.

/** What Import Guitar Pro… offers, without the dot. */
export const GUITAR_PRO_EXTENSIONS = ['gp3', 'gp4', 'gp5', 'gpx']
/** Guitar Pro's newer and older files (GP7's .gp, GP1's .gtp): taken from a drop to say why they cannot be imported. */
const REFUSED_EXTENSIONS = ['gp', 'gtp']

function extension(path: string): string | undefined {
  const dot = path.lastIndexOf('.')
  return dot > path.lastIndexOf('/') + 1 ? path.slice(dot + 1).toLowerCase() : undefined
}

/** Whether `path` is a Guitar Pro file by its extension, one that is imported or one refused with a reason. */
export function isGuitarPro(path: string): boolean {
  const ext = extension(path)
  return ext !== undefined && (GUITAR_PRO_EXTENSIONS.includes(ext) || REFUSED_EXTENSIONS.includes(ext))
}

/** The first Guitar Pro file among `paths`, as dropped on the window. */
export function droppedGuitarPro(paths: readonly string[]): string | undefined {
  return paths.find(isGuitarPro)
}

/** The web's file picker's `accept`. */
export const GUITAR_PRO_ACCEPT = GUITAR_PRO_EXTENSIONS.map((ext) => `.${ext}`).join(',')

/** The score's name for a Guitar Pro file: `song.gp5` gives `song.ly`, as guitar_pro.rs's `score_name`. */
export function scoreName(source: string): string {
  const name = source.slice(source.lastIndexOf('/') + 1)
  const dot = name.lastIndexOf('.')
  const stem = dot > 0 ? name.slice(0, dot) : name
  return `${stem || 'Untitled'}.ly`
}
