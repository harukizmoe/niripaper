import { useCallback, useEffect, useMemo, useState } from 'react'
import { invoke } from './useDaemon'

// The configuration as the settings pages see it: the schema (what each key
// is, its bounds and default) and the values in effect. Both come from the
// app's own copy of the protocol crate (app/src/config_edit.rs), so the pages
// work whether or not the daemon is running.
//
// Every change is written at once (the daemon reloads the file itself);
// `null` removes the key, which is "reset to default". `set` lists the keys
// the file states: with follow_niri on, a key it leaves out comes from niri,
// so "stated at its default" and "absent" are different.

export type Kind =
  | { type: 'bool' }
  | { type: 'int'; min: number; max: number | null }
  | { type: 'float'; min: number; max: number | null }
  | { type: 'path' }
  | { type: 'text' }
  | { type: 'enum'; options: string[] }
  | { type: 'multi_enum'; options: string[] }
  | { type: 'numbers'; count: number; min: number | null; max: number | null }
  | { type: 'animation'; curves: string[] }

export interface SchemaKey {
  name: string
  kind: Kind
  default: unknown
  unit?: string
  hot: boolean
  about: string
}

export type ConfigValue = unknown

interface ConfigView {
  values: Record<string, ConfigValue>
  set: string[]
}

export function useConfig(onError: (message: string) => void) {
  const [schema, setSchema] = useState<Record<string, SchemaKey> | null>(null)
  const [view, setView] = useState<ConfigView | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)

  useEffect(() => {
    Promise.all([
      invoke<{ keys: SchemaKey[] }>('config_schema'),
      invoke<ConfigView>('config_values'),
    ]).then(
      ([s, v]) => {
        setSchema(Object.fromEntries(s.keys.map((k) => [k.name, k])))
        setView(v)
      },
      (e) => setLoadError(String(e)),
    )
  }, [])

  /// Write one key. The value shows at once; if the write is refused the
  /// previous one comes back and the reason is reported.
  const set = useCallback(
    async (key: string, value: ConfigValue | null) => {
      let previous: ConfigView | null = null
      setView((v) => {
        previous = v
        if (!v) return v
        return {
          values: { ...v.values, [key]: value === null ? schema?.[key]?.default : value },
          set: value === null ? v.set.filter((k) => k !== key) : [...new Set([...v.set, key])],
        }
      })
      try {
        setView(await invoke<ConfigView>('config_set', { key, value }))
      } catch (e) {
        setView(previous)
        onError(String(e))
      }
    },
    [schema, onError],
  )

  const api = useMemo(
    () => ({
      ready: schema !== null && view !== null,
      loadError,
      key: (name: string) => schema?.[name],
      value: <T,>(name: string) => view?.values[name] as T,
      /** Whether the file states this key (and "reset" would remove it). */
      isSet: (name: string) => view?.set.includes(name) ?? false,
      set,
      reset: (name: string) => set(name, null),
    }),
    [schema, view, loadError, set],
  )
  return api
}

export type Config = ReturnType<typeof useConfig>
