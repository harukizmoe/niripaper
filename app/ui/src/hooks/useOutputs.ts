import { useCallback, useEffect, useState } from 'react'
import { invoke } from './useDaemon'

/// One monitor the daemon draws on, with the size of its canvas.
export interface OutputInfo {
  name: string
  width: number
  height: number
}

/// The outputs the daemon manages. Empty while it is not running, so a page
/// has to work without any: "every output" is always a valid target.
export function useOutputs() {
  const [outputs, setOutputs] = useState<OutputInfo[]>([])

  const refresh = useCallback(async () => {
    try {
      const raw = await invoke<string>('daemon_state', { output: null })
      const parsed = JSON.parse(raw) as {
        outputs?: Record<string, { canvas?: { width?: number; height?: number } }>
      }
      setOutputs(
        Object.entries(parsed.outputs ?? {}).map(([name, o]) => ({
          name,
          width: o.canvas?.width ?? 0,
          height: o.canvas?.height ?? 0,
        })),
      )
    } catch {
      setOutputs([])
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  return { outputs, refresh }
}

/// "Every output" in the daemon's protocol.
export const EVERY_OUTPUT = '*'

/// What a target is called on screen.
export const outputLabel = (target: string) => (target === EVERY_OUTPUT ? '所有显示器' : target)
