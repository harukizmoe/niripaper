import React, { useState } from 'react'
import { transitions, type TransitionItem } from '../data/wallpapers'
import { ThemeToggle } from './ThemeToggle'

interface TransitionsViewProps {
  isDark: boolean
  onToggleTheme: () => void
  onNotify: (msg: string) => void
}

export const TransitionsView: React.FC<TransitionsViewProps> = ({
  isDark,
  onToggleTheme,
  onNotify,
}) => {
  const [selectedId, setSelectedId] = useState<string>('portal')

  const handleSelect = (tr: TransitionItem) => {
    setSelectedId(tr.id)
    onNotify(`已选过渡效果：${tr.name}`)
  }

  return (
    <div className="view-page active" id="view-transitions">
      <header
        className="wallpaper-top-bar"
        style={{ borderBottom: '1px solid var(--m3-outline-variant)' }}
      >
        <div className="page-title-box">
          <h2
            style={{
              fontSize: 18,
              fontWeight: 800,
              display: 'flex',
              alignItems: 'center',
              gap: 10,
            }}
          >
            <span>换图过渡效果 (Transitions)</span>
            <span
              style={{
                fontSize: 11,
                fontWeight: 800,
                padding: '3px 10px',
                borderRadius: 9999,
                background: 'var(--m3-accent-lavender)',
                color: 'var(--m3-accent-purple)',
              }}
            >
              仅全局配置
            </span>
          </h2>
          <p
            style={{
              fontSize: 12,
              color: 'var(--m3-fg-muted)',
              marginTop: 2,
            }}
          >
            GPU 屏幕空间着色器混合。多显示器换图时全局生效，不按显示器划分。
          </p>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <button
            className="wh-search-btn"
            style={{ height: 38 }}
            onClick={() => onNotify('⚡ 触发全局换图过渡测试播演 (1200 ms)')}
          >
            ▶ 预览过渡
          </button>
          <ThemeToggle isDark={isDark} onToggle={onToggleTheme} />
        </div>
      </header>

      <div
        style={{
          flex: 1,
          overflowY: 'auto',
          padding: '24px 28px',
          display: 'flex',
          flexDirection: 'column',
          gap: 24,
        }}
      >
        <div
          style={{
            fontSize: 13,
            fontWeight: 800,
            color: 'var(--m3-fg-muted)',
            textTransform: 'uppercase',
          }}
        >
          9 款 Shader 转场效果
        </div>
        <div
          className="gallery-masonry"
          style={{ columnCount: 3 }}
          id="transition-grid"
        >
          {transitions.map((tr) => {
            const isSelected = tr.id === selectedId
            return (
              <div
                key={tr.id}
                className={`tr-card ${isSelected ? 'selected' : ''}`}
                style={{
                  background: 'var(--m3-surface)',
                  borderRadius: 'var(--m3-radius-lg)',
                  padding: 16,
                  marginBottom: 14,
                  breakInside: 'avoid',
                  cursor: 'pointer',
                  boxShadow: isSelected
                    ? '0 0 0 3px var(--m3-accent-purple), var(--m3-shadow-md)'
                    : 'var(--m3-shadow-sm)',
                  transition: 'all 0.2s cubic-bezier(0.2, 0, 0, 1)',
                }}
                onClick={() => handleSelect(tr)}
              >
                <div
                  style={{
                    fontWeight: 800,
                    fontSize: 14,
                    display: 'flex',
                    justifyContent: 'space-between',
                  }}
                >
                  <span>{tr.name}</span>
                  {isSelected && (
                    <span
                      style={{
                        fontSize: 11,
                        fontWeight: 800,
                        color: 'var(--m3-accent-purple)',
                      }}
                    >
                      生效中
                    </span>
                  )}
                </div>
                <div
                  style={{
                    fontSize: 12,
                    color: 'var(--m3-fg-muted)',
                    marginTop: 6,
                    lineHeight: 1.4,
                  }}
                >
                  {tr.desc}
                </div>
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}
