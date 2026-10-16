import CssBaseline from '@mui/material/CssBaseline'
import { ThemeProvider } from '@mui/material/styles'
import { useMemo, useState } from 'react'

import { AppShell } from './components/AppShell'
import { darkTheme, lightTheme } from './theme/mui'

// Shell + theme. The four v1 sections follow decisions §4.5:
// 设置页 → 运行期状态 → 库网格; the dashboard (US-001) comes first.
export default function App() {
  const [dark, setDark] = useState(false)
  const theme = useMemo(() => (dark ? darkTheme : lightTheme), [dark])

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <div data-theme={dark ? 'dark' : 'light'} className={dark ? 'dark' : ''}>
        <AppShell dark={dark} onToggleDark={() => setDark((value) => !value)} />
      </div>
    </ThemeProvider>
  )
}
