import Paper from '@mui/material/Paper'
import Typography from '@mui/material/Typography'
import { useEffect, useState } from 'react'

import { invoke } from '../hooks/useDaemon'

// US-002 + FR-3: the settings page is schema-driven. `schema()` comes from the
// compiled-in niripaper-proto (decisions §4.7) — zero daemon involvement — and
// each key renders the widget its `type` names. Adding a config key needs zero
// UI change.
interface SchemaKey {
  name: string
  title: string
  description?: string
  type: string
  default?: unknown
  unit?: string
  min?: number
  max?: number
  choices?: string[]
}

interface Schema {
  keys: SchemaKey[]
  per_output: string[]
}

export function SettingsSection() {
  const [schema, setSchema] = useState<Schema | null>(null)

  useEffect(() => {
    void invoke<Schema>('daemon_schema').then(setSchema).catch(() => setSchema(null))
  }, [])

  return (
    <Paper
      elevation={0}
      sx={{
        padding: 4,
        borderRadius: 'var(--m3-radius-lg)',
        background: 'var(--m3-surface)',
        boxShadow: 'var(--m3-shadow-sm)',
      }}
    >
      <Typography variant="h6" sx={{ fontWeight: 700, marginBottom: 2 }}>
        设置（schema 驱动）
      </Typography>
      {schema === null ? (
        <Typography sx={{ color: 'var(--m3-fg-muted)' }}>正在读取 schema…</Typography>
      ) : (
        <Typography sx={{ color: 'var(--m3-fg-muted)' }}>
          schema 含 {schema.keys.length} 个键、{schema.per_output.length} 个可按输出覆盖。键级控件（US-002 的
          widget 分派）在下一个里程碑接上；本页先证明 schema 通路已通。
        </Typography>
      )}
    </Paper>
  )
}
