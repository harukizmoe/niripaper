import React from 'react'

export type LocalFilter = 'all' | 'static' | 'video' | 'dark' | 'light'

interface LocalControlsProps {
  currentFilter: LocalFilter
  count: number
  onFilterChange: (filter: LocalFilter) => void
}

export const LocalControls: React.FC<LocalControlsProps> = ({
  currentFilter,
  count,
  onFilterChange,
}) => {
  return (
    <div
      style={{
        display: 'flex',
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
    </div>
  )
}
