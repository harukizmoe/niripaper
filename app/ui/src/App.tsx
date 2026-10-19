import { useState, useEffect, useCallback, useRef } from 'react'
import { Rail, type NavTab } from './components/Rail'
import { WallpaperView } from './components/WallpaperView'
import { TransitionsView } from './components/TransitionsView'
import { SettingsView } from './components/SettingsView'
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
  const [currentSource, setCurrentSource] = useState<'wallhaven' | 'local'>('wallhaven')
  const [displayIdx, setDisplayIdx] = useState(0)
  const [isDark, setIsDark] = useState(false)
  const [followWallpaper, setFollowWallpaper] = useState(true)

  const [localList, setLocalList] = useState<WallpaperItem[]>(initialLocalWallpapers)
  const [wallhavenList] = useState<WallpaperItem[]>(whWallpapers)

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

  const handleApplyWallpaper = (params: { fit: string; scale: number; filterPreset: string }) => {
    if (!selectedWp) return

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
      `⚡ [${drawerTargetDisplay}] 已设为当前壁纸：${selectedWp.name} (${params.fit}, ${params.scale.toFixed(
        2
      )}x, 滤镜: ${params.filterPreset})`
    )

    if (followWallpaper) {
      const seed = pickSeedColor(selectedWp.colors)
      setThemeSeed(seed)
    }

    setDrawerOpen(false)
  }

  const handleSaveToLocal = (wp: WallpaperItem) => {
    showSnackbar(`💾 已成功保存到本地：~/Pictures/Wallpapers/${wp.name}`)
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
        tab === 'wallpapers' ? '壁纸库' : tab === 'transitions' ? '过渡动画' : '系统设置'
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
            onSourceChange={handleSourceChange}
            onCycleDisplay={handleCycleDisplay}
            onSelectWallpaper={handleSelectWallpaper}
            onNotify={showSnackbar}
          />
        )}

        {currentTab === 'transitions' && (
          <TransitionsView onNotify={showSnackbar} />
        )}

        {currentTab === 'settings' && (
          <SettingsView
            followWallpaper={followWallpaper}
            onToggleFollowWallpaper={handleToggleFollowWallpaper}
            seedColor={themeSeed}
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
