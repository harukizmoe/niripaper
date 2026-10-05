//! Which GPU drives which output?
//!
//! A Wayland output is only a connector name ("DP-1"). The kernel exposes that
//! connector as `/sys/class/drm/card<N>-<connector>`, the owning DRM card as
//! `/sys/class/drm/card<N>`, and the card's vendor as `card<N>/device/vendor`.
//! Render nodes (`/dev/dri/renderD<M>`) do not carry a number that matches the
//! card's, but both `/sys/class/drm/cardN/device` and
//! `/sys/class/drm/renderDM/device` symlink to the same PCI device, which is
//! what we match on.
//!
//! This matters because the compositor may drive two outputs from two different
//! vendors: rendering an output's wallpaper on the wrong GPU means every frame
//! is copied across GPUs. See `HANDOFF.md` §7.1.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const DRM_CLASS: &str = "/sys/class/drm";
const DEV_DRI: &str = "/dev/dri";

/// A DRM connector, as seen through sysfs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connector {
    /// Wayland output name, e.g. `DP-1`.
    pub name: String,
    /// Owning DRM card, e.g. `card0`.
    pub card: String,
    /// `connected` according to the kernel (a cable/panel is attached).
    pub connected: bool,
}

/// The GPU that owns a DRM card, addressed the way EGL needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gpu {
    /// DRM card, e.g. `card0`.
    pub card: String,
    /// `/dev/dri/renderD<M>` — the node to open for rendering.
    pub render_node: PathBuf,
    /// PCI vendor id, e.g. `0x10de`.
    pub vendor_id: u32,
}

impl Gpu {
    pub fn vendor_name(&self) -> &'static str {
        match self.vendor_id {
            0x1002 | 0x1022 => "AMD",
            0x10de => "NVIDIA",
            0x8086 => "Intel",
            0x1af4 => "virtio",
            0x13b5 => "ARM",
            _ => "unknown",
        }
    }

    /// Connector(s) this GPU currently drives.
    pub fn connectors(&self) -> io::Result<Vec<Connector>> {
        Ok(connectors()?
            .into_iter()
            .filter(|c| c.card == self.card)
            .collect())
    }
}

/// Split one `/sys/class/drm` entry name into `(card, connector)`.
///
/// Entries are `<card><N>-<connector>`, and the connector's own name may contain
/// dashes (`HDMI-A-1`, `DP-2`), so only the *first* dash separates — splitting on
/// the last one, or on every one, gets `card0` / `HDMI` instead.
///
/// Pure, and separate from the directory walk, so the parsing can be tested
/// against made-up names instead of whatever the machine running the tests
/// happens to have plugged in.
fn split_entry(dir: &str) -> Option<(&str, &str)> {
    let (card, connector) = dir.split_once('-')?;
    let is_card = card
        .strip_prefix("card")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    (is_card && !connector.starts_with("Writeback")).then_some((card, connector))
}

/// Every DRM connector the kernel knows about, connected or not.
pub fn connectors() -> io::Result<Vec<Connector>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(DRM_CLASS)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(dir) = file_name.to_str() else {
            continue;
        };
        let Some((card, connector)) = split_entry(dir) else {
            continue;
        };
        let status = fs::read_to_string(entry.path().join("status")).unwrap_or_default();
        out.push(Connector {
            name: connector.to_owned(),
            card: card.to_owned(),
            connected: status.trim() == "connected",
        });
    }
    out.sort_by(|a, b| a.card.cmp(&b.card).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// Every render node present under `/dev/dri`.
pub fn render_nodes() -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(DRM_CLASS)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if name.starts_with("renderD") {
            out.push(Path::new(DEV_DRI).join(name));
        }
    }
    out.sort();
    Ok(out)
}

/// Resolve the GPU that drives `connector`.
pub fn gpu_for_connector(connector: &str) -> Result<Gpu, String> {
    let all = connectors().map_err(|e| format!("reading {DRM_CLASS}: {e}"))?;
    let found = all.iter().find(|c| c.name == connector).ok_or_else(|| {
        let names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
        format!(
            "no DRM connector `{connector}` (present: {})",
            names.join(", ")
        )
    })?;
    gpu_for_card(&found.card)
}

/// Resolve the GPU that owns `card`.
pub fn gpu_for_card(card: &str) -> Result<Gpu, String> {
    let device = fs::canonicalize(Path::new(DRM_CLASS).join(card).join("device"))
        .map_err(|e| format!("resolving {DRM_CLASS}/{card}/device: {e}"))?;
    let vendor_id = read_vendor(card)?;
    for node in render_nodes().map_err(|e| format!("reading {DRM_CLASS}: {e}"))? {
        let name = node
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("odd render node path {node:?}"))?;
        let node_device = fs::canonicalize(Path::new(DRM_CLASS).join(name).join("device"))
            .map_err(|e| format!("resolving {DRM_CLASS}/{name}/device: {e}"))?;
        if node_device == device {
            return Ok(Gpu {
                card: card.to_owned(),
                render_node: node,
                vendor_id,
            });
        }
    }
    Err(format!("no render node shares a device with {card}"))
}

/// The DRM node (card or render) whose device number is `dev`.
///
/// The compositor's dmabuf feedback names devices as `dev_t` values; this is how
/// that turns back into something openable.
pub fn node_for_device_id(dev: u64) -> Result<PathBuf, String> {
    use std::os::unix::fs::MetadataExt;
    let mut candidates = Vec::new();
    for entry in fs::read_dir(DRM_CLASS).map_err(|e| format!("reading {DRM_CLASS}: {e}"))? {
        let entry = entry.map_err(|e| format!("reading {DRM_CLASS}: {e}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let is_node = name
            .strip_prefix("card")
            .or_else(|| name.strip_prefix("renderD"))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        if !is_node {
            continue;
        }
        let path = Path::new(DEV_DRI).join(name);
        if let Ok(meta) = fs::metadata(&path) {
            if meta.rdev() == dev {
                candidates.push(path);
            }
        }
    }
    // Prefer the render node: it needs no DRM master.
    candidates
        .iter()
        .find(|p| p.to_string_lossy().contains("renderD"))
        .or_else(|| candidates.first())
        .cloned()
        .ok_or_else(|| format!("no /dev/dri node has device number {dev}"))
}

/// Resolve the GPU behind a DRM node path (`/dev/dri/renderD*` or `card*`).
pub fn gpu_for_node(node: &Path) -> Result<Gpu, String> {
    let name = node
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("odd DRM node path {node:?}"))?;
    let device = fs::canonicalize(Path::new(DRM_CLASS).join(name).join("device"))
        .map_err(|e| format!("resolving {DRM_CLASS}/{name}/device: {e}"))?;
    let vendor_raw = fs::read_to_string(Path::new(DRM_CLASS).join(name).join("device/vendor"))
        .map_err(|e| format!("reading vendor of {name}: {e}"))?;
    let vendor_id = u32::from_str_radix(vendor_raw.trim().trim_start_matches("0x"), 16)
        .map_err(|e| format!("unparsable vendor id {vendor_raw:?}: {e}"))?;
    let card = fs::read_dir(DRM_CLASS)
        .map_err(|e| format!("reading {DRM_CLASS}: {e}"))?
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .find(|c| {
            c.strip_prefix("card")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                && fs::canonicalize(Path::new(DRM_CLASS).join(c).join("device"))
                    .map(|d| d == device)
                    .unwrap_or(false)
        })
        .unwrap_or_else(|| name.to_owned());
    let render_node = if name.starts_with("renderD") {
        node.to_owned()
    } else {
        gpu_for_card(&card)?.render_node
    };
    Ok(Gpu {
        card,
        render_node,
        vendor_id,
    })
}

fn read_vendor(card: &str) -> Result<u32, String> {
    let raw = fs::read_to_string(Path::new(DRM_CLASS).join(card).join("device/vendor"))
        .map_err(|e| format!("reading vendor of {card}: {e}"))?;
    let raw = raw.trim().trim_start_matches("0x");
    u32::from_str_radix(raw, 16).map_err(|e| format!("unparsable vendor id {raw:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_entries_split_into_card_and_name() {
        // Only the first dash separates: the connector's own name has dashes too.
        assert_eq!(split_entry("card0-HDMI-A-1"), Some(("card0", "HDMI-A-1")));
        assert_eq!(split_entry("card1-eDP-1"), Some(("card1", "eDP-1")));
        assert_eq!(split_entry("card0-DP-1"), Some(("card0", "DP-1")));
        assert_eq!(split_entry("card12-DP-3"), Some(("card12", "DP-3")));

        // Not connectors: a bare card, a render node, a card with no number, and
        // the kernel's writeback entries (which are not displays).
        assert_eq!(split_entry("card0"), None);
        assert_eq!(split_entry("renderD128"), None);
        assert_eq!(split_entry("cardX-DP-1"), None);
        assert_eq!(split_entry("card0-Writeback-1"), None);
    }

    #[test]
    fn render_nodes_look_like_dev_nodes() {
        for node in render_nodes().expect("sysfs present") {
            assert_eq!(node.parent(), Some(Path::new("/dev/dri")), "{node:?}");
            assert!(
                node.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("renderD"),
                "{node:?}"
            );
        }
    }
}
