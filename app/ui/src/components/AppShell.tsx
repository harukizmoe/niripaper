import Box from '@mui/material/Box'
import Typography from '@mui/material/Typography'
import { useState } from 'react'

import { Rail, type Section } from './Rail'
import { DashboardSection } from './DashboardSection'
import { SettingsSection } from './SettingsSection'
import { StateSection } from './StateSection'
import { LibrarySection } from './LibrarySection'
import { useDaemon } from '../hooks/useDaemon'

// The four-area layout from PRD「设计考虑」, in the v1 order (decisions §4.5):
// dashboard/status first, then 设置页 → 运行期状态 → 库网格. The rail is the
// prototype's dark-purple one (tasks/prototype-ui.html).
export function AppShell({ dark, onToggleDark }: { dark: boolean; onToggleDark: () => void }) {
  const [section, setSection] = useState<Section>('dashboard')
  const { status, refresh } = useDaemon()

  return (
    <Box sx={{ display: 'flex', height: '100vh', overflow: 'hidden' }}>
      <Rail
        value={section}
        onChange={setSection}
        daemonReachable={status.reachable}
        dark={dark}
        onToggleDark={onToggleDark}
      />
      <Box
        component="main"
        sx={{
          flex: 1,
          overflow: 'auto',
          padding: 4,
          background: 'var(--m3-canvas)',
        }}
      >
        {section === 'dashboard' && <DashboardSection status={status} onRefresh={refresh} />}
        {section === 'settings' && <SettingsSection />}
        {section === 'state' && <StateSection />}
        {section === 'library' && <LibrarySection />}
        {section !== 'dashboard' && !status.reachable && (
          <Typography variant="body2" sx={{ color: 'var(--m3-fg-muted)', marginTop: 2 }}>
            守护进程未连接 —— 该页面的数据来自守护进程，连接恢复后自动可用。
          </Typography>
        )}
      </Box>
    </Box>
  )
}
