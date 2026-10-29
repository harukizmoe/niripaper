// Controls for config values, one per schema type. Each one shows the value
// while it is being changed and calls `onCommit` once the change is finished
// (slider released, field left or Enter pressed), so a drag writes the file
// once, not once per pixel.

import { useState, type ReactNode } from 'react'
import {
  Box,
  InputAdornment,
  MenuItem,
  Select,
  Slider,
  Stack,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from '@mui/material'

const decimals = (step: number) => (String(step).split('.')[1] ?? '').length
const snap = (v: number, step: number) => Number((Math.round(v / step) * step).toFixed(decimals(step)))

/// A number typed into a field: kept as text while editing, so "0." is not
/// rejected half-way, and parsed on commit.
export function NumberInput({
  value,
  min,
  max,
  step,
  unit,
  disabled,
  width = 104,
  label,
  onCommit,
}: {
  value: number
  min?: number
  max?: number
  step: number
  unit?: string
  disabled?: boolean
  width?: number
  label?: string
  onCommit: (v: number) => void
}) {
  const [text, setText] = useState(String(value))
  // A new value from outside (a reset, a slider) replaces what is typed.
  const [shown, setShown] = useState(value)
  if (value !== shown) {
    setShown(value)
    setText(String(value))
  }

  const commit = () => {
    const n = Number(text)
    if (text.trim() === '' || !Number.isFinite(n)) {
      setText(String(value))
      return
    }
    const clamped = Math.min(max ?? Infinity, Math.max(min ?? -Infinity, snap(n, step)))
    setText(String(clamped))
    if (clamped !== value) onCommit(clamped)
  }

  return (
    <TextField
      size="small"
      value={text}
      disabled={disabled}
      label={label}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()}
      sx={{ width }}
      slotProps={{
        htmlInput: { inputMode: 'decimal', 'aria-label': label, style: { textAlign: 'right' } },
        input: unit ? { endAdornment: <InputAdornment position="end">{unit}</InputAdornment> } : undefined,
      }}
    />
  )
}

/// A slider for a bounded number, with the exact value in a field beside it.
/// The field may go past the slider's range when the key itself allows it.
export function SliderField({
  value,
  min,
  max,
  step,
  inputMax,
  unit,
  disabled,
  onCommit,
}: {
  value: number
  min: number
  max: number
  step: number
  /** Upper bound for the typed value when it is wider than the slider's. */
  inputMax?: number
  unit?: string
  disabled?: boolean
  onCommit: (v: number) => void
}) {
  const [draft, setDraft] = useState(value)
  const [shown, setShown] = useState(value)
  if (value !== shown) {
    setShown(value)
    setDraft(value)
  }

  return (
    <Stack direction="row" spacing={3} sx={{ alignItems: 'center' }}>
      <Slider
        value={Math.min(max, Math.max(min, draft))}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        onChange={(_, v) => setDraft(v as number)}
        onChangeCommitted={(_, v) => {
          const n = snap(v as number, step)
          if (n !== value) onCommit(n)
        }}
        sx={{ flex: 1, mx: 1 }}
      />
      <NumberInput
        value={draft}
        min={min}
        max={inputMax ?? max}
        step={step}
        unit={unit}
        disabled={disabled}
        onCommit={onCommit}
      />
    </Stack>
  )
}

const CURVE_LABELS: Record<string, string> = {
  linear: '匀速',
  'ease-out-quad': '减速（轻）',
  'ease-out-cubic': '减速（中）',
  'ease-out-expo': '减速（强）',
  'cubic-bezier': '自定义贝塞尔',
}

export function CurveSelect({
  value,
  curves,
  disabled,
  onChange,
}: {
  value: string
  curves: string[]
  disabled?: boolean
  onChange: (curve: string) => void
}) {
  return (
    <Select size="small" value={value} disabled={disabled} onChange={(e) => onChange(e.target.value)} sx={{ minWidth: 160 }}>
      {curves.map((c) => (
        <MenuItem key={c} value={c}>
          {CURVE_LABELS[c] ?? c}
        </MenuItem>
      ))}
    </Select>
  )
}

/// Four control points for `cubic-bezier`, as CSS writes them.
export function BezierInput({
  value,
  disabled,
  onCommit,
}: {
  value: number[]
  disabled?: boolean
  onCommit: (points: number[]) => void
}) {
  const names = ['x1', 'y1', 'x2', 'y2']
  return (
    <Stack direction="row" spacing={1}>
      {value.map((p, i) => (
        <NumberInput
          key={names[i]}
          label={names[i]}
          value={p}
          step={0.01}
          // x must stay in 0..1 for the curve to be a function of time; y may
          // overshoot.
          min={i % 2 === 0 ? 0 : undefined}
          max={i % 2 === 0 ? 1 : undefined}
          width={84}
          disabled={disabled}
          onCommit={(n) => onCommit(value.map((q, j) => (j === i ? n : q)))}
        />
      ))}
    </Stack>
  )
}

// ---------------------------------------------------------------------------
// Animation: niri's three forms — an easing, a spring, or off.

export type AnimationValue =
  | { off: true }
  | { duration_ms: number; curve: string; cubic_bezier?: number[] }
  | { spring: { damping_ratio: number; stiffness: number; epsilon: number } }

type Mode = 'easing' | 'spring' | 'off'

const modeOf = (v: AnimationValue): Mode => ('off' in v ? 'off' : 'spring' in v ? 'spring' : 'easing')

// What a form starts as when switched to. The spring is niri's own default.
const EASING_START = { duration_ms: 400, curve: 'ease-out-cubic' }
const SPRING_START = { spring: { damping_ratio: 1.0, stiffness: 800, epsilon: 0.0001 } }
const BEZIER_START = [0.4, 0, 0.2, 1]

export function AnimationField({
  value,
  curves,
  disabled,
  onCommit,
}: {
  value: AnimationValue
  curves: string[]
  disabled?: boolean
  onCommit: (v: AnimationValue) => void
}) {
  const mode = modeOf(value)

  const setMode = (next: Mode | null) => {
    if (!next || next === mode) return
    onCommit(next === 'off' ? { off: true } : next === 'spring' ? SPRING_START : EASING_START)
  }

  return (
    <Stack spacing={2}>
      <ToggleButtonGroup exclusive size="small" value={mode} onChange={(_, m) => setMode(m)} disabled={disabled}>
        <ToggleButton value="easing">缓动</ToggleButton>
        <ToggleButton value="spring">弹簧</ToggleButton>
        <ToggleButton value="off">关闭</ToggleButton>
      </ToggleButtonGroup>

      {'duration_ms' in value && (
        <>
          <Labeled label="时长">
            <SliderField
              value={value.duration_ms}
              min={50}
              max={2000}
              step={10}
              inputMax={10000}
              unit="ms"
              disabled={disabled}
              onCommit={(duration_ms) => onCommit({ ...value, duration_ms })}
            />
          </Labeled>
          <Labeled label="曲线">
            <Stack direction="row" spacing={2} sx={{ alignItems: 'center', flexWrap: 'wrap', rowGap: 1 }}>
              <CurveSelect
                value={value.curve}
                curves={curves}
                disabled={disabled}
                onChange={(curve) =>
                  onCommit(
                    curve === 'cubic-bezier'
                      ? { duration_ms: value.duration_ms, curve, cubic_bezier: value.cubic_bezier ?? BEZIER_START }
                      : { duration_ms: value.duration_ms, curve },
                  )
                }
              />
              {value.curve === 'cubic-bezier' && (
                <BezierInput
                  value={value.cubic_bezier ?? BEZIER_START}
                  disabled={disabled}
                  onCommit={(cubic_bezier) => onCommit({ ...value, cubic_bezier })}
                />
              )}
            </Stack>
          </Labeled>
        </>
      )}

      {'spring' in value && (
        <>
          <Labeled label="阻尼比" hint="小于 1 会回弹">
            <SliderField
              value={value.spring.damping_ratio}
              min={0.1}
              max={2}
              step={0.05}
              disabled={disabled}
              onCommit={(damping_ratio) => onCommit({ spring: { ...value.spring, damping_ratio } })}
            />
          </Labeled>
          <Labeled label="刚度" hint="越大越快">
            <SliderField
              value={value.spring.stiffness}
              min={10}
              max={2000}
              step={10}
              inputMax={100000}
              disabled={disabled}
              onCommit={(stiffness) => onCommit({ spring: { ...value.spring, stiffness } })}
            />
          </Labeled>
        </>
      )}
    </Stack>
  )
}

function Labeled({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <Stack direction="row" spacing={2} sx={{ alignItems: 'center' }}>
      <Box sx={{ width: 112, flexShrink: 0 }}>
        <Typography variant="body1">{label}</Typography>
        {hint && (
          <Typography variant="body2" sx={{ color: 'text.secondary' }}>
            {hint}
          </Typography>
        )}
      </Box>
      <Box sx={{ flex: 1, minWidth: 0 }}>{children}</Box>
    </Stack>
  )
}
