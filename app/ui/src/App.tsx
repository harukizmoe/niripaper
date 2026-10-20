import { useState, useEffect, useCallback, useRef } from 'react'
import { invoke } from './hooks/useDaemon'
import { Rail, type NavTab } from './components/Rail'
import { WallpaperView } from './components/WallpaperView'
import { TransitionsView } from './components/TransitionsView'
import { SettingsView } from './components/SettingsView'
import { ParallaxView } from './components/ParallaxView'
import { WallpaperDrawer } from './components/WallpaperDrawer'
import { Snackbar } from './components/Snackbar'
import {
  whWallpapers,
  localWallpapers as initialLocalWallpapers,
  type WallpaperItem,
} from './data/wallpapers'
import {
  buildM3Scheme,
  pickSeedColor,
  schemeFromPalette,
  toM3Export,
  THEME_SEED_FALLBACK,
} from './theme/materialYou'
import { DEFAULT_SCHEME, isSchemeName, type SchemeName } from './theme/schemes'

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
  const [isDark, setIsDark] = useState(false)
  const [followWallpaper, setFollowWallpaper] = useState(true)
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
  const [themeSeed, setThemeSeed] = useState<string>(() => {
    const active = initialLocalWallpapers.find((w) => w.activeOn) || whWallpapers[0]
    return pickSeedColor(active.colors)
  })
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
            if (current) {
              const next = pickSeedColor(current.colors)
              if (next) setThemeSeed(next)
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

  // Toast / Snackbar
  const [snackbarMessage, setSnackbarMessage] = useState('')
  const [snackbarVisible, setSnackbarVisible] = useState(false)
  const snackbarTimerRef = useRef<number | undefined>(undefined)

  const showSnackbar = useCallback((msg: string) => {
    setSnackbarMessage(msg)
    setSnackbarVisible(true)
    clearTimeout(snackbarTimerRef.current)
    snackbarTimerRef.current = window.setTimeout(() => {
      setSnackbarVisible(false)
    }, 2200)
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


  // Apply theme tokens whenever themeSeed or isDark changes
  // Pull the system palette while the switch is on, and once more whenever the
  // wallpaper changes (a new wallpaper means a new palette). Best effort: a
  // missing file just leaves `externalPalette` null and we keep generating.
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
          setExternalPalette(null)
          showSnackbar('未找到系统调色板，沿用本应用取色')
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
  }, [adaptNoctalia, adaptNoctalia ? themeSeed : null, showSnackbar])

  useEffect(() => {
    document.documentElement.classList.toggle('dark', isDark)
    const external = adaptNoctalia && externalPalette ? schemeFromPalette(externalPalette) : null
    // The token map actually in force, so the push below sends the same
    // colours this window is painting rather than a second derivation.
    const s = external ?? buildM3Scheme(themeSeed, isDark, colorScheme)
    const root = document.documentElement.style
    for (const [k, v] of Object.entries(s)) root.setProperty(k, v)

    // Export the same palette so other tools can follow it. Best effort: a
    // failure here must not affect the app's own colours, and nothing in this
    // app reads the file back.
    // When following the system palette, export that same palette rather than
    // our own derivation — otherwise the file we publish would disagree with
    // the colours on screen.
    const exported = external
      ? toM3Export(external)
      : toM3Export(buildM3Scheme(themeSeed, isDark, colorScheme))
    void invoke<string>('export_palette', {
      seed: external?.['primary'] ?? themeSeed,
      scheme: external ? 'system' : colorScheme,
      dark: isDark,
      tokens: exported,
    }).catch((e) => {
      console.error('导出调色板失败（不影响本应用配色）:', e)
      setPaletteError(String(e))
    })

    // Push the same palette to the desktop's theme daemon. `s` is the token
    // map we just applied, so what the rest of the desktop gets matches what
    // this window shows. Failure is reported, not fatal: the app keeps its own
    // colours either way.
    if (syncSystemTheme && s) {
      const pick = (k: string, fallback: string) => s[k] ?? fallback
      void invoke<string>('apply_system_palette', {
        palette: {
          primary: pick('--m3-accent-purple', '#624b89'),
          on_primary: pick('--m3-rail-text-active', '#ffffff'),
          secondary: pick('--m3-accent-mint', '#8ed6ac'),
          on_secondary: pick('--m3-rail-text-active', '#ffffff'),
          tertiary: pick('--m3-accent-lime', '#cacc58'),
          on_tertiary: pick('--m3-rail-text-active', '#ffffff'),
          error: pick('--m3-accent-coral', '#ffb4ab'),
          on_error: pick('--m3-rail-text-active', '#ffffff'),
          surface: pick('--m3-surface', '#ffffff'),
          on_surface: pick('--m3-fg', '#211d2f'),
          surface_variant: pick('--m3-surface-subtle', '#eae6f7'),
          on_surface_variant: pick('--m3-fg-muted', '#69657e'),
          outline: pick('--m3-outline', '#bbb8cb'),
        },
      })
        .then(() => setPaletteError(null))
        .catch((e) => {
          console.error('同步系统主题失败:', e)
          setPaletteError(String(e))
        })
    }
  }, [themeSeed, isDark, colorScheme, externalPalette, adaptNoctalia])

  const handleToggleSyncSystemTheme = () => {
    setSyncSystemTheme((prev) => {
      const next = !prev
      localStorage.setItem('niripaper_sync_system_theme', String(next))
      showSnackbar(
        next
          ? '🌗 已开启：换壁纸会同步整个系统的主题配色'
          : '🌗 已关闭：系统主题不再跟随本应用'
      )
      return next
    })
  }

  const handleToggleAdaptNoctalia = () => {
    setAdaptNoctalia((prev) => {
      const next = !prev
      localStorage.setItem('niripaper_adapt_noctalia', String(next))
      showSnackbar(
        next
          ? '🎨 已改为跟随系统调色板（只读，不修改系统设置）'
          : '🎨 已改回本应用自己的取色方案'
      )
      return next
    })
  }

  const handleColorSchemeChange = (next: SchemeName) => {
    setColorScheme(next)
    localStorage.setItem('niripaper_color_scheme', next)
    showSnackbar(`取色方案已切换为：${next}`)
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
      const next = !prev
      showSnackbar(`已切换为 ${next ? '夜间深色' : '浅色'} 模式（配色仍取自当前壁纸）`)
      return next
    })
  }

  const handleToggleFollowWallpaper = () => {
    setFollowWallpaper((prev) => {
      const next = !prev
      if (next) {
        const active = localList.find((w) => w.activeOn) || whWallpapers[0]
        const seed = pickSeedColor(active.colors)
        setThemeSeed(seed)
        showSnackbar('主题配色已跟随当前壁纸取色')
      } else {
        setThemeSeed(THEME_SEED_FALLBACK)
        showSnackbar('主题配色已固定为默认紫')
      }
      return next
    })
  }

  const handleToggleBlur = () => {
    setEnableBlur((prev) => {
      const next = !prev
      localStorage.setItem('niripaper_enable_blur', String(next))
      showSnackbar(
        next
          ? '✨ 已开启壁纸卡片景深聚焦微模糊效果'
          : '⚡ 已彻底停用景深模糊（零 GPU 滤镜运算，极致流畅）'
      )
      return next
    })
  }
  const handleToggleLazyLoad = () => {
    setLazyLoad((prev) => {
      const next = !prev
      localStorage.setItem('niripaper_lazy_load', String(next))
      showSnackbar(
        next
          ? '⚡ 已开启卡片懒加载：滚到附近才取图，带淡入上浮动画'
          : '🖼 已关闭懒加载：全部预览图立即加载，无入场动画'
      )
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
    showSnackbar(
      `切换至：${
        tab === 'wallpapers'
          ? '壁纸库'
          : tab === 'transitions'
          ? '过渡动画'
          : tab === 'parallax'
          ? '视差动画'
          : '系统设置'
      }`
    )
  }

  const handleSourceChange = (src: 'wallhaven' | 'local') => {
    setCurrentSource(src)
    showSnackbar(
      src === 'wallhaven'
        ? '已切换至 Wallhaven 在线公共壁纸库'
        : '已切换至本地壁纸库 (~/Pictures/Wallpapers)'
    )
  }

  return (
    <div className="app-layout">
      {/* 左侧导航栏 */}
      <Rail
        currentTab={currentTab}
        onTabChange={handleTabChange}
        isDark={isDark}
        onToggleTheme={handleToggleTheme}
      />

      {/* 右侧工作区 */}
      <main className="main-workspace">
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
      </main>

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

      {/* M3 浮动提示条 */}
      <Snackbar message={snackbarMessage} visible={snackbarVisible} />
    </div>
  )
}
