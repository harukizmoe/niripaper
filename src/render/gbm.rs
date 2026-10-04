//! GBM buffer allocation on a *chosen* render node.
//!
//! libgbm is a loader: `gbm_create_device(fd)` picks the vendor backend that
//! owns `fd` (Mesa's `dri_gbm.so` for AMD/Intel, `nvidia-drm_gbm.so` →
//! `libnvidia-allocator` for NVIDIA). That is how we get buffers belonging to
//! the GPU that drives the output instead of the compositor's default one.
//!
//! Nothing here is linked against Mesa specifically; the entry points are
//! declared inline.

use std::ffi::{c_int, c_uint, c_void};
use std::fs::{File, OpenOptions};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::Path;

/// `DRM_FORMAT_XRGB8888` (fourcc "XR24").
pub const FORMAT_XRGB8888: u32 = 0x3432_5258;
/// `DRM_FORMAT_ARGB8888` (fourcc "AR24").
pub const FORMAT_ARGB8888: u32 = 0x3432_5241;

pub const USE_SCANOUT: u32 = 1 << 0;
pub const USE_RENDERING: u32 = 1 << 2;
pub const USE_LINEAR: u32 = 1 << 4;

/// `DRM_FORMAT_MOD_LINEAR`.
pub const MOD_LINEAR: u64 = 0;
/// `DRM_FORMAT_MOD_INVALID`.
pub const MOD_INVALID: u64 = 0x00ff_ffff_ffff_ffff;

#[link(name = "gbm")]
extern "C" {
    fn gbm_create_device(fd: c_int) -> *mut c_void;
    fn gbm_device_destroy(dev: *mut c_void);
    fn gbm_device_is_format_supported(dev: *mut c_void, format: u32, flags: u32) -> c_int;
    fn gbm_bo_create(
        dev: *mut c_void,
        width: u32,
        height: u32,
        format: u32,
        flags: u32,
    ) -> *mut c_void;
    fn gbm_bo_create_with_modifiers(
        dev: *mut c_void,
        width: u32,
        height: u32,
        format: u32,
        modifiers: *const u64,
        count: c_uint,
    ) -> *mut c_void;
    fn gbm_bo_destroy(bo: *mut c_void);
    fn gbm_bo_get_width(bo: *mut c_void) -> u32;
    fn gbm_bo_get_height(bo: *mut c_void) -> u32;
    fn gbm_bo_get_format(bo: *mut c_void) -> u32;
    fn gbm_bo_get_modifier(bo: *mut c_void) -> u64;
    fn gbm_bo_get_plane_count(bo: *mut c_void) -> c_int;
    fn gbm_bo_get_fd_for_plane(bo: *mut c_void, plane: c_int) -> c_int;
    fn gbm_bo_get_stride_for_plane(bo: *mut c_void, plane: c_int) -> u32;
    fn gbm_bo_get_offset(bo: *mut c_void, plane: c_int) -> u32;
}

/// An open GBM device (one GPU).
pub struct Device {
    ptr: *mut c_void,
    /// GBM does not take ownership of the fd; it must outlive the device.
    _fd: File,
    pub path: std::path::PathBuf,
}

impl Device {
    pub fn open(path: &Path) -> Result<Self, String> {
        let fd = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| format!("opening {}: {e}", path.display()))?;
        let ptr = unsafe { gbm_create_device(fd.as_raw_fd()) };
        if ptr.is_null() {
            return Err(format!(
                "gbm_create_device({}) returned NULL",
                path.display()
            ));
        }
        Ok(Self {
            ptr,
            _fd: fd,
            path: path.to_owned(),
        })
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.ptr
    }

    /// The DRM node fd this device was created from.
    pub fn fd(&self) -> RawFd {
        self._fd.as_raw_fd()
    }

    pub fn supports(&self, format: u32, flags: u32) -> bool {
        unsafe { gbm_device_is_format_supported(self.ptr, format, flags) != 0 }
    }

    /// Allocate a buffer, preferring the given modifiers (compositor order).
    ///
    /// Falls back to driver-chosen modifier only when `modifiers` is empty;
    /// an empty modifier list means "no restriction from the compositor".
    pub fn allocate(
        &self,
        width: u32,
        height: u32,
        format: u32,
        modifiers: &[u64],
    ) -> Result<Bo, String> {
        let flags = USE_RENDERING | USE_LINEAR;
        let ptr = if modifiers.is_empty() {
            unsafe { gbm_bo_create(self.ptr, width, height, format, flags) }
        } else {
            unsafe {
                gbm_bo_create_with_modifiers(
                    self.ptr,
                    width,
                    height,
                    format,
                    modifiers.as_ptr(),
                    modifiers.len() as c_uint,
                )
            }
        };
        if ptr.is_null() {
            return Err(format!(
                "gbm_bo_create({width}x{height}, format 0x{format:08x}, {} modifier(s)) failed",
                modifiers.len()
            ));
        }
        Ok(Bo { ptr })
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { gbm_device_destroy(self.ptr) };
    }
}

/// One plane of a GBM buffer: the fd the compositor will import, plus its
/// offset and stride.
pub struct Plane {
    pub fd: OwnedFd,
    pub offset: u32,
    pub stride: u32,
}

/// A GBM buffer object (and therefore the dmabuf behind it).
pub struct Bo {
    ptr: *mut c_void,
}

impl Bo {
    pub fn width(&self) -> u32 {
        unsafe { gbm_bo_get_width(self.ptr) }
    }

    pub fn height(&self) -> u32 {
        unsafe { gbm_bo_get_height(self.ptr) }
    }

    pub fn format(&self) -> u32 {
        unsafe { gbm_bo_get_format(self.ptr) }
    }

    pub fn modifier(&self) -> u64 {
        unsafe { gbm_bo_get_modifier(self.ptr) }
    }

    /// Materialize every plane as an owned fd. `gbm_bo_get_fd_for_plane` dups
    /// the fd each call, so each `Plane` owns its own.
    pub fn planes(&self) -> Result<Vec<Plane>, String> {
        let count = unsafe { gbm_bo_get_plane_count(self.ptr) };
        if count <= 0 {
            return Err(format!("gbm buffer reports {count} planes"));
        }
        let mut out = Vec::with_capacity(count as usize);
        for plane in 0..count {
            let fd = unsafe { gbm_bo_get_fd_for_plane(self.ptr, plane) };
            if fd < 0 {
                return Err(format!(
                    "gbm_bo_get_fd_for_plane(plane {plane}) failed: {fd}"
                ));
            }
            out.push(Plane {
                // SAFETY: fd is a fresh dup owned by us.
                fd: unsafe { OwnedFd::from_raw_fd(fd) },
                offset: unsafe { gbm_bo_get_offset(self.ptr, plane) },
                stride: unsafe { gbm_bo_get_stride_for_plane(self.ptr, plane) },
            });
        }
        Ok(out)
    }
}

impl Drop for Bo {
    fn drop(&mut self) {
        unsafe { gbm_bo_destroy(self.ptr) };
    }
}

/// Human-readable modifier, matching `drm_mode`/`wayland-info` style.
pub fn modifier_name(modifier: u64) -> String {
    if modifier == MOD_LINEAR {
        return "LINEAR".to_owned();
    }
    if modifier == MOD_INVALID {
        return "INVALID".to_owned();
    }
    let vendor = (modifier >> 56) as u8;
    let vendor = match vendor {
        0x00 => "none",
        0x01 => "intel",
        0x02 => "amd",
        0x03 => "nv",
        0x04 => "samsung",
        0x05 => "qcom",
        0x06 => "vivante",
        0x08 => "vivante",
        0x09 => "broadcom",
        0x0a => "arm",
        0x0b => "allwinner",
        0x0c => "amlogic",
        0x0d => "nvidia",
        _ => "vendor",
    };
    format!("{vendor}:0x{modifier:016x}")
}

/// Fourcc → the 4 characters, for logs.
pub fn fourcc_name(format: u32) -> String {
    let bytes = format.to_le_bytes();
    if bytes.iter().all(|b| b.is_ascii_graphic()) {
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        format!("0x{format:08x}")
    }
}
