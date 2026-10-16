import React from 'react'

import { ThemeToggle } from './ThemeToggle'

interface SettingsViewProps {
  isDark: boolean
  onToggleTheme: () => void
}

export const SettingsView: React.FC<SettingsViewProps> = ({ isDark, onToggleTheme }) => {
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
        <ThemeToggle isDark={isDark} onToggle={onToggleTheme} />
      </header>

      <div style={{ padding: 28 }}>
        <div className="wallhaven-control-box" style={{ margin: 0 }}>
          <div style={{ fontWeight: 800, fontSize: 14 }}>跟随与渲染控制</div>
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
