// Wallhaven thumbnails are cached on disk by the shell (app/src/thumbs.rs).
// The masonry unmounts whatever scrolls away, so without this every return
// trip re-requested pictures the user had already seen.

import { convertFileSrc } from '@tauri-apps/api/core'
import { invoke } from '../hooks/useDaemon'

/// id → `convertFileSrc` URL of the cached file.
const resolved = new Map<string, string>()
/// id → the request in flight, so two mounts of one card share it.
const pending = new Map<string, Promise<string>>()

/// The cached file for this session, if it is already known.
export function cachedThumb(id: string): string | undefined {
  return resolved.get(id)
}

/// Resolve the local file for a thumbnail, downloading it on first use.
/// Falls back to the remote URL when the shell cannot cache it, so a cache
/// failure costs nothing but the old behaviour.
export function loadThumb(id: string, remote: string): Promise<string> {
  const done = resolved.get(id)
  if (done) return Promise.resolve(done)
  const inFlight = pending.get(id)
  if (inFlight) return inFlight

  const request = invoke<string>('wallhaven_thumbnail', { id, url: remote })
    .then((path) => {
      const src = convertFileSrc(path)
      resolved.set(id, src)
      return src
    })
    .catch(() => remote)
    .finally(() => pending.delete(id))
  pending.set(id, request)
  return request
}
