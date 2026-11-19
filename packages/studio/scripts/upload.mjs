// Publishes what `npm run dist` left in release/ (DECISIONS D124): the DMGs and
// the updates of the version in release/latest.json go to
// https://lily.everyday.fail/dist/, latest.json last, so no installed app is
// offered a file that is not there yet. rsync writes each file under a
// temporary name and renames it, so a download never sees half of one. The
// version comes from latest.json, not package.json: a release is cut on its
// own branch, and main may be behind it. Then it reads everything back through
// the public site, Cloudflare included.
import { execFileSync } from 'node:child_process'
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'

const release = 'release'
// The site's server, as ssh names it (`user@host`, or a Host of ~/.ssh/config).
const host = process.env.LILY_SITE_HOST
if (!host) throw new Error('set LILY_SITE_HOST to the site server, as ssh names it')
const destination = `${host}:/var/www/lily.everyday.fail/dist/`
const site = 'https://lily.everyday.fail/dist/'

const manifest = join(release, 'latest.json')
const { version, platforms } = JSON.parse(readFileSync(manifest, 'utf8'))

const updates = Object.values(platforms).map(({ url }) => {
  if (!url.startsWith(site)) throw new Error(`latest.json lists ${url}, which is not on ${site}`)
  return decodeURIComponent(url.slice(site.length))
})
const missing = updates.filter((name) => !existsSync(join(release, name)))
if (missing.length > 0) throw new Error(`latest.json lists files not in ${release}/: ${missing.join(', ')}`)

const dmgs = readdirSync(release).filter((name) => name.endsWith('.dmg') && name.includes(`-${version}-`))
const files = [...dmgs, ...updates].map((name) => join(release, name))

const rsync = (paths) => execFileSync('rsync', ['-tp', ...paths, destination], { stdio: 'inherit' })
rsync(files)
rsync([manifest])

// A wrong size or manifest here is what an installed app would get.
for (const file of files) {
  const url = site + encodeURIComponent(file.slice(release.length + 1))
  const response = await fetch(url, { method: 'HEAD' })
  const length = Number(response.headers.get('content-length'))
  if (!response.ok || length !== statSync(file).size) {
    throw new Error(`${url}: status ${response.status}, ${length} bytes, ${statSync(file).size} here`)
  }
}
const published = await (await fetch(`${site}latest.json`, { cache: 'no-store' })).text()
if (published !== readFileSync(manifest, 'utf8')) throw new Error(`${site}latest.json differs from ${manifest}`)

console.log(`${version} is on ${site}: ${files.length} files and latest.json`)
