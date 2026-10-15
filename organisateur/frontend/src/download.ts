import { Capacitor } from '@capacitor/core'
import { Directory, Filesystem } from '@capacitor/filesystem'
import { Share } from '@capacitor/share'

function bytesToBase64(bytes: Uint8Array) {
  let binary = ''
  const chunk = 0x8000
  for (let index = 0; index < bytes.length; index += chunk) {
    binary += String.fromCharCode(...bytes.subarray(index, index + chunk))
  }
  return btoa(binary)
}

function safeName(name: string) {
  const cleaned = name.trim().replace(/[^\p{L}\p{N}._ -]+/gu, '').replace(/\s+/g, ' ')
  return cleaned || 'tournoi'
}

export async function downloadWorkbook(name: string, bytes: Uint8Array) {
  const filename = `${safeName(name)}.xlsx`
  if (Capacitor.isNativePlatform()) {
    const written = await Filesystem.writeFile({
      path: filename,
      data: bytesToBase64(bytes),
      directory: Directory.Cache,
    })
    await Share.share({
      title: filename,
      text: 'Résultats du tournoi',
      url: written.uri,
    })
    return
  }

  const copy = new Uint8Array(bytes)
  const blob = new Blob([copy], {
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  link.click()
  URL.revokeObjectURL(url)
}
