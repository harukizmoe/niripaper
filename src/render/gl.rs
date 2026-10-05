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
pub const GL_TEXTURE1: u32 = 0x84C1;
pub const GL_TEXTURE_MIN_FILTER: u32 = 0x2801;
pub const GL_TEXTURE_MAG_FILTER: u32 = 0x2800;
pub const GL_TEXTURE_WRAP_S: u32 = 0x2802;
pub const GL_TEXTURE_WRAP_T: u32 = 0x2803;
pub const GL_LINEAR: u32 = 0x2601;
pub const GL_CLAMP_TO_EDGE: u32 = 0x812F;
pub const GL_RGB: u32 = 0x1907;
pub const GL_RGB8: u32 = 0x8051;
pub const GL_RGBA8: u32 = 0x8058;
pub const GL_RGBA: u32 = 0x1908;
pub const GL_PACK_ALIGNMENT: u32 = 0x0D05;
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
    fn glCopyTexSubImage2D(
        target: u32,
        level: i32,
        xoffset: i32,
        yoffset: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    );
    fn glReadPixels(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        format: u32,
        kind: u32,
        pixels: *mut c_void,
    );
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

/// The parameters every 2D texture here wants.
///
/// The min filter has to be a non-mipmapping one: the default is
/// `NEAREST_MIPMAP_LINEAR`, which makes a texture with only level 0
/// **incomplete** — and an incomplete texture samples as black, with no GL error
/// anywhere to say why. `CLAMP_TO_EDGE` because the transition's lookups leave
/// [0, 1] on purpose (a slide, a zoom) and wrapping would repeat the frame.
fn set_2d_parameters() {
    unsafe {
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as i32);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as i32);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as i32);
    }
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
/// Allocate an empty RGBA8 texture at `width`×`height`. Used for render targets
/// that something else draws into (libmpv's video output).
pub fn tex_image_2d_rgba(width: u32, height: u32) {
    unsafe {
        glTexImage2D(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as i32,
            width as i32,
            height as i32,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            std::ptr::null(),
        );
        set_2d_parameters();
    }
}

/// Read the bound framebuffer back as tightly packed RGB. Probing only: it is a
/// synchronous round trip.
pub fn read_pixels_rgb(width: u32, height: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    unsafe {
        glPixelStorei(GL_PACK_ALIGNMENT, 1);
        glReadPixels(
            0,
            0,
            width as i32,
            height as i32,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            rgba.as_mut_ptr().cast(),
        );
    }
    rgba.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect()
}

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
    /// A texture something else draws into (libmpv's video output). Canvas-sized
    /// like `Wallpaper`, so the shader samples both the same way.
    Video(u32),
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
    previous: i32,
    blend: i32,
    effect: i32,
    progress: i32,
    softness: i32,
    center: i32,
    direction: i32,
    params: i32,
}

/// What the shader needs to blend the previous frame in.
///
/// Plain data because this *is* the shader's contract and nothing else. It comes
/// from [`transition::Transition`](crate::render::transition::Transition).
#[derive(Debug, Clone, Copy)]
pub struct Blend {
    /// The snapshot to blend from.
    pub previous: u32,
    /// `Effect::index`.
    pub effect: i32,
    /// Eased progress; past 1 when overshoot is allowed.
    pub progress: f32,
    pub softness: f32,
    pub center: (f32, f32),
    pub direction: (f32, f32),
    /// `(stripes, cell)` — read only by the effects that have a use for them.
    pub params: (f32, f32),
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
            previous: uniform(program, "u_previous"),
            blend: uniform(program, "u_blend"),
            effect: uniform(program, "u_effect"),
            progress: uniform(program, "u_progress"),
            softness: uniform(program, "u_softness"),
            center: uniform(program, "u_center"),
            direction: uniform(program, "u_direction"),
            params: uniform(program, "u_params"),
        };
        Ok(Self {
            program,
            vao,
            uniforms,
        })
    }

    /// Draw the pattern over the whole current framebuffer. `blend` is the
    /// wallpaper transition in flight, if there is one.
    pub fn draw(&self, view: View, content: Content<'_>, blend: Option<&Blend>) {
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
                Content::Video(texture) => {
                    glActiveTexture(GL_TEXTURE0);
                    glBindTexture(GL_TEXTURE_2D, texture);
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
            match blend {
                Some(blend) => {
                    glActiveTexture(GL_TEXTURE1);
                    glBindTexture(GL_TEXTURE_2D, blend.previous);
                    glUniform1i(self.uniforms.previous, 1);
                    glUniform1i(self.uniforms.blend, 1);
                    glUniform1i(self.uniforms.effect, blend.effect);
                    glUniform1f(self.uniforms.progress, blend.progress);
                    glUniform1f(self.uniforms.softness, blend.softness);
                    glUniform2f(self.uniforms.center, blend.center.0, blend.center.1);
                    glUniform2f(
                        self.uniforms.direction,
                        blend.direction.0,
                        blend.direction.1,
                    );
                    glUniform2f(self.uniforms.params, blend.params.0, blend.params.1);
                }
                // Nothing to blend from. Say so explicitly rather than leaning on
                // a sentinel progress: an unbound sampler reads undefined memory.
                None => glUniform1i(self.uniforms.blend, 0),
            }
            glDrawArrays(GL_TRIANGLES, 0, 3);
            glActiveTexture(GL_TEXTURE0);
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
// The wallpaper transition: the previous *rendered* frame, and how to bring the
// new content in. `uv` is the screen coordinate, so any pair of sources (still,
// video, pattern) blends identically — no second decoder, no per-source case.
uniform sampler2D u_previous;
uniform int u_blend;
uniform int u_effect;
uniform float u_progress;
uniform float u_softness;
uniform vec2 u_center;
uniform vec2 u_direction;
uniform vec2 u_params;
out vec4 color;

float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

// A transition between the previous frame and this one.
//
// Fills in the mix amount and the two lookups, so every effect ends at the same
// `mix`: a mask, a pair of displacements, or both. Effects may only move the
// *new* side or cut the old one away — the old side is a frozen snapshot, so
// animating it would be animating a photograph.
//
// `u_effect` numbering is `Effect::index` in render/transition.rs; the two have
// to change together.
void transition(out float mask, out vec2 old_uv, out vec2 new_uv) {
    float t = u_progress;
    mask = clamp(t, 0.0, 1.0);
    old_uv = uv;
    new_uv = uv;

    // Distances are measured in aspect-corrected space, or a circle comes out an
    // ellipse on a 16:9 screen.
    float aspect = u_screen.x / u_screen.y;
    vec2 p = (uv - u_center) * vec2(aspect, 1.0);

    // The moving edge's half-width. Softness is a fraction of the screen: the
    // 0.3 default is a visible gradient, 1 is most of the way across.
    float edge = 0.01 + u_softness * 0.25;
    // The sweep runs from -edge to 1+edge, so t=0 reveals nothing and t=1 leaves
    // no edge on screen.
    float sweep = t * (1.0 + 2.0 * edge) - edge;

    if (u_effect == 2) {
        // Dissolve: grains of the new image appear all over at once.
        float grain = hash(floor(uv * u_screen / 3.0));
        mask = smoothstep(t - u_softness, t + u_softness, grain);
    } else if (u_effect == 3) {
        // Wipe: a straight edge sweeping along u_direction.
        float along = dot(uv - 0.5, u_direction) + 0.5;
        mask = 1.0 - smoothstep(sweep - edge, sweep + edge, along);
    } else if (u_effect == 4) {
        // Stripes: the same edge, but each band leaves at its own moment.
        float along = dot(uv - 0.5, u_direction) + 0.5;
        vec2 across = vec2(-u_direction.y, u_direction.x);
        float band = floor((dot(uv - 0.5, across) + 0.5) * max(u_params.x, 1.0));
        float stagger = 0.45;
        float local = clamp((t - hash(vec2(band, 3.0)) * stagger) / (1.0 - stagger), 0.0, 1.0);
        float band_sweep = local * (1.0 + 2.0 * edge) - edge;
        mask = 1.0 - smoothstep(band_sweep - edge, band_sweep + edge, along);
    } else if (u_effect == 5 || u_effect == 6) {
        // Iris, and the portal below it: a hole opening at u_center.
        // `p` spans half the screen either way, so the farthest corner is half
        // the diagonal — the full diagonal would have the circle cover
        // everything a third of the way in.
        float reach = 0.5 * length(vec2(aspect, 1.0));
        float radius = sweep * reach;
        mask = 1.0 - smoothstep(radius - edge, radius + edge, length(p));
        if (u_effect == 6) {
            // Portal: the old frame is pushed outward as the hole opens, so you
            // move *through* it rather than watch it get cut away. The new side
            // is live, which is the point: a video arrives already moving.
            old_uv = u_center + (uv - u_center) / (1.0 + 0.35 * clamp(t, 0.0, 1.0));
        }
    } else if (u_effect == 7) {
        // Honeycomb: hexagonal cells, each opening in its own order.
        //
        // The cell is found by *cube* rounding — the standard hex-grid
        // algorithm. Rounding in a sheared space, or taking the nearer of two
        // offset square lattices, both give the nearest point under a box
        // metric, whose cells are parallelograms and rectangles. Cube rounding
        // is the nearest under the hex grid's own metric, so the cells are the
        // hexagons they look like.
        float radius = max(u_params.y, 0.01);
        // Axial coordinates for pointy-top hexagons, then cube: x + y + z = 0.
        vec2 axial = vec2(p.x / (1.7320508 * radius), p.y / (1.5 * radius));
        vec3 cube = vec3(axial.x, -axial.x - axial.y, axial.y);
        vec3 rounded = floor(cube + 0.5);
        vec3 delta = abs(rounded - cube);
        // Snap back to the plane: the component that moved least is the one
        // that has to give.
        if (delta.x > delta.y && delta.x > delta.z) {
            rounded.x = -rounded.y - rounded.z;
        } else if (delta.y > delta.z) {
            rounded.y = -rounded.x - rounded.z;
        } else {
            rounded.z = -rounded.x - rounded.y;
        }
        float offset = hash(vec2(rounded.x, rounded.z));
        float stagger = 0.6;
        float local = clamp((t - offset * stagger) / (1.0 - stagger), 0.0, 1.0);
        mask = smoothstep(0.5 - edge, 0.5 + edge, local);
    } else if (u_effect == 8) {
        // Zoom: the new image arrives magnified and settles. This is where an
        // overshooting curve is visible — t past 1 pushes it the other way.
        new_uv = u_center + (uv - u_center) / (1.0 + 0.25 * (1.0 - t));
    } else if (u_effect == 9) {
        // Slide: the new image comes in from the far side, the old one leaves
        // toward u_direction. Both move with the edge — that is what makes it a
        // push rather than a wipe over stationary images.
        old_uv = uv - u_direction * t;
        new_uv = uv + u_direction * (1.0 - t);
        // A push has no soft edge to give. The two frames are exactly adjacent —
        // the old is defined where it has not slid off, the new where it has
        // arrived — so blending across the seam would mix the old's clamped edge
        // pixel with the new's. The seam is at `t`, and it is a step.
        float along = dot(uv - 0.5, u_direction) + 0.5;
        mask = 1.0 - smoothstep(t - 0.001, t + 0.001, along);
    }
}

void main() {
    vec2 canvas = u_screen * u_scale;
    vec2 origin = (canvas - u_screen) * 0.5 + u_offset;
    vec2 c = (uv * u_screen + origin) / canvas;

    vec2 new_c = c;
    vec3 previous_color = vec3(0.0);
    float mask = 1.0;
    if (u_blend == 1) {
        vec2 old_uv;
        vec2 new_uv;
        transition(mask, old_uv, new_uv);
        // A slide (and a portal's push) moves the old frame *off* the screen.
        // There is no old content out there, and the sampler would hand back the
        // stretched edge pixel — so where the lookup leaves the frame, the new
        // side simply shows.
        if (any(lessThan(old_uv, vec2(0.0))) || any(greaterThan(old_uv, vec2(1.0)))) {
            mask = 1.0;
        }
        previous_color = texture(u_previous, old_uv).rgb;
        // The lookups are in screen space; the canvas is `scale` times bigger, so
        // a screen displacement is that many canvas pixels. Sampling outside the
        // frame clamps to the edge, which is harmless: the mask hides exactly
        // those pixels.
        new_c = c + (new_uv - uv) * u_scale;
    }

    vec3 base;
    if (u_has_wallpaper == 1) {
        // The texture is already canvas-sized and cover-cropped, so this is a
        // straight 1:1 lookup.
        base = texture(u_wallpaper, new_c).rgb;
    } else if (u_pattern == 1) {
        vec2 cell = floor(new_c * vec2(96.0, 54.0));
        float h = hash(cell);
        base = vec3(h, fract(h * 7.13), fract(h * 13.7));
    } else {
        int band = int(clamp(new_c.x, 0.0, 0.999) * 4.0);
        base = band == 0 ? vec3(1.0, 0.0, 0.0)
             : band == 1 ? vec3(0.0, 1.0, 0.0)
             : band == 2 ? vec3(0.0, 0.0, 1.0)
                         : vec3(1.0, 1.0, 0.0);
        base *= 0.35 + 0.65 * new_c.y;
        // The canvas edge, visible at the extremes of the travel.
        float edge = min(min(new_c.x, 1.0 - new_c.x), min(new_c.y, 1.0 - new_c.y));
        if (edge < 0.002) {
            base = vec3(1.0);
        }
    }

    if (u_blend == 1) {
        base = mix(previous_color, base, mask);
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
/// A copy of a frame, taken on the GPU. Used to cross-fade away from whatever
/// was on screen when the wallpaper changed: the *rendered* result is
/// snapshotted, so a still and a video are equally blendable.
pub struct Snapshot {
    texture: u32,
    size: (u32, u32),
}

impl Snapshot {
    /// An empty snapshot. Its contents are undefined until `capture` fills them,
    /// which is why there is no way to draw from one that has not been captured.
    pub fn new(width: u32, height: u32) -> Self {
        Self::allocate(width, height, std::ptr::null())
    }

    /// A snapshot that starts black, for a transition with nothing to fade from:
    /// the one played at startup. Costs a zeroed buffer once, which is the price
    /// of the shader never sampling undefined memory.
    pub fn new_black(width: u32, height: u32) -> Self {
        let zeros = vec![0u8; width as usize * height as usize * 4];
        Self::allocate(width, height, zeros.as_ptr().cast())
    }

    fn allocate(width: u32, height: u32, pixels: *const std::ffi::c_void) -> Self {
        let texture = gen_texture();
        bind_texture(GL_TEXTURE_2D, texture);
        set_2d_parameters();
        unsafe {
            glTexImage2D(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                pixels,
            );
        }
        bind_texture(GL_TEXTURE_2D, 0);
        Self {
            texture,
            size: (width, height),
        }
    }

    pub fn texture(&self) -> u32 {
        self.texture
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Copy the currently bound framebuffer into the snapshot. The caller binds
    /// it: `mpv_render_context_render` and `Frame::begin` both manage bindings,
    /// so guessing here would copy the wrong thing.
    pub fn capture(&self) {
        bind_texture(GL_TEXTURE_2D, self.texture);
        unsafe {
            glCopyTexSubImage2D(
                GL_TEXTURE_2D,
                0,
                0,
                0,
                0,
                0,
                self.size.0 as i32,
                self.size.1 as i32,
            );
        }
        bind_texture(GL_TEXTURE_2D, 0);
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        delete_texture(self.texture);
    }
}

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
