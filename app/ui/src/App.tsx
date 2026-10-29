import { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import { Alert, Box, CssBaseline, Snackbar } from '@mui/material'
import { ThemeProvider } from '@mui/material/styles'
import { invoke } from './hooks/useDaemon'
import { Rail, type NavTab } from './components/Rail'
import { WallpaperView } from './components/WallpaperView'
import { TransitionsView } from './components/TransitionsView'
import { SettingsView } from './components/SettingsView'
import { ParallaxView } from './components/ParallaxView'
import { WallpaperDrawer } from './components/WallpaperDrawer'
import {
  whWallpapers,
  localWallpapers as initialLocalWallpapers,
  type WallpaperItem,
} from './data/wallpapers'
import {
  applyRoleVars,
  buildRoles,
  DEFAULT_SCHEME,
  isSchemeName,
  pickSeedColor,
  rolesFromPalette,
  THEME_SEED_FALLBACK,
  type SchemeName,
} from './theme/color'
import { buildTheme } from './theme/mui'

const DISPLAYS = ['DP-1 (主屏)', 'eDP-1 (内屏)', '全部显示器 (*)']

/// The wallpaper the daemon is actually drawing, if it can be determined.
async function currentWallpaperPath(): Promise<string | null> {
  try {
    const raw = await invoke<string>('daemon_state', { output: null })
    const parsed = JSON.parse(raw) as {
      outputs?: Record<string, { wallpaper?: { path?: string } | null }>
    }
    const outputs = parsed.outputs ?? {}
    for (const key of Object.keys(outputs)) {
      const path = outputs[key]?.wallpaper?.path
      if (path) return path
    }
    return null
  } catch {
    return null
  }
}

export default function App() {
  const [currentTab, setCurrentTab] = useState<NavTab>('wallpapers')
  const [currentSource, setCurrentSource] = useState<'wallhaven' | 'local'>('local')
  const [displayIdx, setDisplayIdx] = useState(0)
  // Light/dark is remembered; the first run follows the desktop's preference.
  const [isDark, setIsDark] = useState<boolean>(() => {
    const saved = localStorage.getItem('niripaper_dark')
    return saved !== null ? saved === 'true' : matchMedia('(prefers-color-scheme: dark)').matches
  })
  const [followWallpaper, setFollowWallpaper] = useState<boolean>(
    () => localStorage.getItem('niripaper_follow_wallpaper') !== 'false',
  )
  // Read by the library scan, which is a stable callback and must not seed
  // the theme once the user has switched following off. Written where the
  // switch is toggled, the only place the value changes.
  const followRef = useRef(followWallpaper)
  const [enableBlur, setEnableBlur] = useState<boolean>(() => {
    const saved = localStorage.getItem('niripaper_enable_blur')
    return saved !== null ? saved === 'true' : true
  })
  const [lazyLoad, setLazyLoad] = useState<boolean>(() => {
    const saved = localStorage.getItem('niripaper_lazy_load')
    return saved !== null ? saved === 'true' : true
  })
  const [localList, setLocalList] = useState<WallpaperItem[]>(initialLocalWallpapers)
  const [wallhavenList, setWallhavenList] = useState<WallpaperItem[]>(whWallpapers)
  const [whLoading, setWhLoading] = useState(false)
  const [whTotal, setWhTotal] = useState(1800)

  // Theme seed and the palette's last export failure. Declared before the
  // wallpaper scan below: that callback sets the seed, and a `const` used
  // before its declaration is a TDZ error that takes the whole view down on
  // first render.
  // Until the library scan finds the wallpaper on screen, the fallback: the
  // built-in sample list is placeholder data, not a colour worth painting.
  const [themeSeed, setThemeSeed] = useState<string>(THEME_SEED_FALLBACK)
  const [paletteError, setPaletteError] = useState<string | null>(null)
  // Opt-in: follow the system palette instead of generating our own. The
  // palette is read, never written, so enabling this cannot break another
  // program's configuration.
  const [adaptNoctalia, setAdaptNoctalia] = useState<boolean>(() => {
    return localStorage.getItem('niripaper_adapt_noctalia') === 'true'
  })
  const [externalPalette, setExternalPalette] = useState<Record<string, string> | null>(null)
  // Push the computed palette to the system daemon, so the whole desktop
  // follows niripaper's wallpaper. Separate from `adaptNoctalia`, which reads.
  const [syncSystemTheme, setSyncSystemTheme] = useState<boolean>(() => {
    return localStorage.getItem('niripaper_sync_system_theme') === 'true'
  })


  // 1. 扫描并加载本地真实壁纸 (~/Pictures/Wallpapers)
  // The real scan replaces the built-in sample list, so the theme seed has to
  // follow it: seeding from the placeholders meant the palette stayed on a
  // made-up colour after every start, and changing the wallpaper did not move
  // it either.
  //
  // Which wallpaper is "current" comes from the daemon, not from `activeOn`:
  // that flag only tracks what this session applied, so after a restart it is
  // null and the seed would never settle on the wallpaper actually on screen.
  const refreshLocalWallpapers = useCallback(async () => {
    try {
      const items = await invoke<WallpaperItem[]>('load_local_wallpapers')
      if (items && items.length > 0) {
        setLocalList(items)
        void currentWallpaperPath()
          .then((path) => {
            if (!path) return
            const current = items.find((w) => w.path === path)
            if (!current) {
              console.warn('当前壁纸不在本地库中，无法取色:', path)
            }
            if (current && followRef.current) {
              setThemeSeed(pickSeedColor(current.colors))
            }
          })
          .catch((e) => {
            console.warn('读取当前壁纸失败，沿用现有种子:', e)
          })
      }
    } catch (e) {
      console.warn('读取本地壁纸库失败:', e)
    }
  }, [])

  // Snackbar. A new message replaces the current one; MUI handles the timeout.
  const [snackbar, setSnackbar] = useState<{ key: number; message: string; failed: boolean } | null>(null)
  const showSnackbar = useCallback((message: string) => {
    // A message that reports a failure is shown as one; a leading emoji from
    // older call sites is dropped, the icon says what kind of message it is.
    setSnackbar({
      key: Date.now(),
      message: message.replace(/^\p{Extended_Pictographic}\uFE0F?\s*/u, ''),
      failed: /失败/.test(message),
    })
  }, [])
  // 2. 真实调用 Wallhaven API v1 检索壁纸
  const handleSearchWallhaven = useCallback(
    async (params: {
      q?: string
      categories?: string
      purity?: string
      sorting?: string
      order?: string
      page?: number
      ratios?: string
      atleast?: string
    }) => {
      setWhLoading(true)
      try {
        const res = await invoke<{
          wallpapers: WallpaperItem[]
          total: number
          last_page: number
          current_page: number
        }>('search_wallhaven', { params })
        if (res && res.wallpapers && res.wallpapers.length > 0) {
          setWallhavenList(res.wallpapers)
          setWhTotal(res.total)
        }
      } catch (e) {
        console.warn('检索 Wallhaven 失败:', e)
        showSnackbar(`Wallhaven 检索失败: ${e}`)
      } finally {
        setWhLoading(false)
      }
    },
    [showSnackbar]
  )

  // 启动时同时拉取本地库与在线热门源
  useEffect(() => {
    void refreshLocalWallpapers()
    void handleSearchWallhaven({
      sorting: 'toplist',
      page: 1,
      categories: '110',
      purity: '100',
    })
  }, [refreshLocalWallpapers, handleSearchWallhaven])

  // Drawer state
  const [drawerOpen, setDrawerOpen] = useState(false)
  const [selectedWp, setSelectedWp] = useState<WallpaperItem | null>(null)
  const [selectedSource, setSelectedSource] = useState<'wallhaven' | 'local'>('wallhaven')
  const [drawerTargetDisplay, setDrawerTargetDisplay] = useState('DP-1')

  // Which colour-generation style the seed is rendered through.
  const [colorScheme, setColorScheme] = useState<SchemeName>(() => {
    const saved = localStorage.getItem('niripaper_color_scheme')
    return saved && isSchemeName(saved) ? saved : DEFAULT_SCHEME
  })


  // Pull the system palette while the switch is on, and once more whenever the
  // wallpaper changes (a new wallpaper means a new palette). Best effort: a
  // missing file just leaves `externalPalette` null and we keep generating.
  const paletteRefresh = adaptNoctalia ? themeSeed : null
  useEffect(() => {
    if (!adaptNoctalia) {
      setExternalPalette(null)
      return
    }
    let cancelled = false
    void invoke<Record<string, string> | null>('read_external_palette')
      .then((p) => {
        if (cancelled) return
        if (p && p['primary']) {
          setExternalPalette(p)
        } else {
          // The settings row says so; no toast on top of it.
          setExternalPalette(null)
        }
      })
      .catch((e) => {
        if (cancelled) return
        setExternalPalette(null)
        console.warn('读取系统调色板失败:', e)
      })
    return () => {
      cancelled = true
    }
  }, [adaptNoctalia, paletteRefresh])

  // The roles in force: the system palette when following it, otherwise our
  // own scheme from the wallpaper seed. One map drives the MUI theme, the CSS
  // variables, the exported file and the desktop push, so they cannot differ.
  const roles = useMemo(
    () =>
      (adaptNoctalia && externalPalette ? rolesFromPalette(externalPalette) : null) ??
      buildRoles(themeSeed, isDark, colorScheme),
    [themeSeed, isDark, colorScheme, externalPalette, adaptNoctalia],
  )
  const theme = useMemo(() => buildTheme(roles, isDark), [roles, isDark])
  const following = adaptNoctalia && externalPalette !== null

  useEffect(() => {
    document.documentElement.classList.toggle('dark', isDark)
    applyRoleVars(roles)

    // Export the palette so other tools can follow it. Best effort: a failure
    // here must not affect the app's own colours.
    void invoke<string>('export_palette', {
      seed: following ? roles.primary : themeSeed,
      scheme: following ? 'system' : colorScheme,
      dark: isDark,
      tokens: roles,
    }).catch((e) => {
      console.error('导出调色板失败（不影响本应用配色）:', e)
      setPaletteError(String(e))
    })

    // Push the same palette to the desktop's theme daemon — never while
    // reading its palette, which would hand it its own colours back.
    if (syncSystemTheme && !adaptNoctalia) {
      const pick = (k: string) => roles[k]
      void invoke<string>('apply_system_palette', {
        palette: Object.fromEntries(
          [
            'primary', 'on_primary', 'secondary', 'on_secondary', 'tertiary', 'on_tertiary',
            'error', 'on_error', 'surface', 'on_surface', 'surface_variant',
            'on_surface_variant', 'outline',
          ].map((k) => [k, pick(k)]),
        ),
      })
        .then(() => setPaletteError(null))
        .catch((e) => {
          console.error('同步系统主题失败:', e)
          setPaletteError(String(e))
        })
    }
  }, [roles, isDark, following, themeSeed, colorScheme, syncSystemTheme, adaptNoctalia])

  const handleToggleSyncSystemTheme = () => {
    setSyncSystemTheme((prev) => {
      const next = !prev
      localStorage.setItem('niripaper_sync_system_theme', String(next))
      return next
    })
  }

  const handleToggleAdaptNoctalia = () => {
    setAdaptNoctalia((prev) => {
      const next = !prev
      localStorage.setItem('niripaper_adapt_noctalia', String(next))
      return next
    })
  }

  const handleColorSchemeChange = (next: SchemeName) => {
    setColorScheme(next)
    localStorage.setItem('niripaper_color_scheme', next)
  }

  // ESC key to close drawer
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setDrawerOpen(false)
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  const handleToggleTheme = () => {
    setIsDark((prev) => {
      localStorage.setItem('niripaper_dark', String(!prev))
      return !prev
    })
  }

  const handleToggleFollowWallpaper = () => {
    const next = !followWallpaper
    setFollowWallpaper(next)
    followRef.current = next
    localStorage.setItem('niripaper_follow_wallpaper', String(next))
    if (!next) {
      setThemeSeed(THEME_SEED_FALLBACK)
      return
    }
    // Seed from what the daemon is drawing, as the library scan does.
    void currentWallpaperPath().then((path) => {
      const current = path ? localList.find((w) => w.path === path) : undefined
      if (current) setThemeSeed(pickSeedColor(current.colors))
    })
  }

  const handleToggleBlur = () => {
    setEnableBlur((prev) => {
      const next = !prev
      localStorage.setItem('niripaper_enable_blur', String(next))
      return next
    })
  }
  const handleToggleLazyLoad = () => {
    setLazyLoad((prev) => {
      const next = !prev
      localStorage.setItem('niripaper_lazy_load', String(next))
      return next
    })
  }

  const handleCycleDisplay = () => {
    setDisplayIdx((prev) => {
      const next = (prev + 1) % DISPLAYS.length
      showSnackbar(`切换当前目标屏幕为：${DISPLAYS[next]}`)
      return next
    })
  }

  const handleSelectWallpaper = (wp: WallpaperItem, source: 'wallhaven' | 'local') => {
    setSelectedWp(wp)
    setSelectedSource(source)
    setDrawerOpen(true)
  }

  const handleApplyWallpaper = async (params: {
    fit: string
    scale: number
    filterPreset: string
  }) => {
    if (!selectedWp) return

    let finalPath = selectedWp.path

    if (selectedSource === 'wallhaven') {
      showSnackbar(`⏳ 正在下载并应用 Wallhaven 壁纸「${selectedWp.name}」...`)
      try {
        const ext =
          selectedWp.file_type === 'image/png' ? '.png' : '.jpg'
        const filename =
          selectedWp.name.endsWith('.png') ||
          selectedWp.name.endsWith('.jpg') ||
          selectedWp.name.endsWith('.webp')
            ? selectedWp.name
            : `${selectedWp.name}${ext}`
        finalPath = await invoke<string>('download_wallhaven_wallpaper', {
          fullUrl: selectedWp.full_url || selectedWp.thumb,
          filename,
        })
        await refreshLocalWallpapers()
      } catch (e) {
        showSnackbar(`下载失败: ${e}`)
        return
      }
    }

    if (finalPath) {
      try {
        // Persist first, then swap the screen: `daemon_set` alone leaves
        // config.toml untouched, which is why the wallpaper was forgotten on
        // restart. `scale` here is the drawer's zoom, mapped onto the daemon's
        // canvas scale only when it would actually enlarge the canvas.
        await invoke('daemon_apply', {
          output: drawerTargetDisplay,
          path: finalPath,
          fit: params.fit,
          scale: null,
        })
      } catch (e) {
        showSnackbar(`应用壁纸失败: ${e}`)
        return
      }
    }

    if (selectedSource === 'local') {
      setLocalList((prev) =>
        prev.map((w) => {
          if (w.activeOn === drawerTargetDisplay) {
            return { ...w, activeOn: null }
          }
          if (w.id === selectedWp.id) {
            return { ...w, activeOn: drawerTargetDisplay }
          }
          return w
        })
      )
    }

    showSnackbar(
      `⚡ [${drawerTargetDisplay}] 已设为当前壁纸：${selectedWp.name} (${
        params.fit
      }, ${params.scale.toFixed(2)}x, 滤镜: ${params.filterPreset})`
    )

    if (followWallpaper) {
      // Seed from the daemon's idea of the current wallpaper when we can map
      // it back to a library item (the daemon reports the resolved path, which
      // is what the scan produced); otherwise fall back to the item we applied.
      const fromDaemon = await currentWallpaperPath()
      const match = fromDaemon
        ? localList.find((w) => w.path === fromDaemon) ?? selectedWp
        : selectedWp
      const seed = pickSeedColor(match.colors)
      if (seed) setThemeSeed(seed)
    }

    setDrawerOpen(false)
  }

  const handleSaveToLocal = async (wp: WallpaperItem) => {
    showSnackbar(`⏳ 正在下载 Wallhaven 4K 原图「${wp.name}」...`)
    try {
      const ext = wp.file_type === 'image/png' ? '.png' : '.jpg'
      const filename =
        wp.name.endsWith('.png') ||
        wp.name.endsWith('.jpg') ||
        wp.name.endsWith('.webp')
          ? wp.name
          : `${wp.name}${ext}`
      const savedPath = await invoke<string>('download_wallhaven_wallpaper', {
        fullUrl: wp.full_url || wp.thumb,
        filename,
      })
      showSnackbar(`💾 已成功保存到本地库：${savedPath}`)
      await refreshLocalWallpapers()
    } catch (e) {
      showSnackbar(`保存到本地失败: ${e}`)
    }
    setDrawerOpen(false)
  }

  const handleAddToPlaylist = (wp: WallpaperItem) => {
    showSnackbar(`➕ 已将「${wp.name}」加入 [${drawerTargetDisplay}] 轮播队列`)
    setDrawerOpen(false)
  }

  const handleTabChange = (tab: NavTab) => {
    setCurrentTab(tab)
  }

  const handleSourceChange = (src: 'wallhaven' | 'local') => {
    setCurrentSource(src)
  }

  return (
    <ThemeProvider theme={theme}>
    <CssBaseline />
    <Box sx={{ display: 'flex', height: '100vh', overflow: 'hidden', bgcolor: 'md.surface_container' }}>
      {/* 左侧导航栏 */}
      <Rail
        currentTab={currentTab}
        onTabChange={handleTabChange}
        isDark={isDark}
        onToggleTheme={handleToggleTheme}
      />

      {/* 右侧工作区 */}
      <Box
        component="main"
        className="main-workspace"
        sx={{ my: 1, mr: 1, borderRadius: 4, overflow: 'hidden', bgcolor: 'md.surface' }}
      >
        {currentTab === 'wallpapers' && (
          <WallpaperView
            wallhavenList={wallhavenList}
            localList={localList}
            currentSource={currentSource}
            currentDisplay={DISPLAYS[displayIdx]}
            enableBlur={enableBlur}
            lazyLoad={lazyLoad}
            onSourceChange={handleSourceChange}
            onCycleDisplay={handleCycleDisplay}
            onSelectWallpaper={handleSelectWallpaper}
            onNotify={showSnackbar}
            onSearchWallhaven={handleSearchWallhaven}
            onRefreshLocal={refreshLocalWallpapers}
            whLoading={whLoading}
            whTotal={whTotal}
          />
        )}

        {currentTab === 'transitions' && (
          <TransitionsView onNotify={showSnackbar} />
        )}

        {currentTab === 'parallax' && (
          <ParallaxView onNotify={showSnackbar} />
        )}

        {currentTab === 'settings' && (
          <SettingsView
            followWallpaper={followWallpaper}
            onToggleFollowWallpaper={handleToggleFollowWallpaper}
            seedColor={themeSeed}
            enableBlur={enableBlur}
            onToggleEnableBlur={handleToggleBlur}
            lazyLoad={lazyLoad}
            onToggleLazyLoad={handleToggleLazyLoad}
            colorScheme={colorScheme}
            onColorSchemeChange={handleColorSchemeChange}
            paletteError={paletteError}
            adaptNoctalia={adaptNoctalia}
            onToggleAdaptNoctalia={handleToggleAdaptNoctalia}
            externalPalette={externalPalette}
            syncSystemTheme={syncSystemTheme}
            onToggleSyncSystemTheme={handleToggleSyncSystemTheme}
            onNotify={showSnackbar}
          />
        )}
      </Box>

          {/* 右侧滑出抽屉 */}
          <WallpaperDrawer
            isOpen={drawerOpen}
            wallpaper={selectedWp}
            source={selectedSource}
            targetDisplay={drawerTargetDisplay}
            onClose={() => setDrawerOpen(false)}
            onSetTargetDisplay={setDrawerTargetDisplay}
            onApply={handleApplyWallpaper}
            onSaveToLocal={handleSaveToLocal}
            onAddToPlaylist={handleAddToPlaylist}
            onSearchTag={(tag) => {
              setCurrentTab('wallpapers')
              setCurrentSource('wallhaven')
              showSnackbar(`按标签「${tag}」搜索`)
            }}
          />

      <Snackbar
        key={snackbar?.key}
        open={snackbar !== null}
        autoHideDuration={4000}
        onClose={(_, reason) => reason !== 'clickaway' && setSnackbar(null)}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
      >
        <Alert
          severity={snackbar?.failed ? 'error' : 'info'}
          variant="filled"
          onClose={() => setSnackbar(null)}
          sx={{ alignItems: 'center', maxWidth: 560 }}
        >
          {snackbar?.message}
        </Alert>
      </Snackbar>
    </Box>
    </ThemeProvider>
  )
}
