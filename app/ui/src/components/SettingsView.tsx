import { useState, useEffect, useCallback } from 'react'
import { Box, Button, IconButton, MenuItem, Select, Switch, Tooltip, Typography } from '@mui/material'
import RefreshRounded from '@mui/icons-material/RefreshRounded'
import { invoke } from '../hooks/useDaemon'
import { SCHEME_LABELS, SCHEME_NAMES, DEFAULT_SCHEME, type SchemeName } from '../theme/color'
import { SettingRow, SettingsGroup, SettingsPage } from './settings'

interface SettingsViewProps {
  followWallpaper: boolean
  onToggleFollowWallpaper: () => void
  seedColor: string
  enableBlur: boolean
  onToggleEnableBlur: () => void
  lazyLoad: boolean
  onToggleLazyLoad: () => void
  colorScheme: SchemeName
  onColorSchemeChange: (next: SchemeName) => void
  paletteError: string | null
  adaptNoctalia: boolean
  onToggleAdaptNoctalia: () => void
  externalPalette: Record<string, string> | null
  syncSystemTheme: boolean
  onToggleSyncSystemTheme: () => void
  onNotify: (msg: string) => void
}

interface DaemonStatusInfo {
  running: boolean
}

const mono = { fontFamily: '"Roboto Mono", monospace', fontSize: 13 }

export function SettingsView({
  followWallpaper,
  onToggleFollowWallpaper,
  seedColor,
  enableBlur,
  onToggleEnableBlur,
  lazyLoad,
  onToggleLazyLoad,
  colorScheme,
  onColorSchemeChange,
  paletteError,
  adaptNoctalia,
  onToggleAdaptNoctalia,
  externalPalette,
  syncSystemTheme,
  onToggleSyncSystemTheme,
  onNotify,
}: SettingsViewProps) {
  // `null` until the first check answers, so the row does not flash "未运行".
  const [daemonRunning, setDaemonRunning] = useState<boolean | null>(null)
  const [autostartEnabled, setAutostartEnabled] = useState(false)
  const [configPath, setConfigPath] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const checkDaemon = useCallback(async () => {
    try {
      const info = await invoke<DaemonStatusInfo>('daemon_status_info')
      setDaemonRunning(info.running)
    } catch {
      setDaemonRunning(false)
    }
  }, [])

  useEffect(() => {
    void checkDaemon()
    invoke<boolean>('get_autostart_status').then(setAutostartEnabled, () => setAutostartEnabled(false))
    invoke<string | null>('config_path').then(setConfigPath, () => setConfigPath(null))
  }, [checkDaemon])

  const runDaemon = async (start: boolean) => {
    setBusy(true)
    try {
      await invoke(start ? 'daemon_start' : 'daemon_kill')
      await checkDaemon()
    } catch (e) {
      onNotify(`${start ? '启动' : '停止'}壁纸引擎失败：${e}`)
    } finally {
      setBusy(false)
    }
  }

  const handleToggleAutostart = async () => {
    const next = !autostartEnabled
    try {
      await invoke<string>('set_autostart_enabled', { enabled: next })
      setAutostartEnabled(next)
    } catch (e) {
      onNotify(`设置自动启动失败：${e}`)
    }
  }

  // Following Noctalia replaces everything this app derives itself, and
  // pushing our palette to Noctalia while reading Noctalia's would feed it its
  // own colours back under our name.
  const following = adaptNoctalia && externalPalette !== null

  return (
    <SettingsPage title="设置">
      <SettingsGroup title="壁纸引擎">
        <SettingRow
          label={daemonRunning === null ? '正在检测…' : daemonRunning ? '正在运行' : '未运行'}
          description={
            daemonRunning
              ? '关闭本窗口不会停止壁纸，引擎在后台独立运行。'
              : '启动后由 niripaper 绘制桌面壁纸。'
          }
          control={
            <Box sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
              <Tooltip title="重新检测">
                <IconButton onClick={() => void checkDaemon()} disabled={busy} aria-label="重新检测">
                  <RefreshRounded />
                </IconButton>
              </Tooltip>
              {daemonRunning ? (
                <Button variant="outlined" onClick={() => void runDaemon(false)} disabled={busy}>
                  停止
                </Button>
              ) : (
                <Button
                  variant="contained"
                  onClick={() => void runDaemon(true)}
                  disabled={busy || daemonRunning === null}
                >
                  启动
                </Button>
              )}
            </Box>
          }
        />
        <SettingRow
          label="登录时自动启动"
          description="登录桌面后在后台启动壁纸引擎，不打开本窗口。"
          control={<Switch checked={autostartEnabled} onChange={handleToggleAutostart} />}
        />
        <SettingRow
          label="配置文件"
          description={<Box component="span" sx={mono}>{configPath ?? '~/.config/niripaper/config.toml'}</Box>}
        />
      </SettingsGroup>

      <SettingsGroup title="配色" description="本窗口的界面配色。">
        <SettingRow
          label="从壁纸取色"
          description={
            <>
              界面配色取自当前壁纸。当前取色{' '}
              <Box
                component="span"
                sx={{
                  display: 'inline-block',
                  width: 12,
                  height: 12,
                  borderRadius: '50%',
                  bgcolor: seedColor,
                  verticalAlign: '-1px',
                  border: 1,
                  borderColor: 'divider',
                }}
              />{' '}
              <Box component="span" sx={mono}>
                {seedColor.toUpperCase()}
              </Box>
            </>
          }
          disabled={following}
          control={<Switch checked={followWallpaper} onChange={onToggleFollowWallpaper} disabled={following} />}
        />
        <SettingRow
          label="配色方案"
          description="同一个取色派生出的配色风格。"
          disabled={following}
          modified={colorScheme !== DEFAULT_SCHEME}
          onReset={() => onColorSchemeChange(DEFAULT_SCHEME)}
          control={
            <Select
              size="small"
              value={colorScheme}
              disabled={following}
              onChange={(e) => onColorSchemeChange(e.target.value as SchemeName)}
              sx={{ minWidth: 160 }}
            >
              {SCHEME_NAMES.map((name) => (
                <MenuItem key={name} value={name}>
                  {SCHEME_LABELS[name]}
                </MenuItem>
              ))}
            </Select>
          }
        />
        <SettingRow
          label="使用 Noctalia 的配色"
          description={
            adaptNoctalia && !externalPalette
              ? '没有找到 Noctalia 的调色板，暂时沿用壁纸取色。'
              : '改用 Noctalia 当前的调色板。只读取，不修改它的设置。'
          }
          control={<Switch checked={adaptNoctalia} onChange={onToggleAdaptNoctalia} />}
        />
        <SettingRow
          label="同步到 Noctalia"
          description={
            adaptNoctalia
              ? '正在使用 Noctalia 的配色，无需同步。'
              : '把这套配色设为 Noctalia 的调色板，跟随主题的应用会一起变色。'
          }
          disabled={adaptNoctalia}
          control={
            <Switch checked={syncSystemTheme && !adaptNoctalia} onChange={onToggleSyncSystemTheme} disabled={adaptNoctalia} />
          }
        />
        {paletteError && (
          <SettingRow
            label="调色板写入失败"
            description={
              <Typography component="span" variant="body1" sx={{ color: 'error.main', ...mono }}>
                {paletteError}
              </Typography>
            }
          />
        )}
      </SettingsGroup>

      <SettingsGroup title="壁纸库">
        <SettingRow
          label="悬停时虚化其他壁纸"
          description="鼠标停在一张壁纸上时，其余壁纸轻微模糊。"
          control={<Switch checked={enableBlur} onChange={onToggleEnableBlur} />}
        />
        <SettingRow
          label="滚动到附近再加载预览图"
          description="关闭后一次加载全部预览图。"
          control={<Switch checked={lazyLoad} onChange={onToggleLazyLoad} />}
        />
      </SettingsGroup>
    </SettingsPage>
  )
}
