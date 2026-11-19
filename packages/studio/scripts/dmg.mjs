// The last step of `npm run dist` (DECISIONS D37, D38, D42): `tauri build`
// has signed Lily Studio.app with the Developer ID and the hardened runtime.
// This notarizes and staples the app, puts it in a DMG beside a link to
// /Applications, and signs, notarizes and staples the DMG too, so a
// downloaded copy opens without a warning even offline. Tauri's own DMG step
// would build the app again and lose the app's ticket, hence this script.
// Uses the notarytool keychain profile in $APPLE_KEYCHAIN_PROFILE.
//
// It makes the update too (D124): the stapled app in a .tar.gz, signed with
// the updater's key ($TAURI_SIGNING_PRIVATE_KEY_PATH, ~/.tauri/lily-studio.key
// by default), and release/latest.json, which lists it for this architecture
// beside the other's of the same version. Everything in release/ but the
// .sig files goes to https://lily.everyday.fail/dist/.
//
//   node scripts/dmg.mjs            notarized, for `npm run dist`
//   node scripts/dmg.mjs --local    the ad-hoc signed app as it is, for `npm run dist:local`
//   --target <triple>               the app of `tauri build --target <triple>`, e.g. x86_64-apple-darwin
import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { homedir, tmpdir } from 'node:os'
import { basename, dirname, join } from 'node:path'

const local = process.argv.includes('--local')
const { productName, version } = JSON.parse(readFileSync('package.json', 'utf8'))
const identity = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).bundle.macOS.signingIdentity
const targetAt = process.argv.indexOf('--target')
const target = targetAt < 0 ? undefined : process.argv[targetAt + 1]
const arch = { 'x86_64-apple-darwin': 'x64', 'aarch64-apple-darwin': 'arm64' }[target] ?? process.arch
const app = `target/${target ? `${target}/` : ''}release/bundle/macos/${productName}.app`
const dmg = `release/${productName}-${version}-${arch}.dmg`
const update = `release/${productName}-${version}-${arch}.app.tar.gz`
const updates = 'https://lily.everyday.fail/dist/'
const run = (command, args, env) => execFileSync(command, args, { stdio: 'inherit', env: { ...process.env, ...env } })

const profile = process.env.APPLE_KEYCHAIN_PROFILE
if (!local && !profile) throw new Error('Set APPLE_KEYCHAIN_PROFILE to a `xcrun notarytool store-credentials` profile.')
const key = process.env.TAURI_SIGNING_PRIVATE_KEY_PATH ?? join(homedir(), '.tauri', 'lily-studio.key')
if (!local && !existsSync(key)) throw new Error(`No updater key at ${key}: set TAURI_SIGNING_PRIVATE_KEY_PATH.`)

/** Uploads `file` to Apple, waits for the verdict, and staples the ticket to `target`. */
function notarize(file, target = file) {
  run('xcrun', ['notarytool', 'submit', file, '--keychain-profile', profile, '--wait'])
  run('xcrun', ['stapler', 'staple', target])
}

const scratch = mkdtempSync(join(tmpdir(), 'lily-studio-dmg-'))
try {
  if (!local) {
    // notarytool takes a zip of the app, not the bundle itself.
    const zip = join(scratch, `${productName}.zip`)
    run('ditto', ['-c', '-k', '--keepParent', app, zip])
    notarize(zip, app)
  }

  const staging = join(scratch, productName)
  mkdirSync(staging)
  // ditto keeps the signature, the stapled ticket and the bundle's links.
  run('ditto', [app, join(staging, `${productName}.app`)])
  symlinkSync('/Applications', join(staging, 'Applications'))
  mkdirSync('release', { recursive: true })
  rmSync(dmg, { force: true })
  run('hdiutil', ['create', '-volname', productName, '-srcfolder', staging, '-fs', 'HFS+', '-format', 'UDZO', '-ov', dmg])

  if (!local) {
    run('codesign', ['--sign', identity, '--timestamp', dmg])
    notarize(dmg)
    // Gatekeeper's own verdict on the DMG, as a downloaded copy would get it.
    run('spctl', ['--assess', '--type', 'open', '--context', 'context:primary-signature', '--verbose=2', dmg])
  }
  if (!local) {
    makeUpdate()
  }
  console.log(dmg)
} finally {
  rmSync(scratch, { recursive: true, force: true })
}

/** The update of this architecture, signed, and its entry in release/latest.json. */
function makeUpdate() {
  // The updater unpacks what is below the archive's first component in place
  // of the running app. COPYFILE_DISABLE keeps tar from adding ._ files.
  rmSync(update, { force: true })
  run('tar', ['-czf', update, '-C', dirname(app), basename(app)], { COPYFILE_DISABLE: '1' })
  // requireSignedVersion in tauri.conf.json takes only a signature bound to the version.
  run('npx', ['tauri', 'signer', 'sign', '--private-key-path', key, '--app-version', version, update])
  const platform = { x64: 'darwin-x86_64', arm64: 'darwin-aarch64' }[arch]
  const manifest = 'release/latest.json'
  const previous = existsSync(manifest) ? JSON.parse(readFileSync(manifest, 'utf8')) : undefined
  const latest =
    previous?.version === version ? previous : { version, notes: '', pub_date: new Date().toISOString(), platforms: {} }
  latest.platforms[platform] = {
    signature: readFileSync(`${update}.sig`, 'utf8'),
    url: updates + encodeURIComponent(basename(update)),
  }
  writeFileSync(manifest, `${JSON.stringify(latest, null, 2)}\n`)
  console.log(update)
}
