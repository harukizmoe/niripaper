import React from 'react'

interface SettingsViewProps {
  followWallpaper: boolean
  onToggleFollowWallpaper: () => void
  seedColor: string
}

export const SettingsView: React.FC<SettingsViewProps> = ({
  followWallpaper,
  onToggleFollowWallpaper,
  seedColor,
}) => {
  return (
    <div className="view-page active" id="view-settings">
      <header
        className="wallpaper-top-bar"
        style={{ borderBottom: '1px solid var(--m3-outline-variant)' }}
      >
        <div>
          <h2 style={{ fontSize: 18, fontWeight: 800 }}>系统与动画设置</h2>
          <p style={{ fontSize: 12, color: 'var(--m3-fg-muted)' }}>
            全局配置文件：~/.config/niripaper/config.toml
          </p>
        </div>
      </header>

      <div
        style={{
          padding: 28,
          display: 'flex',
          flexDirection: 'column',
          gap: 20,
        }}
      >
        {/* 卡片 1：守护进程状态 */}
        <div className="wallhaven-control-box" style={{ margin: 0 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <div className="pulse-dot"></div>
            <div style={{ fontWeight: 800, fontSize: 15 }}>daemon 53023 运行中</div>
            <span
              className="wh-quick-tag"
              style={{
                marginLeft: 'auto',
                background: 'rgba(120, 220, 119, 0.25)',
                color: '#2e7d32',
                fontWeight: 800,
              }}
            >
              NVIDIA · DP-1 渲染中
            </span>
          </div>
          <div
            style={{
              fontSize: 12,
              color: 'var(--m3-fg-muted)',
              marginTop: 6,
            }}
          >
            守护进程通过 Unix Domain Socket 提供 IPC 通讯，负责显卡硬件解码与图层渲染。
          </div>
        </div>

        {/* 卡片 2：动态配色与主题取色 */}
        <div className="wallhaven-control-box" style={{ margin: 0 }}>
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
        </div>

        {/* 卡片 3：跟随与渲染控制 */}
        <div className="wallhaven-control-box" style={{ margin: 0 }}>
          <div style={{ fontWeight: 800, fontSize: 15 }}>跟随与渲染控制</div>
          <div
            style={{
              fontSize: 12,
              color: 'var(--m3-fg-muted)',
              marginTop: 4,
            }}
          >
            视差动画已跟随 Niri 工作区配置；窗口完全遮挡时自动暂停 GPU 渲染。
          </div>
        </div>
      </div>
    </div>
  )
}
