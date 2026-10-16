import Paper from '@mui/material/Paper'
import Typography from '@mui/material/Typography'
import { useCallback, useEffect, useState } from 'react'

import { invoke } from '../hooks/useDaemon'

// US-005: the read-only state view — what the daemon is actually using, and
// where each value came from. One `state` call returns every output.
export function StateSection() {
  const [states, setStates] = useState<Record<string, unknown>[] | null>(null)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    try {
      const reply = await invoke<string>('daemon_state', { output: null })
      // One JSON object per line, one line per output (§3).
      const lines = reply.split('\n').filter((line) => line.trim().length > 0)
      const parsed = lines.map((line) => JSON.parse(line) as Record<string, unknown>)
      setStates(parsed)
      setError(null)
    } catch (caught) {
      setStates(null)
      setError(String(caught))
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  return (
    <Paper
      elevation={0}
      sx={{
        padding: 4,
        borderRadius: 'var(--m3-radius-lg)',
        background: 'var(--m3-surface)',
        boxShadow: 'var(--m3-shadow-sm)',
      }}
    >
      <Typography variant="h6" sx={{ fontWeight: 700, marginBottom: 2 }}>
        运行期状态
      </Typography>
      {error !== null && (
        <Typography sx={{ color: 'var(--m3-accent-coral)' }}>{error}</Typography>
      )}
      {states !== null && (
        <pre
          style={{
            fontFamily: 'var(--font-mono)',
            fontSize: 12,
            whiteSpace: 'pre-wrap',
            color: 'var(--m3-fg-muted)',
          }}
        >
          {JSON.stringify(states, null, 2)}
        </pre>
      )}
      {states === null && error === null && (
        <Typography sx={{ color: 'var(--m3-fg-muted)' }}>等待守护进程…</Typography>
      )}
    </Paper>
  )
}
