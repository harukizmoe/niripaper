//! Video through libmpv's OpenGL render API (`HANDOFF.md` §2/§3, milestone V2).
//!
//! mpv is used as a **library**, not as a program: there is no `mpv` process, no
//! window, no IPC, no config file. It decodes — hardware-accelerated, which is a
//! hard requirement (the test library has 4K60 h264) — and draws the decoded
//! frame into a texture *of ours* (`MPV_RENDER_PARAM_OPENGL_FBO`). Everything
//! after that is the same as the static-image path: the texture is sampled with
//! the parallax offset.
//!
//! The FFI is hand-written, like the EGL/GBM/GL bindings: the surface used here
//! is a handful of functions, and `mpv_render_param` is a tagged union by
//! convention, so a crate would hide exactly the parts that matter.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::os::fd::RawFd;
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use super::gl;

/// Opaque handles. Zero-sized so a raw pointer to them stays a thin pointer.
#[repr(C)]
pub struct MpvHandle {
    _opaque: [u8; 0],
}
#[repr(C)]
pub struct MpvRenderContext {
    _opaque: [u8; 0],
}

/// `mpv_render_param`: a type tag plus a pointer whose meaning depends on it.
#[repr(C)]
struct RenderParam {
    type_: c_int,
    data: *mut c_void,
}

/// `mpv_opengl_init_params` (`render_gl.h`): how libmpv resolves GL entry points.
#[repr(C)]
struct OpenglInitParams {
    get_proc_address: Option<extern "C" fn(*mut c_void, *const c_char) -> *mut c_void>,
    get_proc_address_ctx: *mut c_void,
}

/// `mpv_opengl_fbo`: the render target. `internal_format = 0` lets mpv choose.
#[repr(C)]
struct OpenglFbo {
    fbo: c_int,
    w: c_int,
    h: c_int,
    internal_format: c_int,
}

// `mpv_render_param_type` values (render.h).
const PARAM_API_TYPE: c_int = 1;
const PARAM_OPENGL_INIT_PARAMS: c_int = 2;
const PARAM_OPENGL_FBO: c_int = 3;
/// `MPV_RENDER_UPDATE_FRAME`: a new frame is ready to be drawn.
const UPDATE_FRAME: u64 = 1;

#[link(name = "mpv")]
extern "C" {
    fn mpv_create() -> *mut MpvHandle;
    fn mpv_initialize(ctx: *mut MpvHandle) -> c_int;
    fn mpv_set_option_string(
        ctx: *mut MpvHandle,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;
    fn mpv_command(ctx: *mut MpvHandle, args: *const *const c_char) -> c_int;
    fn mpv_get_property_string(ctx: *mut MpvHandle, name: *const c_char) -> *mut c_char;
    fn mpv_free(data: *mut c_void);
    fn mpv_terminate_destroy(ctx: *mut MpvHandle);
    fn mpv_error_string(code: c_int) -> *const c_char;

    fn mpv_render_context_create(
        result: *mut *mut MpvRenderContext,
        ctx: *mut MpvHandle,
        params: *mut RenderParam,
    ) -> c_int;
    fn mpv_render_context_render(ctx: *mut MpvRenderContext, params: *mut RenderParam) -> c_int;
    fn mpv_render_context_set_update_callback(
        ctx: *mut MpvRenderContext,
        callback: Option<extern "C" fn(*mut c_void)>,
        data: *mut c_void,
    );
    fn mpv_render_context_update(ctx: *mut MpvRenderContext) -> u64;
    fn mpv_render_context_report_swap(ctx: *mut MpvRenderContext);
    fn mpv_render_context_free(ctx: *mut MpvRenderContext);
}

fn error(code: c_int) -> String {
    unsafe {
        let text = mpv_error_string(code);
        if text.is_null() {
            return format!("mpv error {code}");
        }
        format!(
            "mpv error {code}: {}",
            CStr::from_ptr(text).to_string_lossy()
        )
    }
}

fn cstring(text: &str) -> Result<CString, String> {
    CString::new(text).map_err(|e| format!("{text:?} contains a NUL: {e}"))
}

/// Set an option. Must happen before `mpv_initialize`.
fn set_option(ctx: *mut MpvHandle, name: &str, value: &str) -> Result<(), String> {
    let name = cstring(name)?;
    let value = cstring(value)?;
    let code = unsafe { mpv_set_option_string(ctx, name.as_ptr(), value.as_ptr()) };
    if code < 0 {
        return Err(format!("setting mpv option {name:?}: {}", error(code)));
    }
    Ok(())
}

/// libmpv's entry point for GL symbols. It must be the same GL context as the
/// one we render with, which is why this is a plain `eglGetProcAddress`.
extern "C" fn get_proc_address(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    if name.is_null() {
        return ptr::null_mut();
    }
    let name = unsafe { CStr::from_ptr(name) };
    match name.to_str() {
        Ok(name) => super::egl::get_proc_address(name),
        Err(_) => ptr::null_mut(),
    }
}

/// What mpv's update callback is handed: the flag the render path checks, and an
/// eventfd that wakes the daemon's `poll()`. Without the fd the loop would have
/// to poll mpv, which would cost the zero-idle design (§6, M0b criterion ④).
struct Signal {
    pending: AtomicBool,
    fd: RawFd,
}

extern "C" fn on_update(data: *mut c_void) {
    let Some(signal) = (unsafe { (data as *mut Signal).as_ref() }) else {
        return;
    };
    signal.pending.store(true, Ordering::Release);
    // Async-signal-safe enough: one write to an eventfd. EAGAIN means a wakeup
    // is already pending, which is exactly what we want to know.
    let one: u64 = 1;
    unsafe {
        libc::write(
            signal.fd,
            (&one as *const u64).cast(),
            std::mem::size_of::<u64>(),
        );
    }
}

/// A decoding, hardware-accelerated video, drawn into a texture we own.
pub struct Video {
    ctx: *mut MpvHandle,
    render: *mut MpvRenderContext,
    texture: u32,
    fbo: u32,
    width: u32,
    height: u32,
    /// Set by mpv from another thread when a frame is ready. Boxed so the
    /// callback's pointer stays valid for as long as the context lives.
    signal: Box<Signal>,
    /// Frames drawn so far, for the probe's log.
    pub frames: u64,
}

impl Video {
    /// `width`/`height` are the *render target's* size, not the video's: mpv
    /// scales into whatever we give it, which is what lets the canvas be the
    /// output size times `scale`.
    /// `fps` caps the frame rate mpv produces (`0` keeps the source's). A
    /// wallpaper does not need 60 fps: niri's layer frame callbacks come at
    /// 60 Hz anyway, and every drawn frame also costs a backdrop re-blur.
    pub fn new(path: &Path, width: u32, height: u32, fps: u32) -> Result<Self, String> {
        let ctx = unsafe { mpv_create() };
        if ctx.is_null() {
            return Err("mpv_create failed".to_owned());
        }
        // Everything that matters for a wallpaper: no config of the user's, no
        // scripts, no audio, loop forever, and decode in hardware.
        for (name, value) in [
            ("vo", "libmpv"),
            ("hwdec", "auto-safe"),
            ("loop-file", "inf"),
            ("mute", "yes"),
            ("aid", "no"),
            ("config", "no"),
            ("load-scripts", "no"),
            ("terminal", "no"),
            ("input-default-bindings", "no"),
            ("osc", "no"),
        ] {
            set_option(ctx, name, value)?;
        }
        if fps > 0 {
            // Decimate rather than resample: for a loop this only has to look
            // right, and dropping frames costs nothing.
            set_option(ctx, "vf", &format!("fps={fps}:round=down"))?;
        }
        let code = unsafe { mpv_initialize(ctx) };
        if code < 0 {
            unsafe { mpv_terminate_destroy(ctx) };
            return Err(format!("mpv_initialize: {}", error(code)));
        }

        // A plain RGBA texture and an FBO to render into. Not a dmabuf: this is
        // an intermediate, and the presentation path samples it.
        let texture = gl::gen_texture();
        gl::bind_texture(gl::GL_TEXTURE_2D, texture);
        gl::tex_image_2d_rgba(width, height);
        gl::bind_texture(gl::GL_TEXTURE_2D, 0);
        let fbo = gl::gen_framebuffer();
        gl::bind_framebuffer(gl::GL_FRAMEBUFFER, fbo);
        gl::framebuffer_texture_2d(
            gl::GL_FRAMEBUFFER,
            gl::GL_COLOR_ATTACHMENT0,
            gl::GL_TEXTURE_2D,
            texture,
        );
        let status = gl::framebuffer_status();
        gl::bind_framebuffer(gl::GL_FRAMEBUFFER, 0);
        if status != gl::GL_FRAMEBUFFER_COMPLETE {
            unsafe { mpv_terminate_destroy(ctx) };
            return Err(format!("video render target incomplete (0x{status:x})"));
        }

        let api_type = cstring("opengl")?;
        let mut init = OpenglInitParams {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: ptr::null_mut(),
        };
        let mut params = [
            RenderParam {
                type_: PARAM_API_TYPE,
                data: api_type.as_ptr() as *mut c_void,
            },
            RenderParam {
                type_: PARAM_OPENGL_INIT_PARAMS,
                data: (&mut init as *mut OpenglInitParams).cast(),
            },
            RenderParam {
                type_: 0,
                data: ptr::null_mut(),
            },
        ];
        let mut render: *mut MpvRenderContext = ptr::null_mut();
        let code = unsafe { mpv_render_context_create(&mut render, ctx, params.as_mut_ptr()) };
        if code < 0 {
            unsafe { mpv_terminate_destroy(ctx) };
            return Err(format!("mpv_render_context_create: {}", error(code)));
        }

        let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if fd < 0 {
            unsafe {
                mpv_render_context_free(render);
                mpv_terminate_destroy(ctx);
            }
            return Err(format!("eventfd: {}", std::io::Error::last_os_error()));
        }
        let signal = Box::new(Signal {
            pending: AtomicBool::new(true),
            fd,
        });
        let data = (&*signal as *const Signal) as *mut c_void;
        unsafe { mpv_render_context_set_update_callback(render, Some(on_update), data) };

        let mut video = Self {
            ctx,
            render,
            texture,
            fbo,
            width,
            height,
            signal,
            frames: 0,
        };
        video.load(path)?;
        Ok(video)
    }

    fn load(&mut self, path: &Path) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| format!("{} is not valid UTF-8", path.display()))?;
        let command = cstring("loadfile")?;
        let argument = cstring(path)?;
        let replace = cstring("replace")?;
        let args = [
            command.as_ptr(),
            argument.as_ptr(),
            replace.as_ptr(),
            ptr::null(),
        ];
        let code = unsafe { mpv_command(self.ctx, args.as_ptr()) };
        if code < 0 {
            return Err(format!("loadfile {path}: {}", error(code)));
        }
        Ok(())
    }

    /// Decode and draw the newest frame into our texture. Returns whether a new
    /// frame was drawn (and therefore whether the wallpaper moved).
    pub fn render(&mut self) -> Result<bool, String> {
        if !self.signal.pending.swap(false, Ordering::AcqRel) {
            return Ok(false);
        }
        let flags = unsafe { mpv_render_context_update(self.render) };
        if flags & UPDATE_FRAME == 0 {
            return Ok(false);
        }
        let mut fbo = OpenglFbo {
            fbo: self.fbo as c_int,
            w: self.width as c_int,
            h: self.height as c_int,
            internal_format: 0,
        };
        let mut params = [
            RenderParam {
                type_: PARAM_OPENGL_FBO,
                data: (&mut fbo as *mut OpenglFbo).cast(),
            },
            RenderParam {
                type_: 0,
                data: ptr::null_mut(),
            },
        ];
        let code = unsafe { mpv_render_context_render(self.render, params.as_mut_ptr()) };
        if code < 0 {
            return Err(format!("mpv_render_context_render: {}", error(code)));
        }
        // Required after the frame has been shown; skipping it stalls mpv's
        // frame pacing (`HANDOFF.md` §3).
        unsafe { mpv_render_context_report_swap(self.render) };
        self.frames += 1;
        Ok(true)
    }

    /// The texture mpv draws into. Sample this with the parallax offset.
    pub fn texture(&self) -> u32 {
        self.texture
    }

    /// The eventfd mpv's update callback writes to. Watch this instead of
    /// polling: readable means "call [`Video::render`]".
    pub fn fd(&self) -> RawFd {
        self.signal.fd
    }

    /// Clear a pending wakeup. Reads are non-blocking, and one read drains the
    /// counter.
    pub fn drain(&self) {
        let mut counter: u64 = 0;
        unsafe {
            libc::read(
                self.signal.fd,
                (&mut counter as *mut u64).cast(),
                std::mem::size_of::<u64>(),
            );
        }
    }

    /// The framebuffer mpv draws into. `mpv_render_context_render` manages GL
    /// state, so anything reading the result (a probe, a readback) has to bind
    /// this again first — the default framebuffer is not it.
    pub fn fbo(&self) -> u32 {
        self.fbo
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// What is actually decoding — `"vaapi"`, `"nvdec"`, … or `"no"` when it fell
    /// back to software. The point of querying instead of trusting `hwdec=auto-safe`.
    pub fn hwdec(&self) -> String {
        self.property("hwdec-current")
    }

    /// The video's own size, as reported by mpv (the render target may differ).
    pub fn video_size(&self) -> Option<(u64, u64)> {
        let width = self.property("width").parse::<u64>().ok()?;
        let height = self.property("height").parse::<u64>().ok()?;
        Some((width, height))
    }

    fn property(&self, name: &str) -> String {
        let Ok(name) = cstring(name) else {
            return String::new();
        };
        let value = unsafe { mpv_get_property_string(self.ctx, name.as_ptr()) };
        if value.is_null() {
            return String::new();
        }
        let text = unsafe { CStr::from_ptr(value).to_string_lossy().into_owned() };
        unsafe { mpv_free(value.cast()) };
        text
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        unsafe {
            // Close the wakeup fd last: `signal` must outlive the callback.
            let fd = self.signal.fd;
            // The callback must not fire after `pending` is gone.
            mpv_render_context_set_update_callback(self.render, None, ptr::null_mut());
            mpv_render_context_free(self.render);
            mpv_terminate_destroy(self.ctx);
            gl::delete_framebuffer(self.fbo);
            gl::delete_texture(self.texture);
            libc::close(fd);
        }
    }
}
