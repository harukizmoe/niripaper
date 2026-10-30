import { useCallback, useEffect, useState } from 'react'

// The invoke bridge. In Tauri this reaches app/src/commands.rs; in a plain
// browser (`npm run dev` without Tauri) it degrades to "daemon unreachable",
// which is exactly what the UI must render (PRD US-013).

interface TauriWindow {
  __TAURI_INTERNALS__?: {
    invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>
  }
}

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const internals = (window as unknown as TauriWindow).__TAURI_INTERNALS__
  if (!internals) {
    throw new Error('未在 Tauri 窗口内运行 —— 守护进程连接不可用')
  }
  return internals.invoke(cmd, args) as Promise<T>
}

/// The connection status the app shell renders (US-001: open the app and know
/// whether the daemon is up, and what it manages).
export interface DaemonStatus {
  reachable: boolean
  error?: string
}

export function useDaemon() {
  const [status, setStatus] = useState<DaemonStatus>({ reachable: false })

  const refresh = useCallback(async () => {
    try {
      await invoke<string>('daemon_query', { output: null })
      setStatus({ reachable: true })
    } catch (error) {
      setStatus({ reachable: false, error: String(error) })
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  return { status, refresh }
}
