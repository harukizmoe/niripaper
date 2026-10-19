import React, { useState } from 'react'

interface ParallaxViewProps {
  onNotify: (msg: string) => void
}

// 模拟 4 个独立的垂直工作区（§4.4），每个工作区均包含横向平铺列以及列内垂直平铺窗口（tiles）
const SIMULATED_WORKSPACES = [
  {
    id: 1,
    name: '工作区 1 · 日常开发',
    tag: '主屏任务',
    columns: [
      { id: 1, windows: ['Kitty Terminal'] },
      { id: 2, windows: ['Zen Browser', 'DevTools 检查器'] }, // 2 扇垂直平铺窗口！
      { id: 3, windows: ['VS Code: niripaper'] },
      { id: 4, windows: ['Dolphin 文件管理', '图片预览'] }, // 2 扇垂直平铺窗口！
      { id: 5, windows: ['Matrix / 聊天'] },
      { id: 6, windows: ['git status'] },
    ],
  },
  {
    id: 2,
    name: '工作区 2 · 系统编译',
    tag: '编译监控',
    columns: [
      { id: 1, windows: ['btop++ 资源监控', 'journalctl -f 日志'] }, // 2 扇垂直平铺窗口！
      { id: 2, windows: ['Cargo Watch 实时编译'] },
      { id: 3, windows: ['niri msg 事件流', 'glxgears 测试'] }, // 2 扇垂直平铺窗口！
      { id: 4, windows: ['Rust Analyzer 诊断'] },
      { id: 5, windows: ['Git Graph 提交树'] },
      { id: 6, windows: ['GDB / DAP 调试器'] },
    ],
  },
  {
    id: 3,
    name: '工作区 3 · 影音媒体',
    tag: '媒体娱乐',
    columns: [
      { id: 1, windows: ['Spotify 播放器'] },
      { id: 2, windows: ['mpv 4K 原画', '动态歌词滚动'] }, // 2 扇垂直平铺窗口！
      { id: 3, windows: ['Discord 社区'] },
      { id: 4, windows: ['OBS Studio', '混音台'] }, // 2 扇垂直平铺窗口！
      { id: 5, windows: ['Steam'] },
      { id: 6, windows: ['Web Radio'] },
    ],
  },
  {
    id: 4,
    name: '工作区 4 · 知识文献',
    tag: '学术笔记',
    columns: [
      { id: 1, windows: ['Zathura PDF 论文'] },
      { id: 2, windows: ['Obsidian 笔记', '双链图谱'] }, // 2 扇垂直平铺窗口！
      { id: 3, windows: ['Rust 文档 (cargo doc)'] },
      { id: 4, windows: ['Niri 官方 Wiki'] },
      { id: 5, windows: ['LaTeX 渲染器', '公式速查'] }, // 2 扇垂直平铺窗口！
      { id: 6, windows: ['待办清单'] },
    ],
  },
]

export const ParallaxView: React.FC<ParallaxViewProps> = ({ onNotify }) => {
  // Parallax specification parameters (§4)
  const [scale, setScale] = useState(1.1)
  const [columnSpan, setColumnSpan] = useState(6)
  const [workspaceSpan, setWorkspaceSpan] = useState(6)
  const [duration, setDuration] = useState(600)
  const [curve, setCurve] = useState('ease-out-cubic')
  const [followNiri, setFollowNiri] = useState(true)
  const [overviewZoom, setOverviewZoom] = useState(0.96)
  const [slowdown, setSlowdown] = useState(1.0)
  const [isOff, setIsOff] = useState(false)

  // 2D 布局交互仿真状态
  // 1. 垂直工作区索引 (0 .. 3)
  const [activeWs, setActiveWs] = useState(0)
  // 2. 当前横向焦点列 (0 .. columnSpan - 1)
  // 核心约束 (§4.4 规则 8)：垂直切换工作区时，横向焦点与位移必须严格保持冻结锁定（纯轴向运动），绝对不能发生横向抖动或斜向漂移！
  const [activeCol, setActiveCol] = useState(1)
  // 3. 总览开合状态 (§2.19)
  const [isOverviewOpen, setIsOverviewOpen] = useState(false)

  // --------------------------------------------------------------------------
  // 数学计算：横向与纵向双轴视差位移与几何尺寸 (§4.1 / §4.2.1 / §4.4)
  // --------------------------------------------------------------------------
  // 横向进度 (§4.2.1: progress = activeCol / (column_span - 1))
  const hProgress = activeCol / Math.max(1, columnSpan - 1)
  const screenWidth = 2560
  const overflowX = screenWidth * (scale - 1)
  // §4.3: x = overflow * (0.5 - progress)
  const offsetPxX = overflowX * (0.5 - hProgress)

  // 纵向进度 (§4.4: v_progress = activeIndex / (workspace_span - 1))
  const vProgress = activeWs / Math.max(1, workspaceSpan - 1)
  const screenHeight = 1440
  const overflowY = screenHeight * (scale - 1)
  // §4.3: y = overflow * (0.5 - progress)
  const offsetPxY = overflowY * (0.5 - vProgress)

  // 总览倍率与总放大倍数
  const currentZoom = isOverviewOpen ? overviewZoom : 1.0
  const totalScale = scale * currentZoom

  // 壁纸背景 2D 平移百分比
  // 垂直切换工作区时，hProgress 保持原值，因此 bgTranslateX 绝对静止，仅 bgTranslateY 移动（纯垂直轴运动）！
  const bgTranslateX = (0.5 - hProgress) * ((scale - 1) / scale) * 100
  const bgTranslateY = (0.5 - vProgress) * ((scale - 1) / scale) * 100

  // 切换窗口列（横向视差）
  const handleColSelect = (colIdx: number) => {
    setActiveCol(colIdx)
    onNotify(
      `↔ 聚焦平铺列 ${colIdx + 1} / ${columnSpan}（横向视差位移 ${
        offsetPxX >= 0 ? `+${offsetPxX.toFixed(1)}` : offsetPxX.toFixed(1)
      } px，纵向保持锁定）`
    )
  }

  // 切换工作区（纵向视差 · 上下切换）
  // 注意：此处不改变 activeCol，严格执行 §4.4 规则 8 的纯轴向运动原则
  const handleWorkspaceSelect = (wsIdx: number) => {
    const targetWs = Math.max(0, Math.min(SIMULATED_WORKSPACES.length - 1, wsIdx))
    setActiveWs(targetWs)
    onNotify(
      `↕ 垂直切换至工作区 ${targetWs + 1}「${
        SIMULATED_WORKSPACES[targetWs].name
      }」（纵向位移 ${
        offsetPxY >= 0 ? `+${offsetPxY.toFixed(1)}` : offsetPxY.toFixed(1)
      } px，横向严格锁定不动）`
    )
  }

  // 上切工作区
  const handleWorkspaceUp = () => {
    if (activeWs > 0) {
      handleWorkspaceSelect(activeWs - 1)
    } else {
      onNotify('已位于最顶部工作区')
    }
  }

  // 下切工作区
  const handleWorkspaceDown = () => {
    if (activeWs < SIMULATED_WORKSPACES.length - 1) {
      handleWorkspaceSelect(activeWs + 1)
    } else {
      onNotify('已位于最底部工作区')
    }
  }

  // 总览切换
  const handleToggleOverview = () => {
    setIsOverviewOpen((prev) => {
      const next = !prev
      onNotify(
        next
          ? '🔭 打开 Niri 工作区总览（触发画布 0.96 景深后撤）'
          : '🔭 关闭 Niri 总览（画布平滑回弹复原）'
      )
      return next
    })
  }

  // 综合视差动效试演：自动轮播横向切列 + 垂直切工作区
  const handleTestParallax = () => {
    const nextCol = (activeCol + 1) % columnSpan
    if (nextCol === 0) {
      const nextWs = (activeWs + 1) % SIMULATED_WORKSPACES.length
      handleWorkspaceSelect(nextWs)
    } else {
      handleColSelect(nextCol)
    }
  }

  return (
    <div className="view-page active" id="view-parallax">
      {/* 顶栏 */}
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
            <span>平铺桌面视差与缩放 (Parallax & Motion)</span>
            <span
              className="wh-quick-tag"
              style={{
                fontSize: 11,
                fontWeight: 800,
                background: 'rgba(214, 248, 88, 0.25)',
                color: '#4d6006',
              }}
            >
              Niri 核心联动 §4
            </span>
          </h2>
          <p
            style={{
              fontSize: 12,
              color: 'var(--m3-fg-muted)',
              marginTop: 2,
            }}
          >
            基于 Niri 窗口平铺与工作区滚动的布局反应式视差。画布超额放大裁剪，根据窗口焦点列平滑位移；总览时自动缩放后撤。
          </p>
        </div>

        <button
          className="wh-search-btn"
          style={{ height: 38 }}
          onClick={handleTestParallax}
        >
          ▶ 视差动效试演
        </button>
      </header>

      {/* 页面主体内容 */}
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
        {/* ================================================================
            模块一：交互式 Niri 平铺视差沙盒（支持 2D 垂直多工作区堆叠滚动 + 列内垂直平铺窗口）
            ================================================================ */}
        <div
          className="wallhaven-control-box"
          style={{
            margin: 0,
            background: 'var(--m3-surface)',
            borderRadius: 'var(--m3-radius-xl)',
            overflow: 'hidden',
            padding: 24,
          }}
        >
          {/* 沙盒顶栏控制 */}
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: 16,
              flexWrap: 'wrap',
              gap: 12,
            }}
          >
            <div>
              <div style={{ fontWeight: 800, fontSize: 15 }}>
                实时视差与多工作区纵横仿真沙盒
              </div>
              <div
                style={{
                  fontSize: 12,
                  color: 'var(--m3-fg-muted)',
                  marginTop: 2,
                }}
              >
                工作区在垂直方向堆叠成轴；列内支持多窗口上下垂直平铺。切工作区时横向严格锁定，纯纵向平移。
              </div>
            </div>

            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              {/* 上切工作区 */}
              <button
                className="wh-toggle-pill"
                style={{ height: 36, padding: '0 14px', fontWeight: 800 }}
                onClick={handleWorkspaceUp}
                disabled={activeWs === 0}
              >
                ▲ 上切工作区
              </button>

              {/* 下切工作区 */}
              <button
                className="wh-toggle-pill"
                style={{ height: 36, padding: '0 14px', fontWeight: 800 }}
                onClick={handleWorkspaceDown}
                disabled={activeWs === SIMULATED_WORKSPACES.length - 1}
              >
                ▼ 下切工作区
              </button>

              {/* 总览开关 */}
              <button
                className={`wh-toggle-pill ${isOverviewOpen ? 'active' : ''}`}
                style={{ height: 36, padding: '0 16px', fontWeight: 800 }}
                onClick={handleToggleOverview}
              >
                {isOverviewOpen ? '🔭 总览已开 (0.96x)' : '🔭 打开工作区总览'}
              </button>
            </div>
          </div>

          {/* 仿真屏幕视口 (扩大至 340px，充分展现垂直多工作区层级) */}
          <div
            style={{
              width: '100%',
              height: 340,
              borderRadius: 'var(--m3-radius-lg)',
              overflow: 'hidden',
              position: 'relative',
              background: '#09080e',
              border: '1.5px solid var(--m3-outline-variant)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            {/* 底层：超额放大并随横向 + 纵向 2D 双轴视差平移的壁纸 */}
            <div
              style={{
                position: 'absolute',
                inset: '-30%',
                backgroundImage:
                  'url("https://images.unsplash.com/photo-1519681393784-d120267933ba?w=1600&auto=format&fit=crop&q=80")',
                backgroundSize: 'cover',
                backgroundPosition: 'center',
                transform: `translate(${bgTranslateX.toFixed(
                  2
                )}%, ${bgTranslateY.toFixed(2)}%) scale(${totalScale.toFixed(3)})`,
                transition: isOff
                  ? 'none'
                  : `transform ${
                      (duration * slowdown) / 1000
                    }s cubic-bezier(0.2, 0, 0, 1)`,
                willChange: 'transform',
                filter: isOverviewOpen ? 'brightness(0.7)' : 'none',
              }}
            />

            {/* 中间层：垂直多工作区滚动卷轴容器 (Workspace Reel) */}
            <div
              style={{
                position: 'relative',
                zIndex: 10,
                width: '84%',
                height: '82%',
                overflow: 'hidden',
                borderRadius: 'var(--m3-radius-md)',
              }}
            >
              <div
                style={{
                  width: '100%',
                  height: '100%',
                  display: 'flex',
                  flexDirection: 'column',
                  transform: `translateY(-${activeWs * 100}%) scale(${
                    isOverviewOpen ? 0.88 : 1
                  })`,
                  transition: isOff
                    ? 'none'
                    : `transform ${
                        (duration * slowdown) / 1000
                      }s cubic-bezier(0.2, 0, 0, 1)`,
                }}
              >
                {SIMULATED_WORKSPACES.map((ws, wsIdx) => {
                  const isCurrentWs = wsIdx === activeWs
                  return (
                    <div
                      key={ws.id}
                      style={{
                        width: '100%',
                        height: '100%',
                        flexShrink: 0,
                        display: 'flex',
                        gap: 12,
                        padding: '6px 4px',
                        boxSizing: 'border-box',
                      }}
                    >
                      {ws.columns.slice(0, columnSpan).map((col, colIdx) => {
                        const isColActive = isCurrentWs && colIdx === activeCol
                        return (
                          <div
                            key={col.id}
                            onClick={() => handleColSelect(colIdx)}
                            style={{
                              flex: 1,
                              display: 'flex',
                              flexDirection: 'column',
                              gap: 6, // 列内垂直平铺窗口之间的间距
                              cursor: 'pointer',
                            }}
                          >
                            {/* 列内窗口 (支持 1 扇全高窗口，或 2 扇上下垂直平铺窗口) */}
                            {col.windows.map((winName, winIdx) => (
                              <div
                                key={winIdx}
                                style={{
                                  flex: 1,
                                  borderRadius: 'var(--m3-radius-md)',
                                  background: isColActive
                                    ? 'rgba(45, 38, 65, 0.85)'
                                    : 'rgba(25, 20, 38, 0.75)',
                                  border: isColActive
                                    ? '2px solid var(--m3-accent-lime)'
                                    : '1px solid rgba(255, 255, 255, 0.15)',
                                  boxShadow: isColActive
                                    ? '0 8px 20px rgba(0, 0, 0, 0.45)'
                                    : 'none',
                                  display: 'flex',
                                  flexDirection: 'column',
                                  padding: '8px 10px',
                                  transition: 'all 0.2s ease',
                                  userSelect: 'none',
                                }}
                              >
                                <div
                                  style={{
                                    display: 'flex',
                                    justifyContent: 'space-between',
                                    alignItems: 'center',
                                  }}
                                >
                                  <span
                                    style={{
                                      fontSize: 10,
                                      fontWeight: 800,
                                      color: isColActive
                                        ? 'var(--m3-accent-lime)'
                                        : 'rgba(255, 255, 255, 0.7)',
                                    }}
                                  >
                                    列 {colIdx + 1}
                                    {col.windows.length > 1 &&
                                      ` · 窗 ${winIdx + 1}`}
                                  </span>
                                  {isColActive && winIdx === 0 && (
                                    <span
                                      style={{
                                        fontSize: 9,
                                        padding: '1px 5px',
                                        borderRadius: 9999,
                                        background: 'var(--m3-accent-lime)',
                                        color: '#1c192b',
                                        fontWeight: 900,
                                      }}
                                    >
                                      焦点
                                    </span>
                                  )}
                                </div>

                                <div
                                  style={{
                                    marginTop: 'auto',
                                    fontSize: 10,
                                    color: 'rgba(255, 255, 255, 0.55)',
                                    fontFamily: 'var(--font-mono)',
                                    overflow: 'hidden',
                                    textOverflow: 'ellipsis',
                                    whiteSpace: 'nowrap',
                                  }}
                                >
                                  {winName}
                                </div>
                              </div>
                            ))}
                          </div>
                        )
                      })}
                    </div>
                  )
                })}
              </div>
            </div>

            {/* 右侧悬浮：Niri 风格垂直工作区切换条 (Vertical Workspace Strip) */}
            <div
              style={{
                position: 'absolute',
                right: 14,
                top: '50%',
                transform: 'translateY(-50%)',
                zIndex: 20,
                display: 'flex',
                flexDirection: 'column',
                gap: 8,
                background: 'rgba(18, 14, 28, 0.92)',
                padding: '10px 8px',
                borderRadius: 'var(--m3-radius-lg)',
                border: '1px solid rgba(255, 255, 255, 0.12)',
              }}
            >
              <div
                style={{
                  fontSize: 9,
                  fontWeight: 800,
                  color: 'rgba(255, 255, 255, 0.5)',
                  textAlign: 'center',
                  marginBottom: 2,
                  letterSpacing: 0.5,
                }}
              >
                工作区
              </div>
              {SIMULATED_WORKSPACES.map((ws, idx) => {
                const isWsActive = idx === activeWs
                return (
                  <button
                    key={ws.id}
                    onClick={() => handleWorkspaceSelect(idx)}
                    style={{
                      height: 32,
                      padding: '0 10px',
                      borderRadius: 'var(--m3-radius-md)',
                      border: isWsActive
                        ? '1.5px solid var(--m3-accent-lime)'
                        : '1px solid transparent',
                      background: isWsActive
                        ? 'rgba(214, 248, 88, 0.25)'
                        : 'rgba(255, 255, 255, 0.06)',
                      color: isWsActive ? 'var(--m3-accent-lime)' : '#fff',
                      fontSize: 11,
                      fontWeight: 800,
                      cursor: 'pointer',
                      display: 'flex',
                      alignItems: 'center',
                      gap: 6,
                      transition: 'all 0.18s ease',
                      whiteSpace: 'nowrap',
                    }}
                  >
                    <span
                      style={{
                        width: 6,
                        height: 6,
                        borderRadius: '50%',
                        background: isWsActive
                          ? 'var(--m3-accent-lime)'
                          : 'rgba(255,255,255,0.4)',
                      }}
                    />
                    <span>WS {ws.id}</span>
                  </button>
                )
              })}
            </div>

            {/* 底部悬浮双轴遥测数据 */}
            <div
              style={{
                position: 'absolute',
                bottom: 12,
                left: 14,
                display: 'flex',
                gap: 8,
                zIndex: 20,
                flexWrap: 'wrap',
              }}
            >
              {/* 横向视差参数 */}
              <span
                style={{
                  fontFamily: 'var(--font-mono)',
                  fontSize: 11,
                  background: 'rgba(18, 14, 28, 0.88)',
                  color: '#fff',
                  padding: '4px 10px',
                  borderRadius: 'var(--m3-radius-full)',
                  border: '1px solid rgba(255,255,255,0.1)',
                }}
              >
                ↔ 横向: <strong>h={hProgress.toFixed(2)}</strong> (
                {offsetPxX >= 0
                  ? `+${offsetPxX.toFixed(1)}`
                  : offsetPxX.toFixed(1)}{' '}
                px) · 列 {activeCol + 1}/{columnSpan}
              </span>

              {/* 纵向视差参数 */}
              <span
                style={{
                  fontFamily: 'var(--font-mono)',
                  fontSize: 11,
                  background: 'rgba(18, 14, 28, 0.88)',
                  color: 'var(--m3-accent-lime)',
                  padding: '4px 10px',
                  borderRadius: 'var(--m3-radius-full)',
                  border: '1px solid rgba(214, 248, 88, 0.3)',
                }}
              >
                ↕ 纵向: <strong>v={vProgress.toFixed(2)}</strong> (
                {offsetPxY >= 0
                  ? `+${offsetPxY.toFixed(1)}`
                  : offsetPxY.toFixed(1)}{' '}
                px) · 工作区 {activeWs + 1}/{workspaceSpan}
              </span>

              {/* 缩放参数 */}
              <span
                style={{
                  fontFamily: 'var(--font-mono)',
                  fontSize: 11,
                  background: 'rgba(18, 14, 28, 0.88)',
                  color: 'var(--m3-accent-lavender)',
                  padding: '4px 10px',
                  borderRadius: 'var(--m3-radius-full)',
                  border: '1px solid rgba(255,255,255,0.1)',
                }}
              >
                🔍 scale: <strong>{scale.toFixed(2)}x</strong> · zoom:{' '}
                <strong>{currentZoom.toFixed(2)}x</strong>
              </span>
            </div>
          </div>
        </div>

        {/* ================================================================
            模块二：视差跨度与画布缩放 (Spans & Scale · §4.1)
            ================================================================ */}
        <div
          className="wallhaven-control-box"
          style={{ margin: 0, padding: 22 }}
        >
          <div style={{ fontWeight: 800, fontSize: 15, marginBottom: 4 }}>
            视差跨度与画布放大 (§4.1 / §4.2.1 / §4.4)
          </div>
          <div
            style={{
              fontSize: 12,
              color: 'var(--m3-fg-muted)',
              marginBottom: 16,
            }}
          >
            画布按比例超额放大裁剪，以容纳横向与竖向平移行程。两轴跨度独立设定，彼此互不干扰。
          </div>

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(3, 1fr)',
              gap: 16,
            }}
          >
            {/* scale */}
            <div className="m3-slider-tile">
              <div className="m3-tile-header">
                <span className="m3-tile-label">🔍 画布缩放倍率 (scale)</span>
                <div className="m3-num-badge">
                  <input
                    type="number"
                    min="1.0"
                    max="1.35"
                    step="0.01"
                    value={scale.toFixed(2)}
                    onChange={(e) =>
                      setScale(parseFloat(e.target.value) || 1.1)
                    }
                  />
                  <span className="m3-num-badge-unit">x</span>
                </div>
              </div>
              <input
                type="range"
                className="m3-slider"
                min="1.0"
                max="1.35"
                step="0.01"
                value={scale}
                onChange={(e) => setScale(parseFloat(e.target.value))}
              />
              <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                默认 1.10x，规格上限 1.35x。
              </span>
            </div>

            {/* column_span */}
            <div className="m3-slider-tile">
              <div className="m3-tile-header">
                <span className="m3-tile-label">📑 列视差跨度 (column_span)</span>
                <div className="m3-num-badge">
                  <input
                    type="number"
                    min="2"
                    max="12"
                    value={columnSpan}
                    onChange={(e) =>
                      setColumnSpan(parseInt(e.target.value) || 6)
                    }
                  />
                  <span className="m3-num-badge-unit">列</span>
                </div>
              </div>
              <input
                type="range"
                className="m3-slider"
                min="2"
                max="12"
                value={columnSpan}
                onChange={(e) => setColumnSpan(parseInt(e.target.value))}
              />
              <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                固定列分母（默认 6），窗口增删不影响视差步长。
              </span>
            </div>

            {/* workspace_span */}
            <div className="m3-slider-tile">
              <div className="m3-tile-header">
                <span className="m3-tile-label">
                  🖥️ 工作区跨度 (workspace_span)
                </span>
                <div className="m3-num-badge">
                  <input
                    type="number"
                    min="2"
                    max="12"
                    value={workspaceSpan}
                    onChange={(e) =>
                      setWorkspaceSpan(parseInt(e.target.value) || 6)
                    }
                  />
                  <span className="m3-num-badge-unit">屏</span>
                </div>
              </div>
              <input
                type="range"
                className="m3-slider"
                min="2"
                max="12"
                value={workspaceSpan}
                onChange={(e) => setWorkspaceSpan(parseInt(e.target.value))}
              />
              <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                垂直工作区滑动分母（默认 6），与横向列完全独立。
              </span>
            </div>
          </div>
        </div>

        {/* ================================================================
            模块三：视差跟随动画与曲线 (§4.2.6)
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
              marginBottom: 16,
            }}
          >
            <div>
              <div style={{ fontWeight: 800, fontSize: 15 }}>
                视差跟随动画与曲线 (§4.2.6)
              </div>
              <div
                style={{
                  fontSize: 12,
                  color: 'var(--m3-fg-muted)',
                  marginTop: 2,
                }}
              >
                窗口焦点变动时壁纸跟随滑动的缓动参数。支持跟随 Niri 配置或独立覆盖。
              </div>
            </div>

            <button
              className={`wh-toggle-pill ${followNiri ? 'active' : ''}`}
              style={{ height: 36, padding: '0 16px', fontWeight: 800 }}
              onClick={() => {
                setFollowNiri((v) => !v)
                onNotify(
                  !followNiri
                    ? '已开启自动跟随 Niri 动画配置'
                    : '已切换为独立覆盖参数'
                )
              }}
            >
              {followNiri ? '🔗 自动跟随 Niri animations { }' : '⚙️ 独立参数配置'}
            </button>
          </div>

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: '1.2fr 1fr',
              gap: 16,
            }}
          >
            {/* duration_ms */}
            <div className="m3-slider-tile">
              <div className="m3-tile-header">
                <span className="m3-tile-label">⏱️ 视差跟随动画时长 (duration_ms)</span>
                <div className="m3-num-badge">
                  <input
                    type="number"
                    min="100"
                    max="1500"
                    step="50"
                    value={duration}
                    onChange={(e) =>
                      setDuration(parseInt(e.target.value) || 600)
                    }
                  />
                  <span className="m3-num-badge-unit">ms</span>
                </div>
              </div>
              <input
                type="range"
                className="m3-slider"
                min="100"
                max="1500"
                step="50"
                value={duration}
                onChange={(e) => setDuration(parseInt(e.target.value))}
              />
              <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                对标 Niri animations.parallax：默认 600 ms。
              </span>
            </div>

            {/* curve */}
            <div className="m3-slider-tile">
              <span className="m3-tile-label" style={{ marginBottom: 6 }}>
                📈 缓动曲线类型 (curve)
              </span>
              <div className="m3-select-wrapper">
                <select
                  className="m3-select"
                  value={curve}
                  onChange={(e) => setCurve(e.target.value)}
                >
                  <option value="ease-out-cubic">ease-out-cubic (推荐默认)</option>
                  <option value="ease-out-quad">ease-out-quad</option>
                  <option value="ease-out-expo">ease-out-expo</option>
                  <option value="linear">linear</option>
                  <option value="cubic-bezier">cubic-bezier</option>
                </select>
                <div className="m3-select-arrow">▼</div>
              </div>
            </div>
          </div>
        </div>

        {/* ================================================================
            模块四：工作区总览拉远与弹簧物理 (§2.19)
            ================================================================ */}
        <div
          className="wallhaven-control-box"
          style={{ margin: 0, padding: 22 }}
        >
          <div style={{ fontWeight: 800, fontSize: 15, marginBottom: 4 }}>
            工作区总览拉远过渡 (Overview Zoom · §2.19)
          </div>
          <div
            style={{
              fontSize: 12,
              color: 'var(--m3-fg-muted)',
              marginBottom: 16,
            }}
          >
            在 Niri 开启概览/总览模式时，画布后撤形成拉远景深层次感，采用标准弹簧物理模拟。
          </div>

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(2, 1fr)',
              gap: 16,
            }}
          >
            {/* overview_zoom */}
            <div className="m3-slider-tile">
              <div className="m3-tile-header">
                <span className="m3-tile-label">🔭 总览画布后撤缩放 (zoom)</span>
                <div className="m3-num-badge">
                  <input
                    type="number"
                    min="0.80"
                    max="1.00"
                    step="0.01"
                    value={overviewZoom.toFixed(2)}
                    onChange={(e) =>
                      setOverviewZoom(parseFloat(e.target.value) || 0.96)
                    }
                  />
                  <span className="m3-num-badge-unit">x</span>
                </div>
              </div>
              <input
                type="range"
                className="m3-slider"
                min="0.80"
                max="1.00"
                step="0.01"
                value={overviewZoom}
                onChange={(e) => setOverviewZoom(parseFloat(e.target.value))}
              />
              <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                默认 0.96x。小于 1.0 时向后拉远画布。
              </span>
            </div>

            {/* global slowdown & off */}
            <div className="m3-slider-tile">
              <div className="m3-tile-header">
                <span className="m3-tile-label">⏳ 时间膨胀系数 (slowdown)</span>
                <div className="m3-num-badge">
                  <input
                    type="number"
                    min="0.1"
                    max="5.0"
                    step="0.1"
                    value={slowdown.toFixed(1)}
                    onChange={(e) =>
                      setSlowdown(parseFloat(e.target.value) || 1.0)
                    }
                  />
                  <span className="m3-num-badge-unit">x</span>
                </div>
              </div>
              <input
                type="range"
                className="m3-slider"
                min="0.1"
                max="5.0"
                step="0.1"
                value={slowdown}
                onChange={(e) => setSlowdown(parseFloat(e.target.value))}
              />
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  marginTop: 6,
                }}
              >
                <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                  慢动作调参：大于 1.0 延长时间
                </span>
                <button
                  className={`wh-toggle-pill ${isOff ? 'active' : ''}`}
                  style={{ height: 26, fontSize: 11, padding: '0 10px' }}
                  onClick={() => setIsOff((v) => !v)}
                >
                  {isOff ? '⚡ 零过渡跳跃已启用' : '正常平滑过渡'}
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
