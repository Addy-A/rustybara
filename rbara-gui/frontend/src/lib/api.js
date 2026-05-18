import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

export async function pickPdfFiles() {
  const selected = await open({
    multiple: true,
    filters: [{ name: 'PDF', extensions: ['pdf'] }],
  })
  if (!selected) return []
  return Array.isArray(selected) ? selected : [selected]
}

export async function loadMetadata(path) {
  return await invoke('load_metadata', { path })
}

export async function trimMarks(paths, outputDir, overwrite) {
  return await invoke('trim_marks', { paths, outputDir, overwrite })
}

export async function resizeToBleed(paths, bleedInches, outputDir, overwrite) {
  return await invoke('resize_to_bleed', {
    paths,
    bleedInches,
    outputDir,
    overwrite,
  })
}

export async function exportImages(paths, format, dpi, outputDir) {
  return await invoke('export_images', { paths, format, dpi, outputDir })
}

export async function remapColors(
  paths,
  from,
  to,
  tolerance,
  outputDir,
  overwrite,
) {
  return await invoke('remap_colors', {
    paths,
    from,
    to,
    tolerance,
    outputDir,
    overwrite,
  })
}

export async function convertColorSpace(
  paths,
  fromProfile,
  toProfile,
  intent,
  outputDir,
  overwrite,
) {
  return await invoke('convert_color_space', {
    paths,
    fromProfile,
    toProfile,
    intent,
    outputDir,
    overwrite,
  })
}

export async function flattenSpots(paths, outputDir, overwrite) {
  return await invoke('flatten_spots', { paths, outputDir, overwrite })
}

export async function addTrimBox(paths, bleedInches, outputDir, overwrite) {
  return await invoke('add_trim_box', {
    paths,
    bleedInches,
    outputDir,
    overwrite,
  })
}

export async function splitPages(paths, panelWidthPts, outputDir) {
  return await invoke('split_pages', { paths, panelWidthPts, outputDir })
}

export async function extractPages(paths, pageNums, outputDir, overwrite) {
  return await invoke('extract_pages', {
    paths,
    pageNums,
    outputDir,
    overwrite,
  })
}

export async function loadIccProfile() {
  return await invoke('load_icc_profile')
}

export async function listCustomProfiles() {
  return await invoke('list_custom_profiles')
}

export async function openInViewer(path, page = 0, dpi = 150) {
  return await invoke('open_in_viewer', { path, page, dpi })
}

// Parses a 1-indexed page string like "1, 3-5, 7" into 0-indexed numbers for the backend.
export function parsePageNums(input) {
  return [
    ...new Set(
      String(input)
        .split(',')
        .flatMap((s) => {
          s = s.trim()
          const range = s.match(/^(\d+)-(\d+)$/)
          if (range) {
            const from = parseInt(range[1], 10)
            const to = parseInt(range[2], 10)
            return Array.from(
              { length: Math.max(0, to - from + 1) },
              (_, i) => from + i,
            )
          }
          const n = parseInt(s, 10)
          return isNaN(n) ? [] : [n]
        })
        .filter((n) => n >= 1)
        .map((n) => n - 1),
    ),
  ].sort((a, b) => a - b)
}

export function basename(path) {
  if (!path) return ''
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return i >= 0 ? path.slice(i + 1) : path
}

export function colorSpaceTagClass(cs) {
  if (cs === 'PureCMYK') return 'cmyk'
  if (cs === 'PureRGB') return 'rgb'
  if (cs === 'Mixed') return 'mixed'
  return ''
}

export function formatSize(kb) {
  if (kb < 1024) return `${kb} KB`
  if (kb < 1024 * 1024) return `${(kb / 1024).toFixed(1)} MB`
  if (kb < 1024 * 1024 * 1024) return `${(kb / (1024 * 1024)).toFixed(2)} GB`
  return `${(kb / (1024 * 1024 * 1024)).toFixed(2)} TB`
}

export function colorSpaceLabel(cs) {
  switch (cs) {
    case 'PureCMYK':
      return 'CMYK'
    case 'PureRGB':
      return 'RGB'
    case 'Mixed':
      return 'Mixed'
    default:
      return '—'
  }
}
