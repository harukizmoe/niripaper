import type { ReactNode } from 'react'
import { Box, ButtonBase, IconButton, Tooltip, Typography } from '@mui/material'
import { alpha } from '@mui/material/styles'
import WallpaperOutlined from '@mui/icons-material/WallpaperOutlined'
import WallpaperRounded from '@mui/icons-material/WallpaperRounded'
import AnimationOutlined from '@mui/icons-material/AnimationOutlined'
import AnimationRounded from '@mui/icons-material/AnimationRounded'
import LayersOutlined from '@mui/icons-material/LayersOutlined'
import LayersRounded from '@mui/icons-material/LayersRounded'
import SettingsOutlined from '@mui/icons-material/SettingsOutlined'
import SettingsRounded from '@mui/icons-material/SettingsRounded'
import DarkModeOutlined from '@mui/icons-material/DarkModeOutlined'
import LightModeOutlined from '@mui/icons-material/LightModeOutlined'
import BrightnessAutoOutlined from '@mui/icons-material/BrightnessAutoOutlined'
import type { ThemeMode } from '../App'
import { shape } from '../theme/mui'

export type NavTab = 'wallpapers' | 'transitions' | 'parallax' | 'settings'

const DESTINATIONS: { tab: NavTab; label: string; icon: ReactNode; active: ReactNode }[] = [
  { tab: 'wallpapers', label: '壁纸', icon: <WallpaperOutlined />, active: <WallpaperRounded /> },
  { tab: 'transitions', label: '过渡', icon: <AnimationOutlined />, active: <AnimationRounded /> },
  { tab: 'parallax', label: '视差', icon: <LayersOutlined />, active: <LayersRounded /> },
  { tab: 'settings', label: '设置', icon: <SettingsOutlined />, active: <SettingsRounded /> },
]

interface RailProps {
  currentTab: NavTab
  onTabChange: (tab: NavTab) => void
  themeMode: ThemeMode
  isDark: boolean
  onCycleTheme: () => void
}

/// M3 navigation rail: an indicator pill around the icon, the label below it.
export function Rail({ currentTab, onTabChange, themeMode, isDark, onCycleTheme }: RailProps) {
  return (
    <Box
      component="nav"
      sx={{
        width: 88,
        flexShrink: 0,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: 1.5,
        py: 3,
        userSelect: 'none',
      }}
    >
      {DESTINATIONS.map((d) => {
        const selected = d.tab === currentTab
        return (
          <ButtonBase
            key={d.tab}
            onClick={() => onTabChange(d.tab)}
            aria-current={selected ? 'page' : undefined}
            // M3 puts the state layer on the indicator, not the whole item.
            disableRipple
            sx={(t) => ({
              flexDirection: 'column',
              gap: 0.5,
              width: 72,
              borderRadius: `${shape.lg}px`,
              color: selected ? t.palette.md.on_surface : t.palette.md.on_surface_variant,
              '& .indicator': {
                width: 56,
                height: 32,
                borderRadius: `${shape.full}px`,
                display: 'grid',
                placeItems: 'center',
                bgcolor: selected ? t.palette.md.secondary_container : 'transparent',
                color: selected ? t.palette.md.on_secondary_container : 'inherit',
                transition: 'background-color 200ms cubic-bezier(0.2, 0, 0, 1)',
              },
              '&:hover .indicator': {
                bgcolor: selected
                  ? t.palette.md.secondary_container
                  : alpha(t.palette.md.on_surface, 0.08),
              },
              '&.Mui-focusVisible .indicator': {
                outline: `2px solid ${t.palette.md.secondary}`,
                outlineOffset: 2,
              },
            })}
          >
            <span className="indicator">{selected ? d.active : d.icon}</span>
            <Typography variant="caption" sx={{ fontWeight: selected ? 700 : 500 }}>
              {d.label}
            </Typography>
          </ButtonBase>
        )
      })}

      <Box sx={{ mt: 'auto' }}>
        <Tooltip
          title={
            themeMode === 'system'
              ? `外观：跟随系统（当前${isDark ? '深色' : '浅色'}）`
              : themeMode === 'light'
                ? '外观：浅色模式'
                : '外观：深色模式'
          }
          placement="right"
        >
          <IconButton onClick={onCycleTheme} aria-label="切换外观模式">
            {themeMode === 'system' ? (
              <BrightnessAutoOutlined />
            ) : isDark ? (
              <DarkModeOutlined />
            ) : (
              <LightModeOutlined />
            )}
          </IconButton>
        </Tooltip>
      </Box>
    </Box>
  )
}
