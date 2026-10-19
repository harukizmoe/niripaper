import React from 'react'

interface WallhavenControlsProps {
  searchQuery: string
  onSearchChange: (q: string) => void
  onSearchSubmit: () => void
  onQuickTag: (tag: string) => void
  onMatchScreen: () => void
  onResetFilters: () => void
  categoryAnime: boolean
  onToggleCategoryAnime: () => void
  categoryGeneral: boolean
  onToggleCategoryGeneral: () => void
  categoryPeople: boolean
  onToggleCategoryPeople: () => void
  puritySfw: boolean
  onTogglePuritySfw: () => void
  puritySketchy: boolean
  onTogglePuritySketchy: () => void
  sortOption: string
  onSortChange: (sort: string) => void
  loading?: boolean
}

export const WallhavenControls: React.FC<WallhavenControlsProps> = ({
  searchQuery,
  onSearchChange,
  onSearchSubmit,
  onQuickTag,
  onMatchScreen,
  onResetFilters,
  categoryAnime,
  onToggleCategoryAnime,
  categoryGeneral,
  onToggleCategoryGeneral,
  categoryPeople,
  onToggleCategoryPeople,
  puritySfw,
  onTogglePuritySfw,
  puritySketchy,
  onTogglePuritySketchy,
  sortOption,
  onSortChange,
  loading = false,
}) => {
  const quickTags = [
    'Cyberpunk',
    'Anime',
    'Landscape',
    'Pixel Art',
    'Fantasy',
    'Sci-Fi',
    'Minimalism',
  ]

  return (
    <div className="wallhaven-control-box" id="wh-control-panel">
      {/* 搜索框行 */}
      <div className="wh-search-row">
        <div className="wh-search-input-wrap">
          <span style={{ fontSize: 16, opacity: 0.6 }}>🔍</span>
          <input
            type="text"
            className="wh-search-input"
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') onSearchSubmit()
            }}
            placeholder="在 Wallhaven 数百万壁纸中搜索标签（如 anime, cyberpunk, landscape, night...）"
          />
          {searchQuery && (
            <span
              style={{ cursor: 'pointer', opacity: 0.5, fontSize: 13 }}
              onClick={() => {
                onSearchChange('')
                onResetFilters()
              }}
            >
              ✕
            </span>
          )}
        </div>
        <button
          className="wh-search-btn"
          onClick={onSearchSubmit}
          disabled={loading}
          style={{ opacity: loading ? 0.7 : 1 }}
        >
          <span>{loading ? '⏳ 检索中…' : '搜 索'}</span>
        </button>
      </div>

      {/* 热门标签快捷胶囊 */}
      <div className="wh-tags-quick-bar">
        <span
          style={{
            fontSize: 11,
            fontWeight: 700,
            color: 'var(--m3-fg-muted)',
            marginRight: 4,
          }}
        >
          热门灵感:
        </span>
        {quickTags.map((tag) => (
          <button
            key={tag}
            className="wh-quick-tag"
            onClick={() => onQuickTag(tag)}
          >
            #{tag}
          </button>
        ))}
      </div>

      {/* 筛选行：分类 / 内容 / 排序 / 智能匹配当前屏 */}
      <div className="wh-filter-row">
        <div className="wh-filter-group">
          <span className="wh-group-label">分类:</span>
          <button
            className={`wh-toggle-pill ${categoryGeneral ? 'active' : ''}`}
            onClick={onToggleCategoryGeneral}
          >
            综合
          </button>
          <button
            className={`wh-toggle-pill ${categoryAnime ? 'active' : ''}`}
            onClick={onToggleCategoryAnime}
          >
            动漫
          </button>
          <button
            className={`wh-toggle-pill ${categoryPeople ? 'active' : ''}`}
            onClick={onToggleCategoryPeople}
          >
            人物
          </button>
        </div>

        <div className="wh-filter-group">
          <span className="wh-group-label">分级:</span>
          <button
            className={`wh-toggle-pill wh-purity-pill ${
              puritySfw ? 'active-sfw' : ''
            }`}
            onClick={onTogglePuritySfw}
          >
            SFW (安全)
          </button>
          <button
            className={`wh-toggle-pill wh-purity-pill ${
              puritySketchy ? 'active-sketchy' : ''
            }`}
            onClick={onTogglePuritySketchy}
          >
            敏感 (Sketchy)
          </button>
        </div>

        <div className="wh-filter-group">
          <span className="wh-group-label">排序:</span>
          <select
            value={sortOption}
            onChange={(e) => onSortChange(e.target.value)}
            style={{
              background: 'var(--m3-surface-subtle)',
              border: 'none',
              padding: '6px 14px',
              borderRadius: 'var(--m3-radius-full)',
              fontSize: 12,
              fontWeight: 700,
              color: 'var(--m3-fg)',
              outline: 'none',
              cursor: 'pointer',
            }}
          >
            <option value="toplist">🔥 热门 Toplist</option>
            <option value="hot">⚡ 最火爆 Hot</option>
            <option value="random">🎲 随机探索</option>
            <option value="favorites">⭐ 收藏最多</option>
            <option value="views">👀 浏览最多</option>
            <option value="date_added">⏱️ 最新发布</option>
          </select>
        </div>

        {/* 智能屏幕匹配按钮 */}
        <div className="wh-filter-group">
          <button className="wh-match-screen-btn" onClick={onMatchScreen}>
            <span>🖥️ 匹配当前 DP-1 (16:9 · ≥2K)</span>
          </button>
          <button className="wh-clear-btn" onClick={onResetFilters}>
            重置
          </button>
        </div>
      </div>
    </div>
  )
}
