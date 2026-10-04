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
pub const GL_TEXTURE0: u32 = 0x84C0;
pub const GL_TEXTURE_MIN_FILTER: u32 = 0x2801;
pub const GL_TEXTURE_MAG_FILTER: u32 = 0x2800;
pub const GL_TEXTURE_WRAP_S: u32 = 0x2802;
pub const GL_TEXTURE_WRAP_T: u32 = 0x2803;
pub const GL_LINEAR: u32 = 0x2601;
pub const GL_CLAMP_TO_EDGE: u32 = 0x812F;
pub const GL_RGB: u32 = 0x1907;
pub const GL_RGB8: u32 = 0x8051;
pub const GL_UNSIGNED_BYTE: u32 = 0x1401;
pub const GL_UNPACK_ALIGNMENT: u32 = 0x0CF5;
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
    fn glActiveTexture(texture: u32);
    fn glTexImage2D(
        target: u32,
        level: i32,
        internal_format: i32,
        width: i32,
        height: i32,
        border: i32,
        format: u32,
        kind: u32,
        pixels: *const c_void,
    );
    fn glPixelStorei(pname: u32, param: i32);
    fn glTexParameteri(target: u32, pname: u32, param: i32);
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
    fn glGetUniformLocation(program: u32, name: *const i8) -> i32;
    fn glUniform1f(location: i32, v0: f32);
    fn glUniform1i(location: i32, v0: i32);
    fn glUniform2f(location: i32, v0: f32, v1: f32);
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

/// Which test pattern to draw. Both live in canvas space, so they slide with
/// the parallax offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// Four colour bands: unambiguous in a screenshot, used to check that a
    /// layer really is inside the backdrop.
    Bands,
    /// Pseudo-random blocks with a vertical gradient. §8 needs *unique*
    /// structure for the pixel-displacement correlation: a repeating pattern
    /// aliases and reports "no movement".
    Blocks,
}

/// An RGB texture, sized to the canvas.
#[derive(Debug)]
pub struct Texture {
    pub id: u32,
    pub width: u32,
    pub height: u32,
}

impl Texture {
    /// Upload tightly packed 8-bit RGB pixels.
    pub fn from_rgb(width: u32, height: u32, pixels: &[u8]) -> Self {
        assert_eq!(
            pixels.len(),
            (width as usize) * (height as usize) * 3,
            "pixel buffer does not match {width}x{height} RGB"
        );
        let id = gen_texture();
        unsafe {
            glBindTexture(GL_TEXTURE_2D, id);
            // Rows are tightly packed; the default 4-byte alignment would
            // corrupt odd widths.
            glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as i32);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as i32);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as i32);
            glTexImage2D(
                GL_TEXTURE_2D,
                0,
                GL_RGB8 as i32,
                width as i32,
                height as i32,
                0,
                GL_RGB,
                GL_UNSIGNED_BYTE,
                pixels.as_ptr().cast(),
            );
            glBindTexture(GL_TEXTURE_2D, 0);
        }
        Self { id, width, height }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        // Requires the owning context to be current.
        delete_texture(self.id);
    }
}

/// What to draw: a real wallpaper, or the procedural pattern when there is none.
#[derive(Debug, Clone, Copy)]
pub enum Content<'a> {
    Wallpaper(&'a Texture),
    Pattern(Pattern),
}

/// Where the canvas sits relative to the screen.
#[derive(Debug, Clone, Copy)]
pub struct View {
    /// Screen size in pixels.
    pub screen: (f32, f32),
    /// Canvas scale (§4.1 `scale`): the canvas is `screen * scale`.
    pub scale: f32,
    /// Offset of the canvas relative to centred, in pixels
    /// ([`crate::motion::offset_px`]).
    pub offset: (f32, f32),
    pub pattern: Pattern,
}

impl View {
    /// A still, unzoomed view — the M0b probe's test pattern.
    pub fn flat(screen: (f32, f32), pattern: Pattern) -> Self {
        Self {
            screen,
            scale: 1.0,
            offset: (0.0, 0.0),
            pattern,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Uniforms {
    screen: i32,
    scale: i32,
    offset: i32,
    pattern: i32,
    wallpaper: i32,
    has_wallpaper: i32,
}

/// A shader program plus the empty VAO core-profile GL insists on.
pub struct Renderer {
    program: u32,
    vao: u32,
    uniforms: Uniforms,
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
        let uniforms = Uniforms {
            screen: uniform(program, "u_screen"),
            scale: uniform(program, "u_scale"),
            offset: uniform(program, "u_offset"),
            pattern: uniform(program, "u_pattern"),
            wallpaper: uniform(program, "u_wallpaper"),
            has_wallpaper: uniform(program, "u_has_wallpaper"),
        };
        Ok(Self {
            program,
            vao,
            uniforms,
        })
    }

    /// Draw the pattern over the whole current framebuffer.
    pub fn draw(&self, view: View, content: Content<'_>) {
        unsafe {
            glBindVertexArray(self.vao);
            glUseProgram(self.program);
            glUniform2f(self.uniforms.screen, view.screen.0, view.screen.1);
            glUniform1f(self.uniforms.scale, view.scale);
            glUniform2f(self.uniforms.offset, view.offset.0, view.offset.1);
            match content {
                Content::Wallpaper(texture) => {
                    glActiveTexture(GL_TEXTURE0);
                    glBindTexture(GL_TEXTURE_2D, texture.id);
                    glUniform1i(self.uniforms.wallpaper, 0);
                    glUniform1i(self.uniforms.has_wallpaper, 1);
                }
                Content::Pattern(pattern) => {
                    glUniform1i(
                        self.uniforms.pattern,
                        if pattern == Pattern::Blocks { 1 } else { 0 },
                    );
                    glUniform1i(self.uniforms.has_wallpaper, 0);
                }
            }
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

/// The pattern is evaluated in *canvas* space, so the offset slides it.
///
/// `Blocks` is a hash of the block coordinate: unique structure everywhere,
/// which is what the §8 displacement measurement needs.
const FRAGMENT_SRC: &str = r#"#version 330 core
in vec2 uv;
uniform vec2 u_screen;
uniform float u_scale;
uniform vec2 u_offset;
uniform int u_pattern;
uniform sampler2D u_wallpaper;
uniform int u_has_wallpaper;
out vec4 color;

float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

void main() {
    vec2 canvas = u_screen * u_scale;
    vec2 origin = (canvas - u_screen) * 0.5 + u_offset;
    vec2 c = (uv * u_screen + origin) / canvas;

    if (u_has_wallpaper == 1) {
        // The texture is already canvas-sized and cover-cropped, so this is a
        // straight 1:1 lookup.
        color = vec4(texture(u_wallpaper, c).rgb, 1.0);
        return;
    }

    vec3 base;
    if (u_pattern == 1) {
        vec2 cell = floor(c * vec2(96.0, 54.0));
        float h = hash(cell);
        base = vec3(h, fract(h * 7.13), fract(h * 13.7));
    } else {
        int band = int(clamp(c.x, 0.0, 0.999) * 4.0);
        base = band == 0 ? vec3(1.0, 0.0, 0.0)
             : band == 1 ? vec3(0.0, 1.0, 0.0)
             : band == 2 ? vec3(0.0, 0.0, 1.0)
                         : vec3(1.0, 1.0, 0.0);
    }
    base *= 0.35 + 0.65 * c.y;

    // The canvas edge, visible at the extremes of the travel.
    float edge = min(min(c.x, 1.0 - c.x), min(c.y, 1.0 - c.y));
    if (edge < 0.002) {
        base = vec3(1.0);
    }
    color = vec4(base, 1.0);
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

fn uniform(program: u32, name: &str) -> i32 {
    let c_name = CString::new(name).expect("no interior nul");
    unsafe { glGetUniformLocation(program, c_name.as_ptr()) }
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
