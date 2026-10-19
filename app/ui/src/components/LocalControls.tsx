import React from 'react'

export type LocalFilter = 'all' | 'static' | 'video' | 'dark' | 'light'

interface LocalControlsProps {
  currentFilter: LocalFilter
  count: number
  onFilterChange: (filter: LocalFilter) => void
  onRefresh?: () => void
}

export const LocalControls: React.FC<LocalControlsProps> = ({
  currentFilter,
  count,
  onFilterChange,
  onRefresh,
}) => {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        padding: '0 28px 12px 28px',
        gap: 8,
        userSelect: 'none',
      }}
    >
      <button
        className={`wh-toggle-pill ${currentFilter === 'all' ? 'active' : ''}`}
        onClick={() => onFilterChange('all')}
      >
        全部本地 ({count})
      </button>
      <button
        className={`wh-toggle-pill ${currentFilter === 'static' ? 'active' : ''}`}
        onClick={() => onFilterChange('static')}
      >
        静态图片
      </button>
      <button
        className={`wh-toggle-pill ${currentFilter === 'video' ? 'active' : ''}`}
        onClick={() => onFilterChange('video')}
      >
        动态视频
      </button>
      <button
        className={`wh-toggle-pill ${currentFilter === 'dark' ? 'active' : ''}`}
        onClick={() => onFilterChange('dark')}
      >
        暗色系
      </button>
      <button
        className={`wh-toggle-pill ${currentFilter === 'light' ? 'active' : ''}`}
        onClick={() => onFilterChange('light')}
      >
        亮色系
      </button>

      {onRefresh && (
        <button
          className="wh-toggle-pill"
          style={{
            marginLeft: 'auto',
            background: 'var(--m3-surface)',
            border: '1px solid var(--m3-outline-variant)',
          }}
          onClick={onRefresh}
          title="重新扫描 ~/Pictures/Wallpapers"
        >
          🔄 重新扫描本地库
        </button>
      )}
    </div>
  )
}
