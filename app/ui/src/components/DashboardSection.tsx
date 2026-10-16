import Button from '@mui/material/Button'
import Paper from '@mui/material/Paper'
import Typography from '@mui/material/Typography'

import { invoke } from '../hooks/useDaemon'
import type { DaemonStatus } from '../hooks/useDaemon'

// US-001: open the app and know whether the daemon is running, plus the
// query one-liner (what is on screen right now).
export function DashboardSection({ status, onRefresh }: { status: DaemonStatus; onRefresh: () => Promise<void> }) {
  const queryNow = async () => {
    try {
      const reply = await invoke<string>('daemon_query', { output: null })
      window.alert(reply)
    } catch (error) {
      window.alert(String(error))
    }
  }

  return (
    <Paper
      elevation={0}
      sx={{
        padding: 4,
        borderRadius: 'var(--m3-radius-lg)',
        background: 'var(--m3-surface)',
        boxShadow: 'var(--m3-shadow-sm)',
        maxWidth: 720,
      }}
    >
      <Typography variant="h5" sx={{ fontWeight: 700, marginBottom: 1 }}>
        {status.reachable ? '守护进程在运行' : '守护进程未运行'}
      </Typography>
      <Typography sx={{ color: 'var(--m3-fg-muted)', marginBottom: 3 }}>
        {status.reachable
          ? '壁纸引擎已就绪。到「库」选一张图换上，或到「设置」调参。'
          : status.error ?? '没有找到 $XDG_RUNTIME_DIR/niripaper.sock —— 启动守护进程后再打开本应用。'}
      </Typography>
      <div style={{ display: 'flex', gap: 12 }}>
        <Button variant="contained" onClick={onRefresh} sx={{ borderRadius: 'var(--m3-radius-full)' }}>
          刷新连接
        </Button>
        <Button
          variant="outlined"
          onClick={queryNow}
          disabled={!status.reachable}
          sx={{ borderRadius: 'var(--m3-radius-full)' }}
        >
          查询当前画面
        </Button>
      </div>
    </Paper>
  )
}
