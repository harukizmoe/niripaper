import { useState } from 'react'
import { ButtonBase, Chip, Stack, Switch, ToggleButton, ToggleButtonGroup, Typography, Box } from '@mui/material'
import CheckCircleRounded from '@mui/icons-material/CheckCircleRounded'
import RadioButtonUncheckedRounded from '@mui/icons-material/RadioButtonUncheckedRounded'
import { useConfig, type Config } from '../hooks/useConfig'
import { BezierInput, CurveSelect, Labeled, NumberInput, SliderField } from './fields'
import { SettingRow, SettingsGroup, SettingsPage } from './settings'

// The wallpaper-change transition, as `config.toml` states it. Every row edits
// one key and saves it at once; the daemon reloads the file itself.

const EFFECTS: { id: string; name: string; desc: string }[] = [
  { id: 'none', name: '无过渡', desc: '直接切换，没有动画。' },
  { id: 'fade', name: '淡入淡出', desc: '新壁纸在旧壁纸上逐渐显现。' },
  { id: 'dissolve', name: '溶解', desc: '画面按像素颗粒逐点换成新壁纸。' },
  { id: 'wipe', name: '擦除', desc: '一条边缘扫过屏幕，扫过的地方换成新壁纸。' },
  { id: 'stripes', name: '百叶窗', desc: '边缘分成多条切带，同时向前推进。' },
  { id: 'iris', name: '光圈', desc: '新壁纸从一个圆形开口里展开。' },
  { id: 'portal', name: '传送门', desc: '圆形开口展开的同时，旧画面向外推开。' },
  { id: 'zoom', name: '变焦', desc: '新壁纸从远处迅速拉近，盖住旧画面。' },
  { id: 'slide', name: '滑动', desc: '新壁纸沿一个方向滑入。' },
]

const nameOf = (id: string) => EFFECTS.find((e) => e.id === id)?.name ?? id

// Which effects read which parameter.
const USED_BY = {
  direction: ['wipe', 'stripes', 'slide'],
  softness: ['wipe', 'stripes', 'iris', 'portal'],
  center: ['iris', 'portal', 'zoom'],
  push: ['portal'],
  stripes: ['stripes'],
  start_radius: ['iris', 'portal'],
  hold_ms: ['iris', 'portal'],
}

const BEZIER_START = [0.42, 0, 0.58, 1]

const SELECTIONS = [
  { id: 'fixed', label: '固定', desc: '每次换壁纸都用同一个效果。' },
  { id: 'rotate', label: '轮播', desc: '按顺序轮流使用候选效果。' },
  { id: 'random', label: '随机', desc: '每次从候选效果里随机选一个。' },
]

export function TransitionsView({ onNotify }: { onNotify: (msg: string) => void }) {
  const config = useConfig(onNotify)

  return (
    <SettingsPage title="过渡">
      {config.ready ? (
        <Settings config={config} onNotify={onNotify} />
      ) : (
        <Typography sx={{ color: 'text.secondary', px: 2 }}>
          {config.loadError ? `读取配置失败：${config.loadError}` : '正在读取配置…'}
        </Typography>
      )}
    </SettingsPage>
  )
}

function Settings({ config: c, onNotify }: { config: Config; onNotify: (msg: string) => void }) {
  const selection = c.value<string>('transition.selection')
  const effect = c.value<string>('transition.effect')
  const pool = c.value<string[]>('transition.effects')
  const curve = c.value<string>('transition.curve')
  const fixed = selection === 'fixed'
  const active = fixed ? [effect] : pool

  const curves = (() => {
    const kind = c.key('transition.curve')?.kind
    return kind?.type === 'enum' ? kind.options : []
  })()

  // Looking at one effect narrows the parameters to the ones it reads. It is
  // only a view: no value changes. A focus that has left the selection falls
  // back to showing everything.
  const [focus, setFocus] = useState('all')
  const shown = active.includes(focus) ? [focus] : active

  const applies = (param: keyof typeof USED_BY) => USED_BY[param].filter((id) => shown.includes(id))

  /// Who reads this parameter, as tags under the row's help text.
  const usedBy = (param: keyof typeof USED_BY) => {
    const users = USED_BY[param].filter((id) => active.includes(id))
    return (
      <Stack direction="row" spacing={0.75} sx={{ alignItems: 'center', flexWrap: 'wrap', rowGap: 0.75, mt: 0.75 }}>
        {users.map((id) => (
          <Chip
            key={id}
            size="small"
            label={nameOf(id)}
            color={id === focus ? 'primary' : 'default'}
            variant={id === focus ? 'filled' : 'outlined'}
          />
        ))}
        <Typography variant="body2" sx={{ color: 'text.secondary' }}>
          {USED_BY[param].length === 1 ? '专属参数' : users.length > 1 ? '这些效果共用，改动同时生效' : ''}
        </Typography>
      </Stack>
    )
  }

  const togglePool = (id: string) => {
    if (!pool.includes(id)) return void c.set('transition.effects', [...pool, id])
    if (pool.length <= 1) return onNotify('候选效果至少保留一个')
    void c.set('transition.effects', pool.filter((e) => e !== id))
  }

  const setCurve = (next: string) => {
    void c.set('transition.curve', next)
    // The curve needs its control points before it can run.
    if (next === 'cubic-bezier' && !c.isSet('transition.cubic_bezier')) {
      void c.set('transition.cubic_bezier', BEZIER_START)
    }
  }

  const center = c.value<number[]>('transition.center')
  const bezier = c.value<number[] | null>('transition.cubic_bezier') ?? BEZIER_START
  const percent = (v: number) => Math.round(v * 100)

  const param = (key: string, children: React.ReactNode, label: string, desc: string, which: keyof typeof USED_BY) =>
    applies(which).length > 0 && (
      <SettingRow
        label={label}
        description={<>{desc}{usedBy(which)}</>}
        modified={c.isSet(key)}
        onReset={() => void c.reset(key)}
      >
        {children}
      </SettingRow>
    )

  return (
    <>
      <SettingsGroup title="换壁纸时">
        <SettingRow
          label="效果选择方式"
          description={SELECTIONS.find((s) => s.id === selection)?.desc}
          modified={c.isSet('transition.selection')}
          onReset={() => void c.reset('transition.selection')}
          control={
            <ToggleButtonGroup
              exclusive
              size="small"
              value={selection}
              onChange={(_, v) => v && void c.set('transition.selection', v)}
            >
              {SELECTIONS.map((s) => (
                <ToggleButton key={s.id} value={s.id}>
                  {s.label}
                </ToggleButton>
              ))}
            </ToggleButtonGroup>
          }
        />
        <SettingRow
          label="登录时播放过渡"
          description="守护进程启动时播放一次过渡，登录时不会硬切。"
          modified={c.isSet('transition.on_start')}
          onReset={() => void c.reset('transition.on_start')}
          control={
            <Switch
              checked={c.value<boolean>('transition.on_start')}
              onChange={() => void c.set('transition.on_start', !c.value<boolean>('transition.on_start'))}
            />
          }
        />
      </SettingsGroup>

      <SettingsGroup
        title={fixed ? '使用的效果' : '候选效果'}
        description={fixed ? '选一个效果。' : '勾选参与的效果，至少保留一个。'}
      >
        <Box sx={{ p: 2 }}>
          <Box
            sx={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))',
              gap: 1.5,
            }}
          >
            {EFFECTS.map((e) => {
              const selected = fixed ? effect === e.id : pool.includes(e.id)
              return (
                <ButtonBase
                  key={e.id}
                  role={fixed ? 'radio' : 'checkbox'}
                  aria-checked={selected}
                  onClick={() => (fixed ? void c.set('transition.effect', e.id) : togglePool(e.id))}
                  sx={{
                    display: 'block',
                    textAlign: 'left',
                    p: 2,
                    borderRadius: 3,
                    border: 1,
                    borderColor: selected ? 'primary.main' : 'divider',
                    bgcolor: selected ? 'md.secondary_container' : 'md.surface_container',
                    color: selected ? 'md.on_secondary_container' : 'text.primary',
                  }}
                >
                  <Stack direction="row" sx={{ alignItems: 'center', mb: 0.5 }}>
                    <Typography variant="subtitle1" sx={{ flex: 1 }}>
                      {e.name}
                    </Typography>
                    {selected ? (
                      <CheckCircleRounded fontSize="small" sx={{ color: 'primary.main' }} />
                    ) : (
                      <RadioButtonUncheckedRounded fontSize="small" sx={{ color: 'text.secondary' }} />
                    )}
                  </Stack>
                  <Typography variant="body2" sx={{ color: 'inherit', opacity: 0.8 }}>
                    {e.desc}
                  </Typography>
                </ButtonBase>
              )
            })}
          </Box>
        </Box>
      </SettingsGroup>

      <SettingsGroup title="时长与节奏">
        <SettingRow
          label="过渡时长"
          description="一次过渡从开始到结束的时间。"
          modified={c.isSet('transition.duration_ms')}
          onReset={() => void c.reset('transition.duration_ms')}
        >
          <SliderField
            value={c.value<number>('transition.duration_ms')}
            min={200}
            max={4000}
            step={50}
            inputMax={60000}
            unit="ms"
            onCommit={(v) => void c.set('transition.duration_ms', v)}
          />
        </SettingRow>
        <SettingRow
          label="缓动曲线"
          description="过渡的快慢变化方式。"
          modified={c.isSet('transition.curve')}
          onReset={() => void c.reset('transition.curve')}
          control={<CurveSelect value={curve} curves={curves} onChange={setCurve} />}
        />
        {curve === 'cubic-bezier' && (
          <SettingRow
            label="贝塞尔控制点"
            description="两个控制点的坐标，写法与 CSS 相同。横坐标限制在 0 到 1 之间。"
            modified={c.isSet('transition.cubic_bezier')}
            onReset={() => void c.reset('transition.cubic_bezier')}
          >
            <BezierInput value={bezier} onCommit={(v) => void c.set('transition.cubic_bezier', v)} />
          </SettingRow>
        )}
        <SettingRow
          label="允许曲线回弹"
          description="曲线可以超出起止位置，让效果有回弹的感觉。"
          modified={c.isSet('transition.allow_overshoot')}
          onReset={() => void c.reset('transition.allow_overshoot')}
          control={
            <Switch
              checked={c.value<boolean>('transition.allow_overshoot')}
              onChange={() =>
                void c.set('transition.allow_overshoot', !c.value<boolean>('transition.allow_overshoot'))
              }
            />
          }
        />
      </SettingsGroup>

      {['direction', 'softness', 'center', 'push', 'stripes', 'start_radius', 'hold_ms'].some((p) =>
        applies(p as keyof typeof USED_BY).length > 0,
      ) && (
        <SettingsGroup title="效果参数" description="只列出选中的效果用到的参数。点效果名只看它的参数；被多个效果共用的参数，改动对它们同时生效。">
          <Stack direction="row" spacing={1} sx={{ px: 2, py: 1.5, flexWrap: 'wrap', rowGap: 1 }}>
            {['all', ...active].map((id) => (
              <Chip
                key={id}
                label={id === 'all' ? '全部' : nameOf(id)}
                color={(id === 'all' ? !active.includes(focus) : id === focus) ? 'primary' : 'default'}
                variant={(id === 'all' ? !active.includes(focus) : id === focus) ? 'filled' : 'outlined'}
                onClick={() => setFocus(id)}
              />
            ))}
          </Stack>
          {applies('direction').length > 0 && (
            <SettingRow
              label="方向"
              description={<>边缘推进或新壁纸滑入的方向。{usedBy('direction')}</>}
              modified={c.isSet('transition.direction')}
              onReset={() => void c.reset('transition.direction')}
              control={
                <ToggleButtonGroup
                  exclusive
                  size="small"
                  value={c.value<string>('transition.direction')}
                  onChange={(_, v) => v && void c.set('transition.direction', v)}
                >
                  <ToggleButton value="left">向左</ToggleButton>
                  <ToggleButton value="right">向右</ToggleButton>
                  <ToggleButton value="up">向上</ToggleButton>
                  <ToggleButton value="down">向下</ToggleButton>
                </ToggleButtonGroup>
              }
            />
          )}
          {param(
            'transition.softness',
            <SliderField
              value={c.value<number>('transition.softness')}
              min={0}
              max={1}
              step={0.05}
              onCommit={(v) => void c.set('transition.softness', v)}
            />,
            '边缘柔和度',
            '0 为硬边，1 为很宽的渐变边。',
            'softness',
          )}
          {applies('center').length > 0 && (
            <SettingRow
              label="中心位置"
              description={<>开口或变焦的起点，按屏幕宽高的百分比计，左上角为 0%。{usedBy('center')}</>}
              modified={c.isSet('transition.center')}
              onReset={() => void c.reset('transition.center')}
            >
              <Stack spacing={1}>
                <Labeled label="横向">
                  <SliderField
                    value={percent(center[0])}
                    min={0}
                    max={100}
                    step={1}
                    unit="%"
                    onCommit={(v) => void c.set('transition.center', [v / 100, center[1]])}
                  />
                </Labeled>
                <Labeled label="纵向">
                  <SliderField
                    value={percent(center[1])}
                    min={0}
                    max={100}
                    step={1}
                    unit="%"
                    onCommit={(v) => void c.set('transition.center', [center[0], v / 100])}
                  />
                </Labeled>
              </Stack>
            </SettingRow>
          )}
          {param(
            'transition.push',
            <SliderField
              value={c.value<number>('transition.push')}
              min={0}
              max={1.5}
              step={0.05}
              onCommit={(v) => void c.set('transition.push', v)}
            />,
            '推开幅度',
            '旧画面被向外推开的距离，0 时与光圈相同。',
            'push',
          )}
          {applies('stripes').length > 0 && (
            <SettingRow
              label="切带数量"
              description={<>边缘被分成多少条同时推进的切带。{usedBy('stripes')}</>}
              modified={c.isSet('transition.stripes')}
              onReset={() => void c.reset('transition.stripes')}
              control={
                <NumberInput
                  value={c.value<number>('transition.stripes')}
                  min={2}
                  max={64}
                  step={1}
                  unit="条"
                  onCommit={(v) => void c.set('transition.stripes', v)}
                />
              }
            />
          )}
          {param(
            'transition.start_radius',
            <SliderField
              value={c.value<number>('transition.start_radius')}
              min={0}
              max={1}
              step={0.02}
              onCommit={(v) => void c.set('transition.start_radius', v)}
            />,
            '起始开口大小',
            '开口一开始的大小，按屏幕高度的比例计。0 从一个点展开。',
            'start_radius',
          )}
          {applies('hold_ms').length > 0 && (
            <SettingRow
              label="起步停顿"
              description={<>第一帧先停住不动的时间，让开口的形状看得清。{usedBy('hold_ms')}</>}
              modified={c.isSet('transition.hold_ms')}
              onReset={() => void c.reset('transition.hold_ms')}
            >
              <SliderField
                value={c.value<number>('transition.hold_ms')}
                min={0}
                max={600}
                step={20}
                inputMax={10000}
                unit="ms"
                onCommit={(v) => void c.set('transition.hold_ms', v)}
              />
            </SettingRow>
          )}
        </SettingsGroup>
      )}
    </>
  )
}
