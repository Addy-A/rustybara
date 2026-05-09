import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

export async function pickPdfFiles() {
  const selected = await open({
    multiple: true,
    filters: [{ name: 'PDF', extensions: ['pdf'] }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function loadMetadata(path) {
  return await invoke('load_metadata', { path });
}

export async function trimMarks(paths, outputDir, overwrite) {
  return await invoke('trim_marks', { paths, outputDir, overwrite });
}

export async function resizeToBleed(paths, bleedInches, outputDir, overwrite) {
  return await invoke('resize_to_bleed', { paths, bleedInches, outputDir, overwrite });
}

export async function exportImages(paths, format, dpi, outputDir) {
  return await invoke('export_images', { paths, format, dpi, outputDir });
}

export async function remapColors(paths, from, to, tolerance, outputDir, overwrite) {
  return await invoke('remap_colors', { paths, from, to, tolerance, outputDir, overwrite });
}

export function basename(path) {
  if (!path) return '';
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return i >= 0 ? path.slice(i + 1) : path;
}

export function colorSpaceTagClass(cs) {
  if (cs === 'PureCMYK') return 'cmyk';
  if (cs === 'PureRGB') return 'rgb';
  if (cs === 'Mixed') return 'mixed';
  return '';
}

export function colorSpaceLabel(cs) {
  switch (cs) {
    case 'PureCMYK': return 'CMYK';
    case 'PureRGB': return 'RGB';
    case 'Mixed': return 'Mixed';
    default: return '—';
  }
}
