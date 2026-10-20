import React, { useState, useEffect, useCallback } from 'react'
import { invoke } from '../hooks/useDaemon'
import {
  SCHEME_LABELS,
  SCHEME_NAMES,
  DEFAULT_SCHEME,
  type SchemeName,
} from '../theme/schemes'

interface SettingsViewProps {
  followWallpaper: boolean
  onToggleFollowWallpaper: () => void
  seedColor: string
  enableBlur?: boolean
  onToggleEnableBlur?: () => void
  lazyLoad?: boolean
  onToggleLazyLoad?: () => void
  colorScheme?: SchemeName
  onColorSchemeChange?: (next: SchemeName) => void
  onNotify: (msg: string) => void
}

interface DaemonStatusInfo {
  running: boolean
  socket_path: string
  output_info?: string
}

export const SettingsView: React.FC<SettingsViewProps> = ({
  followWallpaper,
  onToggleFollowWallpaper,
  seedColor,
  enableBlur = true,
  onToggleEnableBlur,
  lazyLoad = true,
  onToggleLazyLoad,
  colorScheme = DEFAULT_SCHEME,
  onColorSchemeChange,
  onNotify,
}) => {
  const [daemonRunning, setDaemonRunning] = useState(false)
  const [socketPath, setSocketPath] = useState('')
  const [outputInfo, setOutputInfo] = useState<string | null>(null)
  const [autostartEnabled, setAutostartEnabled] = useState(false)
  const [parallaxEnabled, setParallaxEnabled] = useState(true)
  const [busy, setBusy] = useState(false)
  const checkDaemon = useCallback(async () => {
    try {
      const info = await invoke<DaemonStatusInfo>('daemon_status_info')
      setDaemonRunning(info.running)
      setSocketPath(info.socket_path)
      setOutputInfo(info.output_info || null)
    } catch {
      setDaemonRunning(false)
    }
  }, [])

  const checkAutostart = useCallback(async () => {
    try {
      const enabled = await invoke<boolean>('get_autostart_status')
      setAutostartEnabled(enabled)
    } catch {
      setAutostartEnabled(false)
    }
  }, [])
  const checkParallax = useCallback(async () => {
    try {
      const cfg = await invoke<{ enabled: boolean }>('get_parallax_config')
      setParallaxEnabled(cfg.enabled)
    } catch {
      setParallaxEnabled(true)
    }
  }, [])

  useEffect(() => {
    void checkDaemon()
    void checkAutostart()
    void checkParallax()
  }, [checkDaemon, checkAutostart, checkParallax])
  const handleStartDaemon = async () => {
    setBusy(true)
    try {
      const msg = await invoke<string>('daemon_start')
      onNotify(msg)
      await checkDaemon()
    } catch (e) {
      onNotify(`启动失败: ${e}`)
    } finally {
      setBusy(false)
    }
  }

  const handleStopDaemon = async () => {
    setBusy(true)
    try {
      await invoke('daemon_kill')
      onNotify('⏹ 壁纸引擎已停止，桌面已无缝回退为系统原生 Noctalia 壁纸')
      await checkDaemon()
    } catch (e) {
      onNotify(`停止守护进程失败: ${e}`)
    } finally {
      setBusy(false)
    }
  }

  const handleToggleAutostart = async () => {
    const next = !autostartEnabled
    try {
      const res = await invoke<string>('set_autostart_enabled', {
        enabled: next,
      })
      setAutostartEnabled(next)
      onNotify(res)
    } catch (e) {
      onNotify(`自启动配置失败: ${e}`)
    }
  }

  return (
    <div className="view-page active" id="view-settings">
      {/* 顶栏 */}
      <header
        className="wallpaper-top-bar"
        style={{ borderBottom: '1px solid var(--m3-outline-variant)' }}
      >
        <div>
          <h2 style={{ fontSize: 18, fontWeight: 800 }}>系统设置与引擎管理</h2>
          <p style={{ fontSize: 12, color: 'var(--m3-fg-muted)' }}>
            配置文件：~/.config/niripaper/config.toml
          </p>
        </div>
      </header>

      {/* 设置卡片容器 */}
      <div
        style={{
          padding: 28,
          display: 'flex',
          flexDirection: 'column',
          gap: 20,
        }}
      >
        {/* ================================================================
            卡片 1：守护进程生命周期管理 (Daemon Lifecycle Management)
            ================================================================ */}
        <div
          className="wallhaven-control-box"
          style={{
            margin: 0,
            padding: 22,
            border: daemonRunning
              ? '1.5px solid rgba(120, 220, 119, 0.4)'
              : '1.5px solid var(--m3-outline-variant)',
          }}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: 12,
              flexWrap: 'wrap',
              gap: 12,
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <div
                style={{
                  width: 10,
                  height: 10,
                  borderRadius: '50%',
                  background: daemonRunning
                    ? 'var(--m3-accent-mint)'
                    : 'var(--m3-outline)',
                  boxShadow: daemonRunning
                    ? '0 0 10px var(--m3-accent-mint)'
                    : 'none',
                  transition: 'all 0.3s ease',
                }}
              />
              <div>
                <div style={{ fontWeight: 800, fontSize: 15 }}>
                  {daemonRunning ? '壁纸引擎运行中 (niripaper)' : '壁纸引擎已停止'}
                </div>
                <div
                  style={{
                    fontSize: 11,
                    color: 'var(--m3-fg-muted)',
                    fontFamily: 'var(--font-mono)',
                    marginTop: 2,
                  }}
                >
                  控制 Socket: {socketPath || '未连接'}
                </div>
              </div>
            </div>

            <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
              <button
                className="wh-clear-btn"
                style={{ height: 36, padding: '0 14px' }}
                onClick={() => {
                  void checkDaemon()
                  onNotify('已刷新守护进程状态')
                }}
                disabled={busy}
              >
                🔄 刷新
              </button>

              {daemonRunning ? (
                <button
                  className="wh-clear-btn"
                  style={{
                    height: 38,
                    padding: '0 18px',
                    fontWeight: 800,
                    background: 'rgba(247, 168, 162, 0.3)',
                    color: '#b91c1c',
                  }}
                  onClick={handleStopDaemon}
                  disabled={busy}
                >
                  ⏹ 停止引擎 (回退 Noctalia)
                </button>
              ) : (
                <button
                  className="m3-act-btn"
                  style={{
                    height: 38,
                    padding: '0 20px',
                    fontWeight: 900,
                    background: 'var(--m3-accent-lime)',
                    color: '#1c192b',
                    boxShadow: '0 4px 14px rgba(214, 248, 88, 0.4)',
                  }}
                  onClick={handleStartDaemon}
                  disabled={busy}
                >
                  ▶ 启动壁纸引擎
                </button>
              )}
            </div>
          </div>

          <div
            style={{
              fontSize: 12,
              color: 'var(--m3-fg)',
              lineHeight: 1.6,
              background: 'var(--m3-surface-subtle)',
              padding: '12px 16px',
              borderRadius: 'var(--m3-radius-md)',
            }}
          >
            {daemonRunning ? (
              <>
                <p>
                  ✅ <strong>当前状态：</strong>
                  守护进程正常运行，已在 Niri 背景层（Background
                  Layer）覆盖渲染壁纸。
                  {outputInfo && (
                    <span style={{ color: 'var(--m3-accent-purple)' }}>
                      {' '}
                      [{outputInfo}]
                    </span>
                  )}
                </p>
                <p style={{ color: 'var(--m3-fg-muted)', marginTop: 4 }}>
                  💡 <strong>资源解耦提示：</strong>
                  关闭本 GUI 桌面客户端不会终止壁纸引擎！轻量级 Rust
                  守护进程（占用极小内存与零待机 CPU）将在后台独立持续运行；若需要完全退出壁纸并退回
                  Noctalia，点击上方的「停止引擎」即可。
                </p>
              </>
            ) : (
              <>
                <p>
                  ⏸️ <strong>当前状态：</strong>
                  守护进程未运行，系统桌面当前由 Noctalia 原生壁纸图层接管呈现。
                </p>
                <p style={{ color: 'var(--m3-fg-muted)', marginTop: 4 }}>
                  点击右上方「启动壁纸引擎」，niripaper
                  将在后台启动并无缝覆盖到桌面背景层，提供视差滑动与过渡动画。
                </p>
              </>
            )}
          </div>
        </div>

        {/* ================================================================
            卡片 2：开机自启动配置 (Autostart Settings)
            ================================================================ */}
        <div
          className="wallhaven-control-box"
          style={{ margin: 0, padding: 22 }}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: 10,
              flexWrap: 'wrap',
              gap: 12,
            }}
          >
            <div>
              <div style={{ fontWeight: 800, fontSize: 15 }}>
                开机静默自启动 (Systemd & XDG Autostart)
              </div>
              <div
                style={{
                  fontSize: 12,
                  color: 'var(--m3-fg-muted)',
                  marginTop: 2,
                }}
              >
                开启后，系统开机登录时将自动启动后台壁纸引擎（零 GUI
                显存占用），无需手动打开管理端。
              </div>
            </div>

            <button
              className={`wh-toggle-pill ${autostartEnabled ? 'active' : ''}`}
              style={{
                height: 38,
                padding: '0 18px',
                fontWeight: 800,
                fontSize: 13,
              }}
              onClick={handleToggleAutostart}
            >
              {autostartEnabled
                ? '☑ 开机自启动已开启'
                : '☐ 开机自启动未开启'}
            </button>
          </div>

          <div
            style={{
              fontSize: 11,
              fontFamily: 'var(--font-mono)',
              color: 'var(--m3-fg-muted)',
              borderTop: '1px solid var(--m3-outline-variant)',
              paddingTop: 10,
            }}
          >
            服务配置路径: ~/.config/systemd/user/niripaper.service 及
            ~/.config/autostart/niripaper.desktop
          </div>
        </div>

        {/* ================================================================
            卡片 3：Material You 动态配色 (Dynamic Theme)
            ================================================================ */}
        <div
          className="wallhaven-control-box"
          style={{ margin: 0, padding: 22 }}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <div>
              <div style={{ fontWeight: 800, fontSize: 15 }}>
                Material You 动态壁纸取色
              </div>
              <div
                style={{
                  fontSize: 12,
                  color: 'var(--m3-fg-muted)',
                  marginTop: 4,
                }}
              >
                自动从当前生效壁纸中提取主色调，基于 OKLCH 算法动态推导界面的 M3 配色。
              </div>
            </div>
            <button
              className={`wh-toggle-pill ${followWallpaper ? 'active' : ''}`}
              style={{ height: 38, padding: '0 18px', fontWeight: 800 }}
              onClick={onToggleFollowWallpaper}
            >
              {followWallpaper ? '🎨 已开启跟随壁纸取色' : '🎨 固定默认配色'}
            </button>
          </div>

          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 10,
              paddingTop: 12,
              borderTop: '1px solid var(--m3-outline-variant)',
              fontSize: 12,
              fontFamily: 'var(--font-mono)',
            }}
          >
            <span
              style={{
                width: 14,
                height: 14,
                borderRadius: '50%',
                background: seedColor,
                border: '2px solid rgba(255, 255, 255, 0.4)',
                boxShadow: '0 2px 6px rgba(0,0,0,0.15)',
              }}
            />
            <span style={{ fontWeight: 700 }}>
              当前取色种子: {seedColor.toUpperCase()}
            </span>
          </div>

          {/* 取色方案：决定同一个种子被如何派生为一整套配色 */}
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              gap: 12,
              flexWrap: 'wrap',
              paddingTop: 12,
              borderTop: '1px solid var(--m3-outline-variant)',
            }}
          >
            <div>
              <div style={{ fontSize: 13, fontWeight: 800 }}>壁纸取色方案</div>
              <div
                style={{
                  fontSize: 12,
                  color: 'var(--m3-fg-muted)',
                  marginTop: 2,
                  maxWidth: 460,
                }}
              >
                决定同一个取色种子派生出的整套配色风格。只影响本应用，不改变系统主题。
              </div>
            </div>
            <div className="m3-select-wrapper" style={{ minWidth: 240 }}>
              <select
                className="m3-select"
                value={colorScheme}
                onChange={(e) =>
                  onColorSchemeChange?.(e.target.value as SchemeName)
                }
              >
                {SCHEME_NAMES.map((name) => (
                  <option key={name} value={name}>
                    {SCHEME_LABELS[name]}
                  </option>
                ))}
              </select>
              <div className="m3-select-arrow">▼</div>
            </div>
          </div>
        </div>

        {/* ================================================================
            卡片 4：跟随与渲染控制
            ================================================================ */}
        {/* 卡片 4：跟随与渲染控制 */}
        <div
          className="wallhaven-control-box"
          style={{ margin: 0, padding: 22 }}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: 10,
              flexWrap: 'wrap',
              gap: 12,
            }}
          >
            <div>
              <div style={{ fontWeight: 800, fontSize: 15 }}>
                平铺桌面视差效果开关 (§4)
              </div>
              <div
                style={{
                  fontSize: 12,
                  color: 'var(--m3-fg-muted)',
                  marginTop: 2,
                }}
              >
                控制画布是否超额放大并随窗口焦点列与工作区滑动产生位移。
              </div>
            </div>

            <button
              className={`wh-toggle-pill ${parallaxEnabled ? 'active' : ''}`}
              style={{ height: 38, padding: '0 18px', fontWeight: 800 }}
              onClick={async () => {
                const next = !parallaxEnabled
                setParallaxEnabled(next)
                try {
                  const cfg = await invoke<{ enabled: boolean }>(
                    'set_parallax_enabled',
                    { enabled: next }
                  )
                  setParallaxEnabled(cfg.enabled)
                  onNotify(
                    cfg.enabled
                      ? '⚡ 平铺视差已开启（已写入 config.toml，守护进程即时生效）'
                      : '⏸ 平铺视差已关闭（已将 scale 设为 1.00x，守护进程即时生效）'
                  )
                } catch (e) {
                  onNotify(`更新配置文件失败: ${e}`)
                }
              }}
            >
              {parallaxEnabled ? '⚡ 桌面视差：已启用' : '⏸ 桌面视差：已停用'}
            </button>
          </div>
          <div
            style={{
              fontSize: 12,
              color: 'var(--m3-fg-muted)',
              marginTop: 4,
              lineHeight: 1.5,
              borderTop: '1px solid var(--m3-outline-variant)',
              paddingTop: 10,
            }}
          >
            视差动画已跟随 Niri 工作区与平铺布局配置；关闭视差时壁纸以 1:1 原画居中静止呈现；当窗口全屏完全遮挡背景时，守护进程自动暂停 GPU 渲染回调，待机功耗为 0%。
          </div>
        </div>

        {/* ================================================================
            卡片 5：UI 视觉滤镜与景深虚化控制 (Visual Effects & Blur)
            ================================================================ */}
        <div
          className="wallhaven-control-box"
          style={{ margin: 0, padding: 22 }}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              flexWrap: 'wrap',
              gap: 12,
            }}
          >
            <div>
              <div style={{ fontWeight: 800, fontSize: 15 }}>
                壁纸悬停景深微模糊 (Depth Blur on Hover)
              </div>
              <div
                style={{
                  fontSize: 12,
                  color: 'var(--m3-fg-muted)',
                  marginTop: 2,
                }}
              >
                当光标悬停某张壁纸时，使背景其余壁纸产生轻微景深虚化 (`blur(3.5px)`) 以聚焦突出。
              </div>
            </div>

            {onToggleEnableBlur && (
              <button
                className={`wh-toggle-pill ${enableBlur ? 'active' : ''}`}
                style={{ height: 38, padding: '0 18px', fontWeight: 800 }}
                onClick={onToggleEnableBlur}
              >
                {enableBlur ? '✨ 景深模糊：已开启' : '⚡ 景深模糊：已关闭 (极致流畅)'}
              </button>
            )}
          </div>

          <div
            style={{
              fontSize: 12,
              color: 'var(--m3-fg-muted)',
              marginTop: 10,
              lineHeight: 1.5,
              borderTop: '1px solid var(--m3-outline-variant)',
              paddingTop: 10,
            }}
          >
            {enableBlur
              ? '当前已开启景深聚焦模糊。若在低配核显或老旧设备上滑动时感知掉帧，可随时在此关闭，停用后悬停只保留 3D 微翘高光，彻底免除 GPU 滤镜运算。'
              : '当前已彻底禁用 CSS 模糊滤镜，所有卡片保持零模糊开销，达到最极致高刷滑动性能。'}
          </div>
        </div>

        {/* ================================================================
            卡片 6：壁纸卡片懒加载与入场动画 (Lazy Loading)
            ================================================================ */}
        <div
          className="wallhaven-control-box"
          style={{ margin: 0, padding: 22 }}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              flexWrap: 'wrap',
              gap: 12,
            }}
          >
            <div>
              <div style={{ fontWeight: 800, fontSize: 15 }}>
                壁纸卡片懒加载与入场动画 (Lazy Load)
              </div>
              <div
                style={{
                  fontSize: 12,
                  color: 'var(--m3-fg-muted)',
                  marginTop: 2,
                }}
              >
                卡片滚动到视口附近才请求预览图，到达时以淡入 + 轻微上浮的波浪式动画呈现。
              </div>
            </div>

            {onToggleLazyLoad && (
              <button
                className={`wh-toggle-pill ${lazyLoad ? 'active' : ''}`}
                style={{ height: 38, padding: '0 18px', fontWeight: 800 }}
                onClick={onToggleLazyLoad}
              >
                {lazyLoad ? '⚡ 懒加载：已开启' : '🖼 懒加载：已关闭'}
              </button>
            )}
          </div>

          <div
            style={{
              fontSize: 12,
              color: 'var(--m3-fg-muted)',
              marginTop: 10,
              lineHeight: 1.5,
              borderTop: '1px solid var(--m3-outline-variant)',
              paddingTop: 10,
            }}
          >
            {lazyLoad
              ? '当前按需加载：视口外约一屏范围内的卡片提前预取，更远的保持骨架占位。首屏卡片会依列顺序错峰入场，翻页或切换筛选时同样生效。'
              : '当前一次性加载全部可见卡片，不做延迟请求、不播放入场动画 —— 适合希望内容瞬时全部出现的场景。'}
          </div>
        </div>
      </div>
    </div>
  )
}
