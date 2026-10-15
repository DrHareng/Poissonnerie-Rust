import { mkdirSync } from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const require = createRequire(
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../frontend/package.json'),
)
const sharp = require('sharp')

const frontendDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const sourcePath = path.join(frontendDir, 'images', 'OrgaTounoi.png')
const resDir = path.join(frontendDir, 'android', 'app', 'src', 'main', 'res')
const publicDir = path.join(frontendDir, 'public')

const iconSizes = {
  mdpi: 48,
  hdpi: 72,
  xhdpi: 96,
  xxhdpi: 144,
  xxxhdpi: 192,
}

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

const black = { r: 0, g: 0, b: 0, alpha: 1 }

async function artwork() {
  return sharp(sourcePath).trim({ threshold: 18 }).png().toBuffer()
}

async function onBlack(size, source, scale) {
  const inner = Math.round(size * scale)
  const logo = await sharp(source)
    .resize(inner, inner, { fit: 'contain', background: { ...black, alpha: 0 } })
    .png()
    .toBuffer()
  return sharp({
    create: { width: size, height: size, channels: 4, background: black },
  })
    .composite([{ input: logo, gravity: 'centre' }])
    .png()
    .toBuffer()
}

async function main() {
  const source = await artwork()
  mkdirSync(publicDir, { recursive: true })
  await sharp(source).resize(512, 512, { fit: 'contain', background: black }).png().toFile(path.join(publicDir, 'logo.png'))

  for (const [density, size] of Object.entries(iconSizes)) {
    const dir = path.join(resDir, `mipmap-${density}`)
    mkdirSync(dir, { recursive: true })
    const icon = await onBlack(size, source, 0.92)
    await sharp(icon).toFile(path.join(dir, 'ic_launcher.png'))
    await sharp(icon).toFile(path.join(dir, 'ic_launcher_round.png'))
    const foreground = await onBlack(foregroundSizes[density], source, 0.72)
    await sharp(foreground).toFile(path.join(dir, 'ic_launcher_foreground.png'))
  }

  const splashIcon = await onBlack(432, source, 0.66)
  mkdirSync(path.join(resDir, 'drawable'), { recursive: true })
  await sharp(splashIcon).toFile(path.join(resDir, 'drawable', 'splash_icon.png'))

  for (const [folder, [width, height]] of Object.entries(splashSizes)) {
    const dir = path.join(resDir, folder)
    mkdirSync(dir, { recursive: true })
    const inner = Math.round(Math.min(width, height) * 0.86)
    const logo = await sharp(source)
      .resize(inner, inner, { fit: 'contain', background: { ...black, alpha: 0 } })
      .png()
      .toBuffer()
    await sharp({
      create: { width, height, channels: 4, background: black },
    })
      .composite([{ input: logo, gravity: 'centre' }])
      .png()
      .toFile(path.join(dir, 'splash.png'))
  }
}

await main()
