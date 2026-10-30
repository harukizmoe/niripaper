//! Per-key editing of `config.toml`, for the settings pages.
//!
//! The pages save each change as it is made, so they need three things: the
//! schema (types, bounds, defaults), the values in effect, and a way to write
//! one key. Writing `null` removes the key, which is how "reset to default" is
//! spelled — the file then states only what differs, as the README asks.
//!
//! "Set in the file" is reported apart from the values: with
//! `animations.follow_niri` on, a key the file leaves out comes from niri, so a
//! key holding its default and a key that is absent are not the same thing.
//!
//! Every write is checked with the daemon's own parser before it reaches the
//! disk: a value the daemon would reject is reported here instead of making
//! the next reload fail and the running configuration go stale.

use niripaper_proto::config::Config;
use niripaper_proto::schema::{keys, Kind};
use serde_json::{Map, Value};
use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, TableLike};

use crate::commands::{read_config_doc, write_config_atomic};

/// The fields an animation is made of. An animation shares its table with
/// other keys (`[animations.overview-open-close]` also holds `zoom`), so it is
/// written and removed field by field, never as the whole table.
const ANIMATION_FIELDS: [&str; 5] = ["off", "duration_ms", "curve", "cubic_bezier", "spring"];

#[derive(serde::Serialize)]
pub struct ConfigView {
    /// Every key's value, defaults filled in.
    values: Map<String, Value>,
    /// The keys the file itself sets.
    set: Vec<&'static str>,
}

/// Every key a page can offer. Read from the app's own copy of the protocol
/// crate, so the pages work while the daemon is stopped.
#[tauri::command]
pub fn config_schema() -> Value {
    niripaper_proto::schema::schema()
}

/// The configuration's values, defaults filled in, keyed by schema name.
#[tauri::command]
pub fn config_values() -> Result<ConfigView, String> {
    let (_, doc) = read_config_doc()?;
    view_of(&doc)
}

/// Set one key (`null` removes it) and return the values afterwards.
#[tauri::command]
pub fn config_set(key: String, value: Value) -> Result<ConfigView, String> {
    let (path, mut doc) = read_config_doc()?;
    set_key(&mut doc, &key, &value)?;
    let view = view_of(&doc).map_err(|e| format!("{key} 的值无效：{e}"))?;
    write_config_atomic(&path, &doc)?;
    Ok(view)
}

fn view_of(doc: &DocumentMut) -> Result<ConfigView, String> {
    Ok(ConfigView {
        values: values_of(&doc.to_string())?,
        set: keys()
            .into_iter()
            .filter(|k| is_set(doc, k.name, &k.kind))
            .map(|k| k.name)
            .collect(),
    })
}

fn is_set(doc: &DocumentMut, key: &str, kind: &Kind) -> bool {
    let mut item = doc.as_item();
    for part in key.split('.') {
        match item.get(part) {
            Some(next) => item = next,
            None => return false,
        }
    }
    match kind {
        Kind::Animation { .. } => item
            .as_table_like()
            .is_some_and(|t| ANIMATION_FIELDS.iter().any(|f| t.contains_key(f))),
        _ => true,
    }
}

/// Values as the daemon would resolve them, without niri's overrides: the
/// pages show and edit what the file says.
fn values_of(text: &str) -> Result<Map<String, Value>, String> {
    let config = Config::parse_without_niri(text)?;
    Ok(niripaper_proto::state::config_values(
        &config,
        config.wallpaper.as_deref(),
        config.video_fps,
        config.fit,
        &config.namespace,
    ))
}

fn set_key(doc: &mut DocumentMut, key: &str, value: &Value) -> Result<(), String> {
    let kind = keys()
        .into_iter()
        .find(|k| k.name == key)
        .map(|k| k.kind)
        .ok_or_else(|| format!("未知的配置项：{key}"))?;

    let mut path: Vec<&str> = key.split('.').collect();
    let leaf = path.pop().ok_or("配置项名称为空")?;
    let mut table: &mut dyn TableLike = doc.as_table_mut();
    for part in path {
        if !table.contains_key(part) {
            if value.is_null() {
                return Ok(()); // Nothing there to remove.
            }
            table.insert(part, Item::Table(Table::new()));
        }
        table = table
            .get_mut(part)
            .and_then(Item::as_table_like_mut)
            .ok_or_else(|| format!("{part} 在配置文件里不是一个表"))?;
    }

    // An animation's forms exclude each other (`off`, an easing, a spring),
    // so its fields are replaced, not merged — and only its fields.
    if matches!(kind, Kind::Animation { .. }) {
        if value.is_null() && !table.contains_key(leaf) {
            return Ok(());
        }
        if !table.contains_key(leaf) {
            table.insert(leaf, Item::Table(Table::new()));
        }
        let inner = table
            .get_mut(leaf)
            .and_then(Item::as_table_like_mut)
            .ok_or_else(|| format!("{leaf} 在配置文件里不是一个表"))?;
        for field in ANIMATION_FIELDS {
            inner.remove(field);
        }
        match value {
            Value::Null => {}
            Value::Object(fields) => {
                for (k, v) in fields {
                    // `cubic_bezier` holds floats even when a point is 0 or 1.
                    inner.insert(k, Item::Value(to_toml(v, k == "cubic_bezier")?));
                }
            }
            _ => return Err(format!("{key} 应当是一个动画")),
        }
        if inner.is_empty() {
            table.remove(leaf);
        }
        return Ok(());
    }

    if value.is_null() {
        table.remove(leaf);
        return Ok(());
    }

    let floats = matches!(kind, Kind::Float { .. } | Kind::Numbers { .. });
    let mut new = to_toml(value, floats)?;
    // Keep the comment that documents the key.
    match table.get_mut(leaf) {
        Some(Item::Value(old)) => {
            *new.decor_mut() = old.decor().clone();
            *old = new;
        }
        _ => {
            table.insert(leaf, Item::Value(new));
        }
    }
    Ok(())
}

/// JSON → TOML. `floats` writes whole numbers as `1.0`: the page sends a
/// slider's `1` for a float key, and `scale = 1` would read as an integer.
fn to_toml(value: &Value, floats: bool) -> Result<toml_edit::Value, String> {
    Ok(match value {
        Value::Bool(b) => (*b).into(),
        Value::Number(n) if !floats && n.is_i64() => n.as_i64().unwrap_or_default().into(),
        Value::Number(n) => n.as_f64().ok_or("数值超出范围")?.into(),
        Value::String(s) => s.as_str().into(),
        Value::Array(items) => {
            let mut array = Array::new();
            for item in items {
                array.push(to_toml(item, floats)?);
            }
            array.into()
        }
        Value::Object(fields) => {
            let mut table = InlineTable::new();
            for (k, v) in fields {
                table.insert(k, to_toml(v, floats)?);
            }
            table.into()
        }
        Value::Null => return Err("null 只能用来删除配置项".to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn set(text: &str, key: &str, value: Value) -> Result<String, String> {
        let mut doc = text.parse::<DocumentMut>().unwrap();
        set_key(&mut doc, key, &value)?;
        values_of(&doc.to_string())?;
        Ok(doc.to_string())
    }

    #[test]
    fn a_float_key_is_written_as_a_float() {
        let out = set("", "scale", json!(1)).unwrap();
        assert_eq!(out.trim(), "scale = 1.0");
    }

    #[test]
    fn null_removes_the_key_and_keeps_the_rest() {
        let out = set("scale = 1.2\ncolumn_span = 4\n", "scale", Value::Null).unwrap();
        assert_eq!(out, "column_span = 4\n");
    }

    #[test]
    fn removing_a_key_that_is_not_there_writes_nothing() {
        let out = set("", "animations.overview-open-close.zoom", Value::Null).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn a_nested_key_creates_its_tables() {
        let out = set("", "animations.overview-open-close.zoom", json!(0.98)).unwrap();
        let config = Config::parse_without_niri(&out).unwrap();
        assert_eq!(config.animations.overview_open_close.zoom, 0.98);
    }

    #[test]
    fn filter_keys_create_filter_table_and_validate() {
        let out = set("", "filter.brightness", json!(1.2)).unwrap();
        assert!(out.contains("[filter]\nbrightness = 1.2"));
        let config = Config::parse_without_niri(&out).unwrap();
        assert_eq!(config.filter.brightness, 1.2);
    }

    /// The comments document every key; editing one must not drop them.
    #[test]
    fn replacing_a_value_keeps_its_comment() {
        let out = set("scale = 1.1  # canvas enlargement\n", "scale", json!(1.2)).unwrap();
        assert_eq!(out, "scale = 1.2  # canvas enlargement\n");
    }

    /// `off` and `duration_ms` may not coexist, so an animation is replaced
    /// whole rather than merged into.
    #[test]
    fn an_animation_replaces_its_previous_form() {
        let text = "[animations.parallax]\noff = true\n";
        let out = set(
            text,
            "animations.parallax",
            json!({ "duration_ms": 400, "curve": "linear" }),
        )
        .unwrap();
        assert!(!out.contains("off"));
        let config = Config::parse_without_niri(&out).unwrap();
        let values = niripaper_proto::schema::animation_value(&config.animations.parallax);
        assert_eq!(values, json!({ "duration_ms": 400, "curve": "linear" }));
    }

    /// `zoom` lives in the overview animation's table; editing or resetting
    /// the animation must leave it alone.
    #[test]
    fn an_animation_keeps_the_other_keys_in_its_table() {
        let text = "[animations.overview-open-close]\nzoom = 0.98\noff = true\n";
        let out = set(
            text,
            "animations.overview-open-close",
            json!({ "duration_ms": 200, "curve": "linear" }),
        )
        .unwrap();
        assert!(out.contains("zoom = 0.98") && !out.contains("off"), "{out}");
        let out = set(&out, "animations.overview-open-close", Value::Null).unwrap();
        assert!(
            out.contains("zoom = 0.98") && !out.contains("duration_ms"),
            "{out}"
        );
    }

    #[test]
    fn a_reset_animation_drops_its_empty_table() {
        let out = set(
            "[animations.parallax]\noff = true\n",
            "animations.parallax",
            Value::Null,
        )
        .unwrap();
        assert!(!out.contains("parallax"), "{out}");
    }

    /// A key stated at its default is still "set": with follow_niri on, that
    /// is what makes it win over niri's value.
    #[test]
    fn set_lists_what_the_file_states() {
        let doc = "scale = 1.1\n[animations.overview-open-close]\nzoom = 0.98\n"
            .parse::<DocumentMut>()
            .unwrap();
        let view = view_of(&doc).unwrap();
        assert!(view.set.contains(&"scale"));
        assert!(view.set.contains(&"animations.overview-open-close.zoom"));
        assert!(!view.set.contains(&"animations.overview-open-close"));
        assert!(!view.set.contains(&"column_span"));
    }

    #[test]
    fn a_value_the_daemon_rejects_is_refused() {
        assert!(set("", "scale", json!(9.0)).is_err());
        assert!(set("", "column_span", json!(1)).is_err());
    }

    #[test]
    fn an_unknown_key_is_refused() {
        assert!(set("", "no_such_key", json!(1)).is_err());
    }
}
