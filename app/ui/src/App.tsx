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
  applyThemeTokens,
  pickSeedColor,
  THEME_SEED_FALLBACK,
} from './theme/materialYou'

const DISPLAYS = ['DP-1 (主屏)', 'eDP-1 (内屏)', '全部显示器 (*)']

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
  const [localList, setLocalList] = useState<WallpaperItem[]>(initialLocalWallpapers)
  const [wallhavenList, setWallhavenList] = useState<WallpaperItem[]>(whWallpapers)
  const [whLoading, setWhLoading] = useState(false)
  const [whTotal, setWhTotal] = useState(1800)

  // 1. 扫描并加载本地真实壁纸 (~/Pictures/Wallpapers)
  const refreshLocalWallpapers = useCallback(async () => {
    try {
      const items = await invoke<WallpaperItem[]>('load_local_wallpapers')
      if (items && items.length > 0) {
        setLocalList(items)
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

  // Theme seed
  const [themeSeed, setThemeSeed] = useState<string>(() => {
    const active = initialLocalWallpapers.find((w) => w.activeOn) || whWallpapers[0]
    return pickSeedColor(active.colors)
  })


  // Apply theme tokens whenever themeSeed or isDark changes
  useEffect(() => {
    document.documentElement.classList.toggle('dark', isDark)
    applyThemeTokens(themeSeed, isDark)
  }, [themeSeed, isDark])

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
      const seed = pickSeedColor(selectedWp.colors)
      setThemeSeed(seed)
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
