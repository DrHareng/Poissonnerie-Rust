import { copyFileSync, existsSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const frontendDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const repoDir = path.resolve(frontendDir, '..')
const apiOrigin = process.env.VITE_API_ORIGIN || 'http://51.255.44.29/infinity'
const toolsDir = path.join(repoDir, 'tools')

function firstExisting(dir, prefix) {
  if (!existsSync(dir)) return null
  return readdirSync(dir)
    .filter((name) => name.startsWith(prefix))
    .map((name) => path.join(dir, name))
    .find((candidate) => existsSync(candidate)) ?? null
}

const jdkHome = process.env.JAVA_HOME || firstExisting(toolsDir, 'jdk-')
const androidHome =
  process.env.ANDROID_HOME ||
  process.env.ANDROID_SDK_ROOT ||
  path.join(toolsDir, 'android-sdk')

function toolchainEnv() {
  const env = {
    ...process.env,
    CAPACITOR: '1',
    VITE_API_ORIGIN: apiOrigin,
  }
  if (jdkHome && existsSync(jdkHome)) {
    env.JAVA_HOME = jdkHome
    env.PATH = `${path.join(jdkHome, 'bin')}${path.delimiter}${env.PATH ?? ''}`
  }
  if (existsSync(androidHome)) {
    env.ANDROID_HOME = androidHome
    env.ANDROID_SDK_ROOT = androidHome
  }
  return env
}

function run(command, args, cwd = frontendDir) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: 'inherit',
    env: toolchainEnv(),
    shell: true,
  })
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

function writeLocalProperties() {
  if (!existsSync(androidHome)) return
  const sdkDir = androidHome.replaceAll('\\', '/')
  writeFileSync(
    path.join(frontendDir, 'android', 'local.properties'),
    `sdk.dir=${sdkDir}\n`,
  )
}

console.log(`API de l'APK : ${apiOrigin}`)
run('node', ['scripts/generate-android-brand.mjs'])
run('npx', ['vue-tsc', '-b'])
run('npx', ['vite', 'build'])
run('npx', ['cap', 'sync', 'android'])

const androidDir = path.join(frontendDir, 'android')
writeLocalProperties()
const gradle = process.platform === 'win32' ? 'gradlew.bat' : './gradlew'
run(gradle, ['assembleDebug', '--no-daemon'], androidDir)

const apkSrc = path.join(
  androidDir,
  'app',
  'build',
  'outputs',
  'apk',
  'debug',
  'app-debug.apk',
)
const outDir = path.join(frontendDir, '..', 'dist-apk')
mkdirSync(outDir, { recursive: true })
const apkDest = path.join(outDir, 'poissonnerie-debug.apk')
copyFileSync(apkSrc, apkDest)
console.log(`APK prêt : ${apkDest}`)
console.log('')
console.log('Installation :')
console.log('  adb install -r dist-apk/poissonnerie-debug.apk')
console.log('')
console.log("L'APK parle à la prod (pas besoin de adb reverse).")
console.log('Le login Discord nécessite le flux mobile déployé sur le serveur.')
