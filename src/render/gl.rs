//! The GL subset the renderer needs, plus a test pattern.
//!
//! Linked against `libGL.so`, which is GLVND: it dispatches to whichever vendor
//! EGL made current. That is why the same binary works whether the context came
//! from `libEGL_nvidia` or `libEGL_mesa`.

use std::ffi::{c_void, CStr, CString};
use std::ptr;

pub const GL_NO_ERROR: u32 = 0;
pub const GL_VENDOR: u32 = 0x1F00;
pub const GL_RENDERER: u32 = 0x1F01;
pub const GL_VERSION: u32 = 0x1F02;
pub const GL_EXTENSIONS: u32 = 0x1F03;

pub const GL_TEXTURE_2D: u32 = 0x0DE1;
pub const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
pub const GL_FRAMEBUFFER: u32 = 0x8D40;
pub const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
pub const GL_TRIANGLES: u32 = 0x0004;
pub const GL_VERTEX_SHADER: u32 = 0x8B31;
pub const GL_FRAGMENT_SHADER: u32 = 0x8B30;
pub const GL_COMPILE_STATUS: u32 = 0x8B81;
pub const GL_LINK_STATUS: u32 = 0x8B82;

#[link(name = "GL")]
#[allow(non_snake_case)]
extern "C" {
    fn glGetString(name: u32) -> *const u8;
    fn glGetError() -> u32;
    fn glViewport(x: i32, y: i32, w: i32, h: i32);
    fn glFinish();

    fn glGenTextures(n: i32, textures: *mut u32);
    fn glBindTexture(target: u32, texture: u32);
    fn glDeleteTextures(n: i32, textures: *const u32);
    fn glEGLImageTargetTexture2DOES(target: u32, image: *mut c_void);

    fn glGenFramebuffers(n: i32, framebuffers: *mut u32);
    fn glBindFramebuffer(target: u32, framebuffer: u32);
    fn glFramebufferTexture2D(
        target: u32,
        attachment: u32,
        textarget: u32,
        texture: u32,
        level: i32,
    );
    fn glCheckFramebufferStatus(target: u32) -> u32;
    fn glDeleteFramebuffers(n: i32, framebuffers: *const u32);

    fn glCreateShader(kind: u32) -> u32;
    fn glShaderSource(shader: u32, count: i32, strings: *const *const i8, lengths: *const i32);
    fn glCompileShader(shader: u32);
    fn glGetShaderiv(shader: u32, pname: u32, params: *mut i32);
    fn glGetShaderInfoLog(shader: u32, max: i32, len: *mut i32, log: *mut i8);
    fn glDeleteShader(shader: u32);
    fn glCreateProgram() -> u32;
    fn glAttachShader(program: u32, shader: u32);
    fn glLinkProgram(program: u32);
    fn glGetProgramiv(program: u32, pname: u32, params: *mut i32);
    fn glGetProgramInfoLog(program: u32, max: i32, len: *mut i32, log: *mut i8);
    fn glUseProgram(program: u32);
    fn glDeleteProgram(program: u32);
    fn glGenVertexArrays(n: i32, arrays: *mut u32);
    fn glBindVertexArray(array: u32);
    fn glDeleteVertexArrays(n: i32, arrays: *const u32);
    fn glDrawArrays(mode: u32, first: i32, count: i32);
}

/// Create one texture name.
pub fn gen_texture() -> u32 {
    let mut name = 0;
    unsafe { glGenTextures(1, &mut name) };
    name
}

pub fn bind_texture(target: u32, texture: u32) {
    unsafe { glBindTexture(target, texture) };
}

pub fn delete_texture(texture: u32) {
    unsafe { glDeleteTextures(1, &texture) };
}

/// Bind an `EGLImage` to the currently bound texture.
pub fn egl_image_target_texture(target: u32, image: super::egl::EglImage) {
    unsafe { glEGLImageTargetTexture2DOES(target, image.as_ptr()) };
}

pub fn gen_framebuffer() -> u32 {
    let mut name = 0;
    unsafe { glGenFramebuffers(1, &mut name) };
    name
}

pub fn bind_framebuffer(target: u32, framebuffer: u32) {
    unsafe { glBindFramebuffer(target, framebuffer) };
}

pub fn framebuffer_texture_2d(target: u32, attachment: u32, tex_target: u32, texture: u32) {
    unsafe { glFramebufferTexture2D(target, attachment, tex_target, texture, 0) };
}

pub fn framebuffer_status() -> u32 {
    unsafe { glCheckFramebufferStatus(GL_FRAMEBUFFER) }
}

pub fn delete_framebuffer(framebuffer: u32) {
    unsafe { glDeleteFramebuffers(1, &framebuffer) };
}

pub fn viewport(width: u32, height: u32) {
    unsafe { glViewport(0, 0, width as i32, height as i32) };
}

/// Block until the GPU has finished what we queued.
pub fn finish() {
    unsafe { glFinish() };
}

/// A shader program plus the empty VAO core-profile GL insists on.
pub struct Renderer {
    program: u32,
    vao: u32,
}

impl Renderer {
    pub fn new() -> Result<Self, String> {
        let vertex = compile(GL_VERTEX_SHADER, VERTEX_SRC)?;
        let fragment = compile(GL_FRAGMENT_SHADER, FRAGMENT_SRC)?;
        let program = unsafe { glCreateProgram() };
        unsafe {
            glAttachShader(program, vertex);
            glAttachShader(program, fragment);
            glLinkProgram(program);
        }
        let mut status = 0;
        unsafe { glGetProgramiv(program, GL_LINK_STATUS, &mut status) };
        if status == 0 {
            let log =
                info_log(|max, len, buf| unsafe { glGetProgramInfoLog(program, max, len, buf) });
            return Err(format!("program link failed: {log}"));
        }
        unsafe {
            glDeleteShader(vertex);
            glDeleteShader(fragment);
        }
        let mut vao = 0;
        unsafe { glGenVertexArrays(1, &mut vao) };
        Ok(Self { program, vao })
    }

    /// Draw the test pattern over the whole current framebuffer.
    pub fn draw_pattern(&self) {
        unsafe {
            glBindVertexArray(self.vao);
            glUseProgram(self.program);
            glDrawArrays(GL_TRIANGLES, 0, 3);
            glBindVertexArray(0);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // The owner must have made the context current.
        unsafe {
            glDeleteProgram(self.program);
            glDeleteVertexArrays(1, &self.vao);
        }
    }
}

/// Four vertical bands plus a white frame: unambiguous in a screenshot, and
/// comparable enough that a blurred backdrop copy is obvious.
const FRAGMENT_SRC: &str = r#"#version 330 core
in vec2 uv;
out vec4 color;
void main() {
    int band = int(clamp(uv.x, 0.0, 0.999) * 4.0);
    vec3 c = band == 0 ? vec3(1.0, 0.0, 0.0)
           : band == 1 ? vec3(0.0, 1.0, 0.0)
           : band == 2 ? vec3(0.0, 0.0, 1.0)
                       : vec3(1.0, 1.0, 0.0);
    c *= 0.35 + 0.65 * uv.y;
    float edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    if (edge < 0.002) {
        c = vec3(1.0);
    }
    color = vec4(c, 1.0);
}
"#;

const VERTEX_SRC: &str = r#"#version 330 core
out vec2 uv;
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

fn compile(kind: u32, source: &str) -> Result<u32, String> {
    let shader = unsafe { glCreateShader(kind) };
    let c_source = CString::new(source).map_err(|e| e.to_string())?;
    let ptr = c_source.as_ptr();
    unsafe {
        glShaderSource(shader, 1, &ptr, ptr::null());
        glCompileShader(shader);
    }
    let mut status = 0;
    unsafe { glGetShaderiv(shader, GL_COMPILE_STATUS, &mut status) };
    if status == 0 {
        let log = info_log(|max, len, buf| unsafe { glGetShaderInfoLog(shader, max, len, buf) });
        return Err(format!("shader compile failed: {log}"));
    }
    Ok(shader)
}

fn info_log(fetch: impl Fn(i32, *mut i32, *mut i8)) -> String {
    let mut buf = vec![0i8; 1024];
    let mut len = 0;
    fetch(buf.len() as i32, &mut len, buf.as_mut_ptr());
    let len = len.clamp(0, buf.len() as i32) as usize;
    let bytes: Vec<u8> = buf[..len].iter().map(|b| *b as u8).collect();
    String::from_utf8_lossy(&bytes).trim().to_owned()
}

/// Last GL error, or `None` when the pipeline is clean.
pub fn last_error() -> Option<u32> {
    let e = unsafe { glGetError() };
    if e == GL_NO_ERROR {
        None
    } else {
        Some(e)
    }
}

pub fn string(name: u32) -> String {
    let ptr = unsafe { glGetString(name) };
    if ptr.is_null() {
        return "<null>".to_owned();
    }
    unsafe { CStr::from_ptr(ptr as *const i8) }
        .to_string_lossy()
        .into_owned()
}
