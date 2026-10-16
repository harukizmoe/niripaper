import Box from '@mui/material/Box'
import Drawer from '@mui/material/Drawer'
import IconButton from '@mui/material/IconButton'
import ListItemButton from '@mui/material/ListItemButton'
import ListItemIcon from '@mui/material/ListItemIcon'
import ListItemText from '@mui/material/ListItemText'
import Typography from '@mui/material/Typography'
import DarkModeIcon from '@mui/icons-material/DarkMode'
import LightModeIcon from '@mui/icons-material/LightMode'
import DashboardIcon from '@mui/icons-material/Dashboard'
import TuneIcon from '@mui/icons-material/Tune'
import MonitorIcon from '@mui/icons-material/Monitor'
import PhotoLibraryIcon from '@mui/icons-material/PhotoLibrary'

export type Section = 'dashboard' | 'settings' | 'state' | 'library'

const SECTIONS: { id: Section; label: string; icon: React.ReactNode }[] = [
  { id: 'dashboard', label: '总览', icon: <DashboardIcon /> },
  { id: 'settings', label: '设置', icon: <TuneIcon /> },
  { id: 'state', label: '状态', icon: <MonitorIcon /> },
  { id: 'library', label: '库', icon: <PhotoLibraryIcon /> },
]

// The prototype's dark-purple navigation rail (tasks/prototype-ui.html:
// --m3-rail-*), with the daemon-reachability dot the shell gets from useDaemon.
export function Rail({
  value,
  onChange,
  daemonReachable,
  dark,
  onToggleDark,
}: {
  value: Section
  onChange: (section: Section) => void
  daemonReachable: boolean
  dark: boolean
  onToggleDark: () => void
}) {
  return (
    <Drawer
      variant="permanent"
      sx={{
        width: 220,
        flexShrink: 0,
        '& .MuiDrawer-paper': {
          width: 220,
          background: 'var(--m3-rail-bg)',
          color: 'var(--m3-rail-text)',
          borderRight: 'none',
          display: 'flex',
          flexDirection: 'column',
          padding: 2,
        },
      }}
    >
      <Typography sx={{ padding: 2, color: '#fff', fontWeight: 700, fontSize: 18 }}>
        NiriPaper
      </Typography>
      {SECTIONS.map((section) => {
        const active = section.id === value
        return (
          <ListItemButton
            key={section.id}
            selected={active}
            onClick={() => onChange(section.id)}
            sx={{
              borderRadius: 'var(--m3-radius-md)',
              marginBottom: 0.5,
              color: 'var(--m3-rail-text)',
              '&.Mui-selected': {
                background: 'var(--m3-accent-lime)',
                color: 'var(--m3-rail-text-active)',
                '&:hover': { background: 'var(--m3-accent-lime)' },
              },
              '&:hover': { background: 'var(--m3-rail-item-hover)' },
            }}
          >
            <ListItemIcon sx={{ color: 'inherit', minWidth: 40 }}>{section.icon}</ListItemIcon>
            <ListItemText primary={section.label} />
          </ListItemButton>
        )
      })}
      <Box sx={{ flex: 1 }} />
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, padding: 1 }}>
        <Box
          aria-label={daemonReachable ? '守护进程运行中' : '守护进程未连接'}
          sx={{
            width: 8,
            height: 8,
            borderRadius: '50%',
            background: daemonReachable ? 'var(--m3-accent-mint)' : 'var(--m3-accent-coral)',
          }}
        />
        <Typography variant="caption" sx={{ color: 'var(--m3-rail-text)' }}>
          {daemonReachable ? '守护进程运行中' : '未连接'}
        </Typography>
        <IconButton size="small" onClick={onToggleDark} sx={{ marginLeft: 'auto', color: 'var(--m3-rail-text)' }}>
          {dark ? <LightModeIcon fontSize="small" /> : <DarkModeIcon fontSize="small" />}
        </IconButton>
      </Box>
    </Drawer>
  )
}
