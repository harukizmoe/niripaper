//! EGL pinned to one GPU, plus dmabuf import/export.
//!
//! The whole point of this module is that the display is created from a *GBM
//! device* (`EGL_PLATFORM_GBM_KHR`) rather than from the Wayland connection.
//! `EGL_PLATFORM_WAYLAND_KHR` always ends up on the compositor's default GPU,
//! which on this machine is the AMD iGPU while DP-1 is driven by NVIDIA — every
//! frame would then be copied across GPUs. See `HANDOFF.md` §7.1.
//!
//! Nothing is linked beyond `libEGL.so` (the GLVND loader, which dispatches to
//! `libEGL_nvidia` / `libEGL_mesa`) and every optional entry point is fetched
//! through `eglGetProcAddress`.

use std::ffi::{c_char, c_void, CStr, CString};
use std::os::fd::RawFd;
use std::ptr;

pub type EglDisplay = *mut c_void;
pub type EglConfig = *mut c_void;
pub type EglContext = *mut c_void;
pub type EglSurface = *mut c_void;
pub type EglBoolean = u32;
pub type EglInt = i32;
pub type EglEnum = u32;

/// An imported dmabuf image, as GL sees it.
///
/// A newtype rather than a bare pointer so raw pointers never cross this
/// module's API: callers cannot dereference or forge one by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EglImage(*mut c_void);

impl EglImage {
    pub const NONE: Self = Self(ptr::null_mut());

    pub fn is_none(self) -> bool {
        self.0.is_null()
    }

    /// The underlying handle, for GL entry points that take one.
    pub fn as_ptr(self) -> *mut c_void {
        self.0
    }
}

pub const EGL_NO_DISPLAY: EglDisplay = ptr::null_mut();
pub const EGL_NO_CONTEXT: EglContext = ptr::null_mut();
pub const EGL_NO_SURFACE: EglSurface = ptr::null_mut();
pub const EGL_TRUE: EglBoolean = 1;

const EGL_EXTENSIONS: EglInt = 0x3055;
const EGL_VENDOR: EglInt = 0x3053;
const EGL_VERSION: EglInt = 0x3054;
const EGL_PLATFORM_GBM_KHR: EglEnum = 0x31D7;
const EGL_PLATFORM_DEVICE_EXT: EglEnum = 0x313F;

const EGL_SURFACE_TYPE: EglInt = 0x3033;
const EGL_PBUFFER_BIT: EglInt = 0x0001;
const EGL_WINDOW_BIT: EglInt = 0x0004;
const EGL_RENDERABLE_TYPE: EglInt = 0x3040;
const EGL_OPENGL_BIT: EglInt = 0x0008;
const EGL_OPENGL_ES2_BIT: EglInt = 0x0004;
const EGL_RED_SIZE: EglInt = 0x3024;
const EGL_GREEN_SIZE: EglInt = 0x3023;
const EGL_BLUE_SIZE: EglInt = 0x3022;
const EGL_ALPHA_SIZE: EglInt = 0x3021;
const EGL_WIDTH: EglInt = 0x3057;
const EGL_HEIGHT: EglInt = 0x3056;
const EGL_NONE: EglInt = 0x3038;

const EGL_OPENGL_API: EglEnum = 0x30A2;
const EGL_CONTEXT_MAJOR_VERSION: EglInt = 0x3098;
const EGL_CONTEXT_MINOR_VERSION: EglInt = 0x30FB;

const EGL_LINUX_DMA_BUF_EXT: EglEnum = 0x3270;
const EGL_LINUX_DRM_FOURCC_EXT: EglInt = 0x3271;
/// `(fd, offset, pitch)` for plane 0..3; plane 3 lives above the modifier range.
const PLANE_FD: [EglInt; 4] = [0x3272, 0x3275, 0x3278, 0x3440];
const PLANE_OFFSET: [EglInt; 4] = [0x3273, 0x3276, 0x3279, 0x3441];
const PLANE_PITCH: [EglInt; 4] = [0x3274, 0x3277, 0x327A, 0x3442];
const PLANE_MOD_LO: [EglInt; 4] = [0x3443, 0x3445, 0x3447, 0x3449];
const PLANE_MOD_HI: [EglInt; 4] = [0x3444, 0x3446, 0x3448, 0x344A];

#[link(name = "EGL")]
extern "C" {
    fn eglGetProcAddress(name: *const c_char) -> *mut c_void;
    fn eglQueryString(dpy: EglDisplay, name: EglInt) -> *const c_char;
    fn eglGetError() -> EglInt;
    fn eglInitialize(dpy: EglDisplay, major: *mut EglInt, minor: *mut EglInt) -> EglBoolean;
    fn eglTerminate(dpy: EglDisplay) -> EglBoolean;
    fn eglChooseConfig(
        dpy: EglDisplay,
        attribs: *const EglInt,
        configs: *mut EglConfig,
        config_size: EglInt,
        num: *mut EglInt,
    ) -> EglBoolean;
    fn eglCreateContext(
        dpy: EglDisplay,
        cfg: EglConfig,
        share: EglContext,
        attribs: *const EglInt,
    ) -> EglContext;
    fn eglDestroyContext(dpy: EglDisplay, ctx: EglContext) -> EglBoolean;
    fn eglMakeCurrent(
        dpy: EglDisplay,
        draw: EglSurface,
        read: EglSurface,
        ctx: EglContext,
    ) -> EglBoolean;
    fn eglCreatePbufferSurface(
        dpy: EglDisplay,
        cfg: EglConfig,
        attribs: *const EglInt,
    ) -> EglSurface;
    fn eglDestroySurface(dpy: EglDisplay, surface: EglSurface) -> EglBoolean;
    fn eglBindAPI(api: EglEnum) -> EglBoolean;
}

type GetPlatformDisplayExt = unsafe extern "C" fn(
    platform: EglEnum,
    native: *mut c_void,
    attribs: *const EglInt,
) -> EglDisplay;
type CreateImageKhr = unsafe extern "C" fn(
    dpy: EglDisplay,
    ctx: EglContext,
    target: EglEnum,
    buffer: *mut c_void,
    attribs: *const EglInt,
) -> *mut c_void;
type DestroyImageKhr = unsafe extern "C" fn(dpy: EglDisplay, image: *mut c_void) -> EglBoolean;
type QueryDmaBufFormatsExt = unsafe extern "C" fn(
    dpy: EglDisplay,
    max: EglInt,
    formats: *mut EglInt,
    num: *mut EglInt,
) -> EglBoolean;
type QueryDmaBufModifiersExt = unsafe extern "C" fn(
    dpy: EglDisplay,
    format: EglInt,
    max: EglInt,
    modifiers: *mut u64,
    external_only: *mut EglBoolean,
    num: *mut EglInt,
) -> EglBoolean;

/// Which GLVND EGL vendor library to load.
///
/// GLVND picks a vendor by *platform*, not by device: on this machine the
/// NVIDIA ICD claims `EGL_PLATFORM_GBM_KHR` first and happily returns a display
/// for an AMD GBM device — one that has no configs. Pinning the vendor list
/// before EGL is initialized is the supported way to say "use this GPU".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EglVendor {
    Nvidia,
    Mesa,
}

impl EglVendor {
    /// NVIDIA is `0x10de`; everything else on Linux (AMD, Intel, …) is Mesa.
    pub fn for_pci_vendor(vendor_id: u32) -> Self {
        if vendor_id == 0x10de {
            Self::Nvidia
        } else {
            Self::Mesa
        }
    }

    fn library_hint(self) -> &'static str {
        match self {
            Self::Nvidia => "nvidia",
            Self::Mesa => "mesa",
        }
    }

    fn fallback_json(self) -> &'static str {
        match self {
            Self::Nvidia => "/usr/share/glvnd/egl_vendor.d/10_nvidia.json",
            Self::Mesa => "/usr/share/glvnd/egl_vendor.d/50_mesa.json",
        }
    }

    /// Path of the GLVND vendor JSON that loads this driver's EGL.
    pub fn vendor_json(self) -> Option<std::path::PathBuf> {
        let dir = std::path::Path::new("/usr/share/glvnd/egl_vendor.d");
        let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        if let Some(found) = entries.into_iter().find(|p| {
            std::fs::read_to_string(p)
                .map(|c| c.contains(self.library_hint()))
                .unwrap_or(false)
        }) {
            return Some(found);
        }
        let fallback = std::path::PathBuf::from(self.fallback_json());
        fallback.exists().then_some(fallback)
    }

    /// Pin the Vulkan ICD list to this vendor's family, the way
    /// [`Self::vendor_json`] pins EGL.
    ///
    /// Two APIs, one rule (§7.2): the compositor composites on a single GPU, so
    /// a process that ends up with *both* vendors' drivers gains nothing and
    /// costs a crash. `hwdec=auto-safe` resolves to `vulkan-copy` on this
    /// machine, so mpv/FFmpeg create a Vulkan instance — and creating one makes
    /// the loader load *every* installed ICD, NVIDIA's included, on worker
    /// threads that run while `switch_wallpaper` is tearing the previous media
    /// down. The coredump has that race: two threads inside
    /// `av_hwdevice_ctx_create` → `vkEnumerateInstanceExtensionProperties`,
    /// through `libGLX_nvidia.so.0`, one of them jumping to a null pointer.
    ///
    /// Only intervenes when both families are installed: a machine with one ICD
    /// has nothing to disambiguate, and the loader's default list is left
    /// alone. `VK_DRIVER_FILES` is read when the loader initialises, which is
    /// after this and before anything in the process can ask for Vulkan.
    pub fn pin_vulkan(self) -> String {
        let mut nvidia = Vec::new();
        let mut other = Vec::new();
        for dir in ["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d"] {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            let mut paths: Vec<std::path::PathBuf> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
                .collect();
            paths.sort();
            for path in paths {
                let Ok(contents) = std::fs::read_to_string(&path) else {
                    continue;
                };
                // Same trick as the EGL list: the manifest names the driver
                // library, and only NVIDIA's name says which family it is. Mesa
                // is everything else, which is also how `for_pci_vendor` reads
                // a PCI id.
                if contents.contains("nvidia") {
                    nvidia.push(path);
                } else {
                    other.push(path);
                }
            }
        }
        if nvidia.is_empty() || other.is_empty() {
            return "default ICD list".to_owned();
        }
        let keep = match self {
            Self::Nvidia => nvidia,
            Self::Mesa => other,
        };
        let list = keep
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(":");
        let Ok(value) = std::ffi::CString::new(list.clone()) else {
            return "default ICD list".to_owned();
        };
        let key = std::ffi::CString::new("VK_DRIVER_FILES").unwrap();
        unsafe { libc::setenv(key.as_ptr(), value.as_ptr(), 1) };
        list
    }
}

/// Everything EGL needs to import a dmabuf as a texture.
#[derive(Debug, Clone)]
pub struct DmaBufPlane {
    pub fd: RawFd,
    pub offset: u32,
    pub stride: u32,
}

#[derive(Debug, Clone)]
pub struct DmaBufDesc {
    pub width: u32,
    pub height: u32,
    pub fourcc: u32,
    pub modifier: u64,
    pub planes: Vec<DmaBufPlane>,
}

/// An EGL display/config/context bound to one GBM device.
/// `eglGetProcAddress`, for libraries that resolve GL entry points themselves.
/// libmpv's OpenGL render API does exactly that, and the pointers must come from
/// the same context we render with — which is why this is `eglGetProcAddress`
/// and not `dlsym`.
pub fn get_proc_address(name: &str) -> *mut c_void {
    let Ok(name) = std::ffi::CString::new(name) else {
        return std::ptr::null_mut();
    };
    unsafe { eglGetProcAddress(name.as_ptr()) }
}

pub struct Egl {
    pub display: EglDisplay,
    pub config: EglConfig,
    pub context: EglContext,
    pub config_label: String,
    pub client_extensions: String,
    pub display_extensions: String,
    pub version: String,
    pub vendor: String,
    /// Which `EGL_PLATFORM_*` the display was created from.
    pub platform: String,
    /// The GLVND vendor list we pinned, or "default GLVND order".
    pub vendor_pinned: String,
    /// The `VK_DRIVER_FILES` list we pinned, or "default ICD list".
    pub vulkan_pinned: String,
    surface: EglSurface,
    surfless: bool,
    create_image: Option<CreateImageKhr>,
    destroy_image: Option<DestroyImageKhr>,
    query_dmabuf_formats: Option<QueryDmaBufFormatsExt>,
    query_dmabuf_modifiers: Option<QueryDmaBufModifiersExt>,
}

impl Egl {
    /// Create a context on the GPU behind `device_fd`.
    ///
    /// `EGL_PLATFORM_DEVICE_EXT` (the fd) is tried first because it is
    /// vendor-agnostic: the GLVND loader routes it by the device the fd points
    /// at. `EGL_PLATFORM_GBM_KHR` is the fallback, and is what the NVIDIA GBM
    /// backend implements.
    pub fn new(
        gbm_device: *mut c_void,
        device_fd: RawFd,
        vendor: EglVendor,
    ) -> Result<Self, String> {
        // Must happen before the first EGL call in this process: GLVND reads the
        // vendor list once, when it initialises.
        let vendor_pinned = match vendor.vendor_json() {
            Some(json) => {
                let json = json.to_string_lossy().into_owned();
                let c_json = std::ffi::CString::new(json.clone()).map_err(|e| e.to_string())?;
                let c_key = std::ffi::CString::new("__EGL_VENDOR_LIBRARY_FILENAMES").unwrap();
                unsafe { libc::setenv(c_key.as_ptr(), c_json.as_ptr(), 1) };
                Some(json)
            }
            None => None,
        };
        let vendor_note = vendor_pinned
            .clone()
            .unwrap_or_else(|| "default GLVND order".to_owned());
        // The same rule for the other API. This is the last point guaranteed to
        // run before anything in the process can ask for a Vulkan instance: the
        // daemon loads media after its `Egl`, and so does every probe here.
        let vulkan_note = vendor.pin_vulkan();

        let client_extensions = query_client_extensions();
        let get_platform_display: GetPlatformDisplayExt =
            load("eglGetPlatformDisplayEXT").ok_or("eglGetPlatformDisplayEXT missing")?;

        let mut attempts: Vec<String> = Vec::new();
        let mut display = EGL_NO_DISPLAY;
        let mut platform_label = "";
        let mut config: EglConfig = ptr::null_mut();
        let mut config_label = String::new();
        for (label, platform, native) in [
            (
                "EGL_PLATFORM_DEVICE_EXT",
                EGL_PLATFORM_DEVICE_EXT,
                device_fd as isize as *mut c_void,
            ),
            ("EGL_PLATFORM_GBM_KHR", EGL_PLATFORM_GBM_KHR, gbm_device),
        ] {
            let candidate = unsafe { get_platform_display(platform, native, ptr::null()) };
            if candidate == EGL_NO_DISPLAY {
                attempts.push(format!("{label}: EGL_NO_DISPLAY"));
                continue;
            }
            let mut major = 0;
            let mut minor = 0;
            if unsafe { eglInitialize(candidate, &mut major, &mut minor) } != EGL_TRUE {
                attempts.push(format!("{label}: eglInitialize failed (0x{:x})", unsafe {
                    eglGetError()
                }));
                unsafe { eglTerminate(candidate) };
                continue;
            }
            match choose_config(candidate) {
                Ok((found, found_label)) => {
                    display = candidate;
                    platform_label = label;
                    config = found;
                    config_label = found_label;
                    break;
                }
                Err(err) => {
                    attempts.push(format!("{label}: {err}"));
                    unsafe { eglTerminate(candidate) };
                }
            }
        }
        if display == EGL_NO_DISPLAY {
            return Err(format!(
                "no usable EGL display for this GPU (vendor {vendor_note}, client extensions: {client_extensions}); {}",
                attempts.join("; ")
            ));
        }

        let mut egl = Egl {
            display,
            config,
            context: EGL_NO_CONTEXT,
            config_label,
            client_extensions,
            display_extensions: cstr(unsafe { eglQueryString(display, EGL_EXTENSIONS) }),
            version: cstr(unsafe { eglQueryString(display, EGL_VERSION) }),
            vendor: cstr(unsafe { eglQueryString(display, EGL_VENDOR) }),
            platform: platform_label.to_owned(),
            vendor_pinned: vendor_note,
            vulkan_pinned: vulkan_note,
            surface: EGL_NO_SURFACE,
            surfless: false,
            create_image: load("eglCreateImageKHR"),
            destroy_image: load("eglDestroyImageKHR"),
            query_dmabuf_formats: load("eglQueryDmaBufFormatsEXT"),
            query_dmabuf_modifiers: load("eglQueryDmaBufModifiersEXT"),
        };
        egl.create_context()?;
        Ok(egl)
    }

    fn create_context(&mut self) -> Result<(), String> {
        if unsafe { eglBindAPI(EGL_OPENGL_API) } != EGL_TRUE {
            return Err("eglBindAPI(EGL_OPENGL_API) failed".to_owned());
        }
        // Desktop GL 3.3 core (the API libmpv's render API and our shaders use).
        let context_attribs = [
            EGL_CONTEXT_MAJOR_VERSION,
            3,
            EGL_CONTEXT_MINOR_VERSION,
            3,
            EGL_NONE,
        ];
        let context = unsafe {
            eglCreateContext(
                self.display,
                self.config,
                EGL_NO_CONTEXT,
                context_attribs.as_ptr(),
            )
        };
        if context == EGL_NO_CONTEXT {
            return Err(format!("eglCreateContext failed (0x{:x})", unsafe {
                eglGetError()
            }));
        }
        self.context = context;
        self.make_current()
    }

    /// Make the context current. Uses a surfaceless context when the driver
    /// supports it, otherwise a 1×1 pbuffer.
    pub fn make_current(&mut self) -> Result<(), String> {
        let surfaceless_ok = self
            .display_extensions
            .contains("EGL_KHR_surfaceless_context")
            || egl_version_15(&self.version);
        if surfaceless_ok
            && unsafe { eglMakeCurrent(self.display, EGL_NO_SURFACE, EGL_NO_SURFACE, self.context) }
                == EGL_TRUE
        {
            self.surfless = true;
            return Ok(());
        }
        if self.surface == EGL_NO_SURFACE {
            let attribs = [EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE];
            self.surface =
                unsafe { eglCreatePbufferSurface(self.display, self.config, attribs.as_ptr()) };
            if self.surface == EGL_NO_SURFACE {
                return Err(format!("eglCreatePbufferSurface failed (0x{:x})", unsafe {
                    eglGetError()
                }));
            }
        }
        if unsafe { eglMakeCurrent(self.display, self.surface, self.surface, self.context) }
            != EGL_TRUE
        {
            return Err(format!("eglMakeCurrent failed (0x{:x})", unsafe {
                eglGetError()
            }));
        }
        Ok(())
    }

    pub fn is_surfaceless(&self) -> bool {
        self.surfless
    }

    pub fn has_display_extension(&self, name: &str) -> bool {
        self.display_extensions
            .split_whitespace()
            .any(|e| e == name)
    }

    /// Import a dmabuf as an `EGLImage` (not yet a texture).
    pub fn import_dmabuf(&self, desc: &DmaBufDesc) -> Result<EglImage, String> {
        let create = self
            .create_image
            .ok_or("eglCreateImageKHR missing (EGL_EXT_image_dma_buf_import unsupported)")?;
        if !self.has_display_extension("EGL_EXT_image_dma_buf_import") {
            return Err("display lacks EGL_EXT_image_dma_buf_import".to_owned());
        }
        let mut attribs: Vec<EglInt> = Vec::with_capacity(4 + desc.planes.len() * 5);
        attribs.extend_from_slice(&[
            EGL_WIDTH,
            desc.width as EglInt,
            EGL_HEIGHT,
            desc.height as EglInt,
        ]);
        attribs.extend_from_slice(&[EGL_LINUX_DRM_FOURCC_EXT, desc.fourcc as EglInt]);
        for (i, plane) in desc.planes.iter().enumerate() {
            if i >= PLANE_FD.len() {
                return Err(format!("{} planes exceeds EGL's 4", desc.planes.len()));
            }
            attribs.extend_from_slice(&[
                PLANE_FD[i],
                plane.fd,
                PLANE_OFFSET[i],
                plane.offset as EglInt,
                PLANE_PITCH[i],
                plane.stride as EglInt,
                PLANE_MOD_LO[i],
                desc.modifier as u32 as EglInt,
                PLANE_MOD_HI[i],
                (desc.modifier >> 32) as u32 as EglInt,
            ]);
        }
        attribs.push(EGL_NONE);

        let image = unsafe {
            create(
                self.display,
                EGL_NO_CONTEXT,
                EGL_LINUX_DMA_BUF_EXT,
                ptr::null_mut(),
                attribs.as_ptr(),
            )
        };
        let image = EglImage(image);
        if image.is_none() {
            return Err(format!(
                "eglCreateImageKHR failed (0x{:x}) for {}x{} {} modifier {}",
                unsafe { eglGetError() },
                desc.width,
                desc.height,
                crate::render::gbm::fourcc_name(desc.fourcc),
                crate::render::gbm::modifier_name(desc.modifier),
            ));
        }
        Ok(image)
    }

    pub fn destroy_image(&self, image: EglImage) {
        if let Some(destroy) = self.destroy_image {
            if !image.is_none() {
                unsafe { destroy(self.display, image.as_ptr()) };
            }
        }
    }

    /// Formats this GPU can import/export as dmabufs.
    pub fn dmabuf_formats(&self) -> Vec<u32> {
        let Some(query) = self.query_dmabuf_formats else {
            return Vec::new();
        };
        let mut count: EglInt = 0;
        if unsafe { query(self.display, 0, ptr::null_mut(), &mut count) } != EGL_TRUE || count <= 0
        {
            return Vec::new();
        }
        let mut formats = vec![0 as EglInt; count as usize];
        if unsafe { query(self.display, count, formats.as_mut_ptr(), &mut count) } != EGL_TRUE {
            return Vec::new();
        }
        formats.truncate(count.max(0) as usize);
        formats.into_iter().map(|f| f as u32).collect()
    }

    /// `(modifier, external_only)` pairs this GPU supports for `format`.
    ///
    /// `external_only` means the image can be sampled but **not** rendered into
    /// — the difference between "we can import it" and "we can produce it".
    pub fn dmabuf_modifiers(&self, format: u32) -> Vec<(u64, bool)> {
        let Some(query) = self.query_dmabuf_modifiers else {
            return Vec::new();
        };
        let mut count: EglInt = 0;
        if unsafe {
            query(
                self.display,
                format as EglInt,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut count,
            )
        } != EGL_TRUE
            || count <= 0
        {
            return Vec::new();
        }
        let mut modifiers = vec![0u64; count as usize];
        let mut external = vec![0 as EglBoolean; count as usize];
        if unsafe {
            query(
                self.display,
                format as EglInt,
                count,
                modifiers.as_mut_ptr(),
                external.as_mut_ptr(),
                &mut count,
            )
        } != EGL_TRUE
        {
            return Vec::new();
        }
        modifiers.truncate(count.max(0) as usize);
        external.truncate(count.max(0) as usize);
        modifiers
            .into_iter()
            .zip(external)
            .map(|(m, e)| (m, e != 0))
            .collect()
    }
}

/// Config search, widest last: the first set that yields a config wins.
///
/// We only ever render into FBOs, so the surface type does not matter to us —
/// but it matters to the driver's config list. NVIDIA's GBM platform advertises
/// pbuffer configs, Mesa's advertises window (gbm_surface) ones.
fn choose_config(display: EglDisplay) -> Result<(EglConfig, String), String> {
    const SURFACE_TYPES: [(&str, Option<EglInt>); 3] = [
        ("pbuffer", Some(EGL_PBUFFER_BIT)),
        ("window", Some(EGL_WINDOW_BIT)),
        ("any", None),
    ];
    let mut tried = Vec::new();
    for (surface_label, surface_type) in SURFACE_TYPES {
        for (api_label, api) in [
            ("GL|GLES2", EGL_OPENGL_BIT | EGL_OPENGL_ES2_BIT),
            ("GLES2", EGL_OPENGL_ES2_BIT),
        ] {
            let mut attribs = Vec::new();
            if let Some(bits) = surface_type {
                attribs.extend_from_slice(&[EGL_SURFACE_TYPE, bits]);
            }
            attribs.extend_from_slice(&[
                EGL_RENDERABLE_TYPE,
                api,
                EGL_RED_SIZE,
                8,
                EGL_GREEN_SIZE,
                8,
                EGL_BLUE_SIZE,
                8,
                EGL_ALPHA_SIZE,
                8,
                EGL_NONE,
            ]);
            let label = format!("{surface_label}, {api_label}, RGBA8");
            if let Some(config) = fetch(display, &attribs, &label, &mut tried) {
                return Ok((config, label));
            }
        }
    }
    if let Some(config) = fetch(display, &[EGL_NONE], "no filter at all", &mut tried) {
        return Ok((config, "no filter at all".to_owned()));
    }
    Err(format!("no config ({})", tried.join("; ")))
}

fn fetch(
    display: EglDisplay,
    attribs: &[EglInt],
    label: &str,
    tried: &mut Vec<String>,
) -> Option<EglConfig> {
    let mut count: EglInt = 0;
    if unsafe { eglChooseConfig(display, attribs.as_ptr(), ptr::null_mut(), 0, &mut count) }
        != EGL_TRUE
    {
        tried.push(format!("{label}: call failed (0x{:x})", unsafe {
            eglGetError()
        }));
        return None;
    }
    if count <= 0 {
        tried.push(format!("{label}: 0 configs"));
        return None;
    }
    let mut config: EglConfig = ptr::null_mut();
    let mut got: EglInt = 0;
    if unsafe { eglChooseConfig(display, attribs.as_ptr(), &mut config, 1, &mut got) } != EGL_TRUE
        || got == 0
    {
        tried.push(format!("{label}: {count} configs but none fetched"));
        return None;
    }
    Some(config)
}

impl Drop for Egl {
    fn drop(&mut self) {
        unsafe {
            eglMakeCurrent(self.display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
            if self.context != EGL_NO_CONTEXT {
                if self.surface != EGL_NO_SURFACE {
                    eglDestroySurface(self.display, self.surface);
                }
                eglDestroyContext(self.display, self.context);
            }
            eglTerminate(self.display);
        }
    }
}

/// Client extensions; valid even without a display on EGL 1.5/most loaders.
pub fn query_client_extensions() -> String {
    cstr(unsafe { eglQueryString(EGL_NO_DISPLAY, EGL_EXTENSIONS) })
}

fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return "<null>".to_owned();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

fn egl_version_15(version: &str) -> bool {
    let mut parts = version.split(['.', ' ']);
    let major: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor) >= (1, 5)
}

/// Fetch an optional EGL entry point.
fn load<T>(name: &str) -> Option<T> {
    let cname = CString::new(name).ok()?;
    let ptr = unsafe { eglGetProcAddress(cname.as_ptr()) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&ptr) })
    }
}
