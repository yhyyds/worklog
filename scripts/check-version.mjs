import { readFileSync } from 'node:fs'

const packageVersion = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8')).version
const tauriVersion = JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8')).version
const cargo = readFileSync(new URL('../src-tauri/Cargo.toml', import.meta.url), 'utf8')
const cargoVersion = cargo.match(/^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)?.[1]
const requestedVersion = process.argv[2]?.replace(/^v/, '')

const versions = { packageVersion, tauriVersion, cargoVersion }
const unique = new Set(Object.values(versions))
if (unique.size !== 1 || !cargoVersion) {
  console.error('版本不一致：', versions)
  process.exit(1)
}

if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(packageVersion)) {
  console.error(`版本不是有效的 SemVer：${packageVersion}`)
  process.exit(1)
}

if (requestedVersion && requestedVersion !== packageVersion) {
  console.error(`发布标签 ${requestedVersion} 与应用版本 ${packageVersion} 不一致`)
  process.exit(1)
}

console.log(`Worklog 版本一致：${packageVersion}`)
