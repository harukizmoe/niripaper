import React from 'react'

export type NavTab = 'wallpapers' | 'transitions' | 'settings'

interface RailProps {
  currentTab: NavTab
  onTabChange: (tab: NavTab) => void
  isDark: boolean
  onToggleTheme: () => void
}

export const Rail: React.FC<RailProps> = ({
  currentTab,
  onTabChange,
  isDark,
  onToggleTheme,
}) => {
  return (
    <aside className="m3-rail">
      <div className="rail-brand">
        <div className="logo-diamond">NIRI</div>
        <span className="brand-title">niripaper</span>
      </div>

      <nav className="rail-menu">
        <button
          className={`rail-pill ${currentTab === 'wallpapers' ? 'active' : ''}`}
          onClick={() => onTabChange('wallpapers')}
        >
          <svg viewBox="0 0 24 24" fill="currentColor">
            <path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 16H5V5h14v14zm-5.04-6.71l-2.75 3.54-1.96-2.36L6.5 17h11l-3.54-4.71z" />
          </svg>
          <span>壁纸库</span>
        </button>

        <button
          className={`rail-pill ${currentTab === 'transitions' ? 'active' : ''}`}
          onClick={() => onTabChange('transitions')}
        >
          <svg viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2l2.4 7.2h7.6l-6 4.8 2.4 7.2-6.4-4.8-6.4 4.8 2.4-7.2-6-4.8h7.6z" />
          </svg>
          <span>过渡动画</span>
        </button>

        <button
          className={`rail-pill ${currentTab === 'settings' ? 'active' : ''}`}
          onClick={() => onTabChange('settings')}
        >
          <svg viewBox="0 0 24 24" fill="currentColor">
            <path d="M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.05.3-.09.63-.09.94s.02.64.07.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z" />
          </svg>
          <span>系统设置</span>
        </button>
      </nav>

      {/* 底部：主题日夜间切换按钮 */}
      <div style={{ marginTop: 'auto' }}>
        <button
          className="rail-pill"
          style={{
            width: '100%',
            justifyContent: 'center',
            background: 'rgba(255, 255, 255, 0.08)',
            color: '#fff',
            gap: 8,
          }}
          onClick={onToggleTheme}
        >
          <span style={{ fontSize: 15 }}>{isDark ? '☀️' : '🌙'}</span>
          <span>{isDark ? '浅色模式' : '夜间深色'}</span>
        </button>
      </div>
    </aside>
  )
}
