import { mkdirSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import sharp from 'sharp'

const frontendDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const brandDir = path.join(frontendDir, 'public', 'brand')
const resDir = path.join(frontendDir, 'android', 'app', 'src', 'main', 'res')

const faviconPath = path.join(brandDir, 'favicon.png')
const splashSourcePath = path.join(brandDir, 'logo.png')

const iconSizes = {
  mdpi: 48,
  hdpi: 72,
  xhdpi: 96,
  xxhdpi: 144,
  xxxhdpi: 192,
}

/** Adaptive icon foreground canvas = 108dp. */
const foregroundSizes = {
  mdpi: 108,
  hdpi: 162,
  xhdpi: 216,
  xxhdpi: 324,
  xxxhdpi: 432,
}

const splashSizes = {
  'drawable-port-mdpi': [320, 480],
  'drawable-port-hdpi': [480, 800],
  'drawable-port-xhdpi': [720, 1280],
  'drawable-port-xxhdpi': [960, 1600],
  'drawable-port-xxxhdpi': [1280, 1920],
  'drawable-land-mdpi': [480, 320],
  'drawable-land-hdpi': [800, 480],
  'drawable-land-xhdpi': [1280, 720],
  'drawable-land-xxhdpi': [1600, 960],
  'drawable-land-xxxhdpi': [1920, 1280],
  drawable: [1280, 1920],
}

async function makeLauncherIcon(size, outPath) {
  const fishSize = Math.round(size * 0.78)
  const fish = await sharp(faviconPath)
    .resize(fishSize, fishSize, {
      fit: 'contain',
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    })
    .png()
    .toBuffer()

  await sharp({
    create: {
      width: size,
      height: size,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 255 },
    },
  })
    .composite([{ input: fish, gravity: 'centre' }])
    .png()
    .toFile(outPath)
}

async function makeForeground(size, outPath) {
  // Keep artwork inside the adaptive safe zone (~66dp of 108dp).
  const fishSize = Math.round(size * (66 / 108))
  const fish = await sharp(faviconPath)
    .resize(fishSize, fishSize, {
      fit: 'contain',
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    })
    .png()
    .toBuffer()

  await sharp({
    create: {
      width: size,
      height: size,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    },
  })
    .composite([{ input: fish, gravity: 'centre' }])
    .png()
    .toFile(outPath)
}

async function makeSplash(width, height, outPath) {
  await sharp(splashSourcePath)
    .resize(width, height, { fit: 'cover', position: 'centre' })
    .png()
    .toFile(outPath)
}

async function main() {
  for (const [density, size] of Object.entries(iconSizes)) {
    const dir = path.join(resDir, `mipmap-${density}`)
    mkdirSync(dir, { recursive: true })
    await makeLauncherIcon(size, path.join(dir, 'ic_launcher.png'))
    await makeLauncherIcon(size, path.join(dir, 'ic_launcher_round.png'))
    await makeForeground(foregroundSizes[density], path.join(dir, 'ic_launcher_foreground.png'))
  }

  for (const [folder, [width, height]] of Object.entries(splashSizes)) {
    const dir = path.join(resDir, folder)
    mkdirSync(dir, { recursive: true })
    await makeSplash(width, height, path.join(dir, 'splash.png'))
  }

  console.log('Icônes Android : favicon.png')
  console.log('Splash Android : logo.png')
}

await main()
