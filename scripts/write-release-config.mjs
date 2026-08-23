import { writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import process from 'node:process'

const publicKey = process.env.TAURI_UPDATER_PUBLIC_KEY?.trim()
if (!publicKey) {
  throw new Error('TAURI_UPDATER_PUBLIC_KEY is required to create the release configuration')
}

const config = {
  bundle: { createUpdaterArtifacts: true },
  plugins: {
    updater: {
      pubkey: publicKey,
      endpoints: ['https://github.com/patchy23/patchyBox/releases/latest/download/latest.json'],
      windows: { installMode: 'passive' },
    },
  },
}

await writeFile(
  resolve('src-tauri/tauri.release.conf.json'),
  `${JSON.stringify(config, null, 2)}\n`,
  'utf8'
)
