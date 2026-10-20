import React, { useState, useEffect } from 'react'
import { transitions, type TransitionItem } from '../data/wallpapers'
import { invoke } from '../hooks/useDaemon'

interface TransitionsViewProps {
  onNotify: (msg: string) => void
}

type SelectionMode = 'fixed' | 'rotate' | 'random'
type DirectionValue = 'left' | 'right' | 'up' | 'down'

interface TransitionConfig {
  selection: 'fixed' | 'rotate' | 'random'
  effect: string
  effects: string[]
  duration_ms: number
  curve: string
  cubic_bezier?: [number, number, number, number]
  allow_overshoot: boolean
  softness: number
  center_x: number
  center_y: number
  start_radius: number
  hold_ms: number
  stripes: number
  push: number
  direction: 'left' | 'right' | 'up' | 'down'
  on_start: boolean
}

export const TransitionsView: React.FC<TransitionsViewProps> = ({ onNotify }) => {
  // --------------------------------------------------------------------------
  // 全局策略与动力学基础参数 (Settings · §5)
  // --------------------------------------------------------------------------
  const [selection, setSelection] = useState<SelectionMode>('rotate')
  const [fixedEffect, setFixedEffect] = useState<string>('fade')
  const [poolEffects, setPoolEffects] = useState<string[]>([
    'portal',
    'iris',
    'dissolve',
  ])
  const [duration, setDuration] = useState<number>(1500)
  const [curve, setCurve] = useState<string>('ease-out-cubic')
  const [allowOvershoot, setAllowOvershoot] = useState<boolean>(false)
  const [onStart, setOnStart] = useState<boolean>(true)

  // --------------------------------------------------------------------------
  // 当前正在检视/调优的高级效果（选中画廊中任意卡片即可在下方专属面板调参）
  // --------------------------------------------------------------------------
  const [inspectEffect, setInspectEffect] = useState<string>('portal')

  // 专属效果几何与物理参数
  const [direction, setDirection] = useState<DirectionValue>('right')
  const [softness, setSoftness] = useState<number>(0.3)
  const [centerX, setCenterX] = useState<number>(50)
  const [centerY, setCenterY] = useState<number>(50)
  const [stripes, setStripes] = useState<number>(12)
  const [push, setPush] = useState<number>(0.55)
  const [startRadius, setStartRadius] = useState<number>(0.0)
  const [hold, setHold] = useState<number>(0)
  const [saving, setSaving] = useState(false)

  // 组件挂载时读取 config.toml 中的真实过渡配置
  useEffect(() => {
    void invoke<TransitionConfig>('get_transition_config')
      .then((cfg) => {
        setSelection(cfg.selection as SelectionMode)
        setFixedEffect(cfg.effect)
        if (cfg.effects && cfg.effects.length > 0) {
          setPoolEffects(cfg.effects)
        }
        setDuration(cfg.duration_ms)
        setCurve(cfg.curve)
        setAllowOvershoot(cfg.allow_overshoot)
        setSoftness(cfg.softness)
        setCenterX(Math.round(cfg.center_x * 100))
        setCenterY(Math.round(cfg.center_y * 100))
        setStartRadius(cfg.start_radius)
        setHold(cfg.hold_ms)
        setStripes(cfg.stripes)
        setPush(cfg.push)
        setDirection(cfg.direction as DirectionValue)
        setOnStart(cfg.on_start)
        setInspectEffect(
          cfg.selection === 'fixed'
            ? cfg.effect
            : cfg.effects[0] || 'portal'
        )
      })
      .catch((err) => {
        console.warn('获取过渡动画配置失败:', err)
      })
  }, [])

  // 保存并写回 config.toml
  const handleSaveConfig = async (override?: Partial<TransitionConfig>) => {
    setSaving(true)
    const payload: TransitionConfig = {
      selection,
      effect: fixedEffect,
      effects: poolEffects,
      duration_ms: duration,
      curve,
      cubic_bezier: [0.42, 0.0, 0.58, 1.0],
      allow_overshoot: allowOvershoot,
      softness,
      center_x: centerX / 100,
      center_y: centerY / 100,
      start_radius: startRadius,
      hold_ms: hold,
      stripes,
      push,
      direction,
      on_start: onStart,
      ...override,
    }

    try {
      const res = await invoke<string>('save_transition_config', { cfg: payload })
      onNotify(res)
    } catch (e) {
      onNotify(`保存过渡配置失败: ${e}`)
    } finally {
      setSaving(false)
    }
  }

  // 切换选中卡片
  const handleCardClick = (tr: TransitionItem) => {
    setInspectEffect(tr.id)
    if (selection === 'fixed') {
      setFixedEffect(tr.id)
      void handleSaveConfig({ effect: tr.id })
      onNotify(`已设为全局固定过渡效果并保存至配置：${tr.name}`)
    } else {
      onNotify(`已激活参数面板：${tr.name}`)
    }
  }

  // 轮播池勾选/移除
  const togglePoolEffect = (e: React.MouseEvent, id: string) => {
    e.stopPropagation()
    const exists = poolEffects.includes(id)
    if (exists && poolEffects.length <= 1) {
      onNotify('轮播池至少需保留 1 款效果')
      return
    }
    const updated = exists
      ? poolEffects.filter((item) => item !== id)
      : [...poolEffects, id]
    setPoolEffects(updated)
    void handleSaveConfig({ effects: updated })
    onNotify(exists ? `已从轮播池移除：${id}` : `已加入轮播候选池并保存：${id}`)
  }

  // 触发真实桌面预览动效
  const handlePreview = async () => {
    try {
      await handleSaveConfig()
      onNotify('⚡ 正在桌面上触发换图过渡动画...')
      await invoke<string>('trigger_test_transition')
      onNotify(`✨ 换图过渡动画已在桌面背景层成功播放！`)
    } catch (e) {
      onNotify(`触发过渡动画失败: ${e}`)
    }
  }
  const selectedInspectObj =
    transitions.find((t) => t.id === inspectEffect) || transitions[0]

  // 判断各组专属参数适用性
  const hasDirection = ['wipe', 'stripes', 'slide'].includes(inspectEffect)
  const hasSoftness = ['wipe', 'stripes', 'iris', 'portal'].includes(
    inspectEffect
  )
  const hasCenter = ['iris', 'portal', 'zoom'].includes(inspectEffect)
  const hasPortal = inspectEffect === 'portal'
  const hasStripes = inspectEffect === 'stripes'
  const hasIrisPortal = ['iris', 'portal'].includes(inspectEffect)
  const isPureColor = ['fade', 'dissolve', 'none'].includes(inspectEffect)

  return (
    <div className="view-page active" id="view-transitions">
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
            <span>换图过渡效果 (Transitions)</span>
            <span
              className="wh-quick-tag"
              style={{
                fontSize: 11,
                fontWeight: 800,
                background: 'var(--m3-accent-lavender)',
                color: 'var(--m3-accent-purple)',
              }}
            >
              仅全局配置 §5
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
            className="wh-toggle-pill active"
            style={{ height: 38, padding: '0 18px', fontWeight: 800 }}
            onClick={() => void handleSaveConfig()}
            disabled={saving}
          >
            {saving ? '⏳ 保存中...' : '💾 保存并应用配置'}
          </button>
          <button
            className="wh-search-btn"
            style={{ height: 38, background: 'var(--m3-primary)', color: '#fff' }}
            onClick={handlePreview}
          >
            ⚡ 在桌面上测试过渡动效
          </button>
        </div>
      </header>

      {/* 滚动容器 */}
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
            模块一：全局策略与基础动力学 (Global Strategy & Dynamics)
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
              flexWrap: 'wrap',
              gap: 12,
            }}
          >
            <div>
              <div style={{ fontWeight: 800, fontSize: 15 }}>
                全局过渡策略与动力学配置
              </div>
              <div
                style={{
                  fontSize: 12,
                  color: 'var(--m3-fg-muted)',
                  marginTop: 2,
                }}
              >
                决定每次换壁纸时的转场选择模式、基础时长以及平滑曲线。
              </div>
            </div>

            {/* 策略选择器 */}
            <div className="m3-seg-box">
              <button
                className={`m3-seg-btn ${
                  selection === 'fixed' ? 'active' : ''
                }`}
                style={{ padding: '0 16px' }}
                onClick={() => {
                  setSelection('fixed')
                  void handleSaveConfig({ selection: 'fixed' })
                  onNotify('已切换为：单一固定效果模式并保存至 config.toml')
                }}
              >
                ★ 单一固定 (Fixed)
              </button>
              <button
                className={`m3-seg-btn ${
                  selection === 'rotate' ? 'active' : ''
                }`}
                style={{ padding: '0 16px' }}
                onClick={() => {
                  setSelection('rotate')
                  void handleSaveConfig({ selection: 'rotate' })
                  onNotify('已切换为：候选池顺序轮播模式并保存至 config.toml')
                }}
              >
                🔄 顺序轮播 (Rotate)
              </button>
              <button
                className={`m3-seg-btn ${
                  selection === 'random' ? 'active' : ''
                }`}
                style={{ padding: '0 16px' }}
                onClick={() => {
                  setSelection('random')
                  void handleSaveConfig({ selection: 'random' })
                  onNotify('已切换为：候选池随机抽选模式并保存至 config.toml')
                }}
              >
                🎲 随机抽选 (Random)
              </button>
            </div>
          </div>

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: '1.2fr 1.2fr 1fr 1fr',
              gap: 16,
              alignItems: 'center',
            }}
          >
            {/* duration */}
            <div className="m3-slider-tile">
              <div className="m3-tile-header">
                <span className="m3-tile-label">⏱️ 过渡时长 (duration)</span>
                <div className="m3-num-badge">
                  <input
                    type="number"
                    min="100"
                    max="3000"
                    step="50"
                    value={duration}
                    onChange={(e) =>
                      setDuration(parseInt(e.target.value) || 1500)
                    }
                  />
                  <span className="m3-num-badge-unit">ms</span>
                </div>
              </div>
              <input
                type="range"
                className="m3-slider"
                min="100"
                max="3000"
                step="50"
                value={duration}
                onChange={(e) => setDuration(parseInt(e.target.value))}
              />
            </div>

            {/* curve */}
            <div className="m3-slider-tile">
              <span className="m3-tile-label" style={{ marginBottom: 6 }}>
                📈 插值缓动曲线 (curve)
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
                  <option value="linear">linear (线性)</option>
                  <option value="cubic-bezier">cubic-bezier</option>
                </select>
                <div className="m3-select-arrow">▼</div>
              </div>
            </div>

            {/* allow_overshoot */}
            <div
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 6,
                padding: '4px 8px',
              }}
            >
              <span style={{ fontSize: 11, fontWeight: 700, color: 'var(--m3-fg-muted)' }}>
                物理回弹 (Overshoot):
              </span>
              <button
                className={`wh-toggle-pill ${allowOvershoot ? 'active' : ''}`}
                style={{ height: 38, width: '100%', fontWeight: 700 }}
                onClick={() => setAllowOvershoot((v) => !v)}
              >
                {allowOvershoot ? '☑ 允许曲线反弹' : '☐ 限制 0..1 夹紧'}
              </button>
            </div>

            {/* on_start */}
            <div
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 6,
                padding: '4px 8px',
              }}
            >
              <span style={{ fontSize: 11, fontWeight: 700, color: 'var(--m3-fg-muted)' }}>
                开机启动播演:
              </span>
              <button
                className={`wh-toggle-pill ${onStart ? 'active' : ''}`}
                style={{ height: 38, width: '100%', fontWeight: 700 }}
                onClick={() => setOnStart((v) => !v)}
              >
                {onStart ? '☑ 开机登录播演' : '☐ 登录时硬切'}
              </button>
            </div>
          </div>
        </div>

        {/* ================================================================
            模块二：9 款 Shader 效果画廊 (3×3 矩阵网格)
            ================================================================ */}
        <div>
          <div
            style={{
              fontSize: 13,
              fontWeight: 800,
              color: 'var(--m3-fg-muted)',
              textTransform: 'uppercase',
              marginBottom: 12,
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <span>
              9 款 Shader 转场效果（点击卡片调优专属参数
              {selection !== 'fixed' && '，点击角标加入轮播池'}）
            </span>
            <span
              style={{
                fontSize: 11,
                fontFamily: 'var(--font-mono)',
                color: 'var(--m3-accent-purple)',
              }}
            >
              当前调参焦点: [{selectedInspectObj.name}]
            </span>
          </div>

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(3, 1fr)',
              gap: 16,
            }}
          >
            {transitions.map((tr) => {
              const isFixedActive =
                selection === 'fixed' && tr.id === fixedEffect
              const isInPool = poolEffects.includes(tr.id)
              const isInspected = tr.id === inspectEffect

              return (
                <div
                  key={tr.id}
                  className="tr-card"
                  style={{
                    background: 'var(--m3-surface)',
                    borderRadius: 'var(--m3-radius-lg)',
                    padding: 18,
                    cursor: 'pointer',
                    boxShadow: isInspected
                      ? '0 0 0 3px var(--m3-accent-purple), var(--m3-shadow-md)'
                      : 'var(--m3-shadow-sm)',
                    border: isFixedActive
                      ? '2px solid var(--m3-accent-lime)'
                      : '1px solid var(--m3-outline-variant)',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: 8,
                    transition: 'all 0.2s cubic-bezier(0.2, 0, 0, 1)',
                  }}
                  onClick={() => handleCardClick(tr)}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                    }}
                  >
                    <span style={{ fontWeight: 800, fontSize: 14 }}>
                      {tr.name}
                    </span>

                    {/* 固定模式下的主激活标签 */}
                    {selection === 'fixed' && isFixedActive && (
                      <span
                        style={{
                          fontSize: 11,
                          fontWeight: 900,
                          padding: '2px 8px',
                          borderRadius: 9999,
                          background: 'var(--m3-accent-lime)',
                          color: '#1c192b',
                        }}
                      >
                        ★ 当前生效
                      </span>
                    )}

                    {/* 轮播/随机模式下的入池开关 */}
                    {selection !== 'fixed' && (
                      <button
                        onClick={(e) => togglePoolEffect(e, tr.id)}
                        style={{
                          border: 'none',
                          padding: '2px 8px',
                          borderRadius: 9999,
                          fontSize: 11,
                          fontWeight: 800,
                          cursor: 'pointer',
                          background: isInPool
                            ? 'rgba(120, 220, 119, 0.25)'
                            : 'var(--m3-surface-subtle)',
                          color: isInPool ? '#2e7d32' : 'var(--m3-fg-muted)',
                        }}
                      >
                        {isInPool ? '✓ 已入池' : '+ 加入池'}
                      </button>
                    )}
                  </div>

                  <div
                    style={{
                      fontSize: 12,
                      color: 'var(--m3-fg-muted)',
                      lineHeight: 1.45,
                    }}
                  >
                    {tr.desc}
                  </div>

                  {isInspected && (
                    <div
                      style={{
                        marginTop: 'auto',
                        paddingTop: 6,
                        borderTop: '1px dashed var(--m3-outline-variant)',
                        fontSize: 10,
                        fontWeight: 700,
                        color: 'var(--m3-accent-purple)',
                        display: 'flex',
                        alignItems: 'center',
                        gap: 4,
                      }}
                    >
                      <span>⚙️ 正在配置下方专属参数</span>
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        </div>

        {/* ================================================================
            模块三：所选效果高级情境调参面板 (Contextual Parameter Inspector)
            ================================================================ */}
        <div
          className="wallhaven-control-box"
          style={{
            margin: 0,
            padding: 22,
            border: '2px solid var(--m3-surface-container)',
          }}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: 16,
              paddingBottom: 12,
              borderBottom: '1px solid var(--m3-outline-variant)',
            }}
          >
            <div>
              <div
                style={{
                  fontWeight: 800,
                  fontSize: 15,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                }}
              >
                <span>⚙️「{selectedInspectObj.name}」专属几何与物理参数调优</span>
                <span
                  className="wh-quick-tag"
                  style={{
                    background: 'var(--m3-surface-subtle)',
                    fontSize: 10,
                    fontWeight: 700,
                  }}
                >
                  着色器动态情境
                </span>
              </div>
              <div
                style={{
                  fontSize: 12,
                  color: 'var(--m3-fg-muted)',
                  marginTop: 2,
                }}
              >
                此效果支持的高级参数已高亮呈现；其它不相干的几何参数已自动屏蔽。
              </div>
            </div>

            <button
              className="wh-clear-btn"
              style={{ height: 32, padding: '0 14px' }}
              onClick={() => {
                setDirection('right')
                setSoftness(0.3)
                setCenterX(50)
                setCenterY(50)
                setStripes(12)
                setPush(0.55)
                setStartRadius(0.0)
                setHold(0)
                onNotify('已恢复专属参数默认值')
              }}
            >
              ↺ 重置此效果参数
            </button>
          </div>

          {/* 无额外几何参数的纯色/解离类效果提示 */}
          {isPureColor && (
            <div
              style={{
                padding: '24px 16px',
                textAlign: 'center',
                color: 'var(--m3-fg-muted)',
                fontSize: 13,
                fontWeight: 600,
              }}
            >
              ✨「{selectedInspectObj.name}」为全屏像素/色度平滑混合，直接由基础时长（
              {duration}ms）与缓动曲线（{curve}）驱动，无需额外几何或方向参数。
            </div>
          )}

          {/* 动态专属参数网格 */}
          {!isPureColor && (
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(3, 1fr)',
                gap: 16,
              }}
            >
              {/* 1. 方向 (direction) · Wipe / Stripes / Slide 专属 */}
              {hasDirection && (
                <div className="m3-slider-tile">
                  <span className="m3-tile-label" style={{ marginBottom: 6 }}>
                    🧭 扫光/滑动推进方向 (direction)
                  </span>
                  <div className="m3-seg-box" style={{ width: '100%' }}>
                    <button
                      className={`m3-seg-btn ${
                        direction === 'left' ? 'active' : ''
                      }`}
                      style={{ flex: 1 }}
                      onClick={() => setDirection('left')}
                    >
                      ⬅️ 向左
                    </button>
                    <button
                      className={`m3-seg-btn ${
                        direction === 'right' ? 'active' : ''
                      }`}
                      style={{ flex: 1 }}
                      onClick={() => setDirection('right')}
                    >
                      ➡️ 向右
                    </button>
                    <button
                      className={`m3-seg-btn ${
                        direction === 'up' ? 'active' : ''
                      }`}
                      style={{ flex: 1 }}
                      onClick={() => setDirection('up')}
                    >
                      ⬆️ 向上
                    </button>
                    <button
                      className={`m3-seg-btn ${
                        direction === 'down' ? 'active' : ''
                      }`}
                      style={{ flex: 1 }}
                      onClick={() => setDirection('down')}
                    >
                      ⬇️ 向下
                    </button>
                  </div>
                  <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                    边缘推进或新画面滑入的轴向。
                  </span>
                </div>
              )}

              {/* 2. 虫洞推挤深度 (push) · Portal 专属 */}
              {hasPortal && (
                <div className="m3-slider-tile">
                  <div className="m3-tile-header">
                    <span className="m3-tile-label">🚀 虫洞推挤深度 (push)</span>
                    <div className="m3-num-badge">
                      <input
                        type="number"
                        min="0.0"
                        max="1.0"
                        step="0.05"
                        value={push.toFixed(2)}
                        onChange={(e) =>
                          setPush(parseFloat(e.target.value) || 0.55)
                        }
                      />
                      <span className="m3-num-badge-unit">x</span>
                    </div>
                  </div>
                  <input
                    type="range"
                    className="m3-slider"
                    min="0.0"
                    max="1.0"
                    step="0.05"
                    value={push}
                    onChange={(e) => setPush(parseFloat(e.target.value))}
                  />
                  <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                    Portal 专属：穿过虫洞时旧画面向外推挤的幅度（0 为 Iris）。
                  </span>
                </div>
              )}

              {/* 3. 百叶窗条带数 (stripes) · Stripes 专属 */}
              {hasStripes && (
                <div className="m3-slider-tile">
                  <div className="m3-tile-header">
                    <span className="m3-tile-label">
                      🪟 百叶窗切带数量 (stripes)
                    </span>
                    <div className="m3-num-badge">
                      <input
                        type="number"
                        min="2"
                        max="64"
                        value={stripes}
                        onChange={(e) =>
                          setStripes(parseInt(e.target.value) || 12)
                        }
                      />
                      <span className="m3-num-badge-unit">条</span>
                    </div>
                  </div>
                  <input
                    type="range"
                    className="m3-slider"
                    min="2"
                    max="64"
                    value={stripes}
                    onChange={(e) => setStripes(parseInt(e.target.value))}
                  />
                  <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                    Stripes 专属：边缘被细分为多少并行推进的等宽切带。
                  </span>
                </div>
              )}

              {/* 4. 首帧停顿时间 (hold) · Iris / Portal 专属 */}
              {hasIrisPortal && (
                <div className="m3-slider-tile">
                  <div className="m3-tile-header">
                    <span className="m3-tile-label">⏸️ 首帧成型停顿 (hold)</span>
                    <div className="m3-num-badge">
                      <input
                        type="number"
                        min="0"
                        max="600"
                        step="20"
                        value={hold}
                        onChange={(e) =>
                          setHold(parseInt(e.target.value) || 0)
                        }
                      />
                      <span className="m3-num-badge-unit">ms</span>
                    </div>
                  </div>
                  <input
                    type="range"
                    className="m3-slider"
                    min="0"
                    max="600"
                    step="20"
                    value={hold}
                    onChange={(e) => setHold(parseInt(e.target.value))}
                  />
                  <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                    让初生光圈在起步前悬停瞬间，让人眼看清成型形状。
                  </span>
                </div>
              )}

              {/* 5. 起始光圈孔径 (start_radius) · Iris / Portal 专属 */}
              {hasIrisPortal && (
                <div className="m3-slider-tile">
                  <div className="m3-tile-header">
                    <span className="m3-tile-label">
                      ⭕ 起始孔径 (start_radius)
                    </span>
                    <div className="m3-num-badge">
                      <input
                        type="number"
                        min="0.0"
                        max="0.5"
                        step="0.02"
                        value={startRadius.toFixed(2)}
                        onChange={(e) =>
                          setStartRadius(parseFloat(e.target.value) || 0.0)
                        }
                      />
                    </div>
                  </div>
                  <input
                    type="range"
                    className="m3-slider"
                    min="0.0"
                    max="0.5"
                    step="0.02"
                    value={startRadius}
                    onChange={(e) => setStartRadius(parseFloat(e.target.value))}
                  />
                  <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                    默认 0.0 从单点成型；大于 0 时首帧即为一个成型圆环。
                  </span>
                </div>
              )}

              {/* 6. 边缘羽化 (softness) · Wipe / Stripes / Iris / Portal */}
              {hasSoftness && (
                <div className="m3-slider-tile">
                  <div className="m3-tile-header">
                    <span className="m3-tile-label">
                      🌫️ 边缘羽化柔度 (softness)
                    </span>
                    <div className="m3-num-badge">
                      <input
                        type="number"
                        min="0.0"
                        max="1.0"
                        step="0.05"
                        value={softness.toFixed(2)}
                        onChange={(e) =>
                          setSoftness(parseFloat(e.target.value) || 0.3)
                        }
                      />
                    </div>
                  </div>
                  <input
                    type="range"
                    className="m3-slider"
                    min="0.0"
                    max="1.0"
                    step="0.05"
                    value={softness}
                    onChange={(e) => setSoftness(parseFloat(e.target.value))}
                  />
                  <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                    0 为刀削般锋利硬边，1 为极柔和弥散渐隐过渡带。
                  </span>
                </div>
              )}

              {/* 7. 发散几何中心 (center) · Iris / Portal / Zoom */}
              {hasCenter && (
                <div className="m3-slider-tile">
                  <div className="m3-tile-header">
                    <span className="m3-tile-label">🎯 径向发散中心点 (center)</span>
                    <div className="m3-num-badge">
                      <span>
                        {centerX}%, {centerY}%
                      </span>
                    </div>
                  </div>
                  <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
                    <span style={{ fontSize: 10, color: 'var(--m3-fg-muted)' }}>X:</span>
                    <input
                      type="range"
                      className="m3-slider"
                      min="0"
                      max="100"
                      value={centerX}
                      onChange={(e) => setCenterX(parseInt(e.target.value))}
                    />
                    <span style={{ fontSize: 10, color: 'var(--m3-fg-muted)' }}>Y:</span>
                    <input
                      type="range"
                      className="m3-slider"
                      min="0"
                      max="100"
                      value={centerY}
                      onChange={(e) => setCenterY(parseInt(e.target.value))}
                    />
                  </div>
                  <span style={{ fontSize: 11, color: 'var(--m3-fg-muted)' }}>
                    默认 (50%, 50%) 位于屏幕正中，可调整为视窗偏焦点。
                  </span>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
