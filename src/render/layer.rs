//! Wayland plumbing: registry, outputs, layer-shell surface, dmabuf buffers.
//!
//! Deliberately explicit rather than generic: this is the layer the daemon will
//! grow into, and the interesting parts (which output, which namespace, which
//! dmabuf formats the compositor actually accepts) are exactly the things
//! `HANDOFF.md` §6 M0b has to prove.

use std::collections::HashMap;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};

use wayland_client::backend::ObjectId;
use wayland_client::globals::{registry_queue_init, GlobalList, GlobalListContents};
use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_output, wl_registry, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_buffer_params_v1, zwp_linux_dmabuf_feedback_v1, zwp_linux_dmabuf_v1,
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use super::dmabuf::Frame;
use super::egl::Egl;
use super::gbm::Device;

/// Per-output state, keyed by object id in [`State::output_info`].
#[derive(Debug, Default, Clone)]
pub struct OutputInfo {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// A `(format, modifier)` pair the compositor says it can import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatModifier {
    pub format: u32,
    pub modifier: u64,
}

#[derive(Debug, Default)]
pub struct State {
    pub outputs: Vec<wl_output::WlOutput>,
    /// Dispatch data is shared (`&U`), so per-output state lives here instead.
    pub output_info: HashMap<ObjectId, OutputInfo>,
    pub configure: Option<Configure>,
    pub closed: bool,
    pub preferred_buffer_scale: i32,
    /// `wl_buffer::release` events seen — the compositor handing buffers back.
    pub releases: u32,
    /// Ids of the buffers the compositor has released, for the pool to recycle.
    pub released: Vec<ObjectId>,
    /// `wl_callback::done` events per surface: the compositor asking that
    /// surface for its next frame. Keyed by the surface's object id, because a
    /// daemon drawing on several outputs has to redraw only the one that asked.
    pub frames_done: HashMap<ObjectId, u64>,
    /// Device the compositor prefers for buffers (dev_t as an integer).
    pub main_device: Option<u64>,
    pub formats: Vec<FormatModifier>,
    /// What the *default* feedback object advertised (the primary GPU).
    pub default_main_device: Option<u64>,
    pub default_formats: Vec<FormatModifier>,
    table: Vec<FormatModifier>,
    tranche: Vec<FormatModifier>,
    tranche_target: Option<u64>,
    pub feedback_done: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Configure {
    pub serial: u32,
    pub width: u32,
    pub height: u32,
}

impl State {
    pub fn output_info(&self, output: &wl_output::WlOutput) -> Option<&OutputInfo> {
        self.output_info.get(&output.id())
    }

    /// Output names, in binding order.
    pub fn output_names(&self) -> Vec<String> {
        self.outputs
            .iter()
            .map(|o| {
                self.output_info(o)
                    .and_then(|d| d.name.clone())
                    .unwrap_or_else(|| "<unnamed>".to_owned())
            })
            .collect()
    }

    /// Outputs that advertised a name, i.e. bindable at v4.
    pub fn output_named(&self, name: &str) -> Option<wl_output::WlOutput> {
        self.outputs
            .iter()
            .find(|o| {
                self.output_info(o)
                    .and_then(|d| d.name.as_deref())
                    .is_some_and(|n| n == name)
            })
            .cloned()
    }

    /// Modifiers the compositor accepts for `format`, LINEAR first.
    pub fn modifiers_for(&self, format: u32) -> Vec<u64> {
        let mut out: Vec<u64> = self
            .formats
            .iter()
            .filter(|f| f.format == format)
            .map(|f| f.modifier)
            .collect();
        out.sort_by_key(|m| if *m == 0 { 0 } else { 1 });
        out.dedup();
        out
    }
}

/// A connected Wayland client with the globals a wallpaper layer needs.
pub struct Client {
    pub conn: Connection,
    pub queue: EventQueue<State>,
    pub state: State,
    globals: GlobalList,
    pub compositor: wl_compositor::WlCompositor,
    pub layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    pub dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
    surface_feedback: Option<zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1>,
    pub dmabuf_version: u32,
    pub compositor_version: u32,
    pub layer_shell_version: u32,
    pub output_version: u32,
}

impl Client {
    pub fn connect() -> Result<Self, String> {
        let conn = Connection::connect_to_env().map_err(|e| format!("connect_to_env: {e}"))?;
        let (globals, queue) =
            registry_queue_init::<State>(&conn).map_err(|e| format!("registry init: {e}"))?;
        let qh = queue.handle();

        let compositor: wl_compositor::WlCompositor = globals
            .bind(&qh, 1..=4, ())
            .map_err(|e| format!("wl_compositor: {e}"))?;
        let layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1 = globals
            .bind(&qh, 1..=5, ())
            .map_err(|e| format!("zwlr_layer_shell_v1: {e}"))?;
        let dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1 = globals
            .bind(&qh, 1..=4, ())
            .map_err(|e| format!("zwp_linux_dmabuf_v1: {e}"))?;

        let mut client = Self {
            conn,
            queue,
            state: State::default(),
            globals,
            compositor_version: compositor.version(),
            layer_shell_version: layer_shell.version(),
            dmabuf_version: dmabuf.version(),
            output_version: 0,
            compositor,
            layer_shell,
            dmabuf,
            surface_feedback: None,
        };

        // wl_output is a multi-instance global: bind each one by hand.
        let registry = client.globals.registry().clone();
        let global_list = client.globals.contents().clone_list();
        for global in global_list {
            if global.interface != "wl_output" {
                continue;
            }
            let version = global.version.min(4);
            let qh = client.queue.handle();
            let output: wl_output::WlOutput = registry.bind(global.name, version, &qh, ());
            client.output_version = client.output_version.max(version);
            client.state.outputs.push(output);
        }

        client.roundtrip()?;
        // Ask for the default dmabuf feedback: it describes the compositor's
        // *primary* GPU. Per-surface feedback (below) is what a real client
        // must use, since a multi-GPU compositor renders each output on that
        // output's GPU.
        let _default_feedback = client
            .dmabuf
            .get_default_feedback(&client.queue.handle(), ());
        client.roundtrip()?;
        client.state.default_main_device = client.state.main_device.take();
        client.state.default_formats = std::mem::take(&mut client.state.formats);
        client.state.feedback_done = false;
        Ok(client)
    }

    /// Ask the compositor which device and formats it wants for `surface`.
    ///
    /// This is the authoritative answer for "which GPU should render this
    /// output", and it can differ per output.
    pub fn use_surface_feedback(&mut self, surface: &wl_surface::WlSurface) -> Result<(), String> {
        self.state.main_device = None;
        self.state.formats.clear();
        self.state.feedback_done = false;
        let feedback = self
            .dmabuf
            .get_surface_feedback(surface, &self.queue.handle(), ());
        self.surface_feedback = Some(feedback);
        self.roundtrip()
    }

    pub fn roundtrip(&mut self) -> Result<(), String> {
        self.queue
            .roundtrip(&mut self.state)
            .map(|_| ())
            .map_err(|e| format!("roundtrip: {e}"))
    }

    pub fn flush(&self) -> Result<(), String> {
        self.conn.flush().map_err(|e| format!("flush: {e}"))
    }

    /// Poll the Wayland fd for at most `timeout`, then dispatch whatever
    /// arrived. Returns the number of events dispatched.
    ///
    /// Blocking on `poll` rather than spinning is what makes idle cost nothing.
    pub fn wait_events(&mut self, timeout: std::time::Duration) -> Result<usize, String> {
        let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        if let Some(guard) = self.conn.prepare_read() {
            let fd = guard.connection_fd();
            let mut pollfd = libc::pollfd {
                fd: fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
            if ready > 0 {
                guard.read().map_err(|e| format!("reading events: {e}"))?;
            }
            // Dropping the guard without reading cancels the read.
        }
        self.queue
            .dispatch_pending(&mut self.state)
            .map_err(|e| format!("dispatching events: {e}"))
    }

    /// Block until the compositor has released `want` buffers in total.
    pub fn wait_for_releases(
        &mut self,
        want: u32,
        timeout: std::time::Duration,
    ) -> Result<(), String> {
        let start = std::time::Instant::now();
        while self.state.releases < want {
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Err(format!(
                    "timed out waiting for buffer release ({}/{want})",
                    self.state.releases
                ));
            }
            self.wait_events(remaining)?;
        }
        Ok(())
    }

    /// The connection fd, for an event loop that wants to poll it.
    pub fn fd(&self) -> std::os::fd::RawFd {
        self.conn
            .prepare_read()
            .map(|guard| std::os::fd::AsRawFd::as_raw_fd(&guard.connection_fd()))
            .unwrap_or(-1)
    }

    pub fn handle(&self) -> QueueHandle<State> {
        self.queue.handle()
    }

    /// Create a background layer surface on `output` and wait for its configure.
    pub fn create_layer_surface(
        &mut self,
        output: &wl_output::WlOutput,
        namespace: &str,
        layer: zwlr_layer_shell_v1::Layer,
    ) -> Result<LayerSurface, String> {
        self.state.configure = None;
        self.state.closed = false;
        let qh = self.queue.handle();
        let surface = self.compositor.create_surface(&qh, ());
        let layer_surface = self.layer_shell.get_layer_surface(
            &surface,
            Some(output),
            layer,
            namespace.to_owned(),
            &qh,
            (),
        );
        // Anchored to every edge with a zero size: the compositor picks the
        // size, and the configure event tells us what it picked.
        layer_surface.set_size(0, 0);
        layer_surface.set_anchor(zwlr_layer_surface_v1::Anchor::all());
        // -1: a wallpaper stretches under/over panels (layer-shell spec).
        layer_surface.set_exclusive_zone(-1);
        layer_surface
            .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
        surface.commit();

        while self.state.configure.is_none() {
            if self.state.closed {
                return Err("layer surface closed before configure".to_owned());
            }
            self.queue
                .blocking_dispatch(&mut self.state)
                .map_err(|e| format!("waiting for configure: {e}"))?;
        }
        let configure = self.state.configure.expect("checked above");
        layer_surface.ack_configure(configure.serial);

        let output_name = self
            .state
            .output_info(output)
            .and_then(|d| d.name.clone())
            .unwrap_or_else(|| "<unnamed>".to_owned());
        Ok(LayerSurface {
            surface,
            layer_surface,
            output: output.clone(),
            output_name,
            width: configure.width,
            height: configure.height,
            serial: configure.serial,
        })
    }

    /// Attach `frame` to `surface` so the compositor imports the dmabuf.
    ///
    /// The returned `wl_buffer` must outlive the commit: the compositor holds a
    /// reference until it sends `wl_buffer::release`.
    pub fn attach(&mut self, surface: &LayerSurface, frame: &Frame) -> wl_buffer::WlBuffer {
        let buffer = create_wl_buffer(&self.dmabuf, &self.queue.handle(), frame);
        surface.surface.attach(Some(&buffer), 0, 0);
        surface
            .surface
            .damage_buffer(0, 0, frame.width as i32, frame.height as i32);
        surface.surface.commit();
        buffer
    }
}

/// A mapped layer surface.
pub struct LayerSurface {
    pub surface: wl_surface::WlSurface,
    pub layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    pub output: wl_output::WlOutput,
    pub output_name: String,
    pub width: u32,
    pub height: u32,
    pub serial: u32,
}

impl LayerSurface {
    /// Ask the compositor to tell us when it wants the next frame.
    ///
    /// **The caller must commit**: `wl_surface.frame` only takes effect on the
    /// next `wl_surface.commit`, so a request without one is silently dropped
    /// and the callback never arrives. Commit the frame request together with
    /// the buffer being submitted, or on its own when no draw is due yet.
    ///
    /// This is the only thing that drives redrawing: while no callback is
    /// outstanding the process sits in `poll()` and costs nothing.
    pub fn request_frame(&self, qh: &QueueHandle<State>) -> wl_callback::WlCallback {
        // The callback carries its own surface's id, so the handler can tell
        // which output asked without the caller having to track an index.
        self.surface.frame(qh, self.id())
    }

    /// This surface's object id — the key its frame callbacks arrive under.
    pub fn id(&self) -> ObjectId {
        self.surface.id()
    }

    pub fn destroy(&self) {
        self.layer_surface.destroy();
        self.surface.destroy();
    }
}

/// Import `frame`'s planes as a `wl_buffer`.
pub fn create_wl_buffer(
    dmabuf: &zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
    qh: &QueueHandle<State>,
    frame: &Frame,
) -> wl_buffer::WlBuffer {
    let params = dmabuf.create_params(qh, ());
    for (idx, plane) in frame.planes.iter().enumerate() {
        params.add(
            plane.fd.as_fd(),
            idx as u32,
            plane.offset,
            plane.stride,
            (frame.modifier >> 32) as u32,
            frame.modifier as u32,
        );
    }
    let buffer = params.create_immed(
        frame.width as i32,
        frame.height as i32,
        frame.format,
        zwp_linux_buffer_params_v1::Flags::empty(),
        qh,
        (),
    );
    params.destroy();
    buffer
}

// --- dispatch -------------------------------------------------------------

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // `registry_queue_init` maintains the list; dynamic changes are not
        // relevant to a probe that maps a surface once.
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &wl_compositor::WlCompositor,
        _event: wl_compositor::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &wl_surface::WlSurface,
        event: wl_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_surface::Event::PreferredBufferScale { factor } = event {
            state.preferred_buffer_scale = factor;
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            state.releases += 1;
            state.released.push(proxy.id());
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ObjectId> for State {
    fn event(
        state: &mut Self,
        _proxy: &wl_callback::WlCallback,
        _event: wl_callback::Event,
        data: &ObjectId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        *state.frames_done.entry(data.clone()).or_default() += 1;
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &wl_output::WlOutput,
        event: wl_output::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let info = state.output_info.entry(proxy.id()).or_default();
        match event {
            wl_output::Event::Name { name } => info.name = Some(name),
            wl_output::Event::Description { description } => info.description = Some(description),
            _ => {}
        }
    }
}

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        _event: zwlr_layer_shell_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                state.configure = Some(Configure {
                    serial,
                    width,
                    height,
                });
            }
            zwlr_layer_surface_v1::Event::Closed => state.closed = true,
            _ => {}
        }
    }
}

impl Dispatch<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
        event: zwp_linux_dmabuf_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Only sent when bound below v4.
        match event {
            zwp_linux_dmabuf_v1::Event::Format { format } => {
                state.formats.push(FormatModifier {
                    format,
                    modifier: u64::MAX,
                });
            }
            zwp_linux_dmabuf_v1::Event::Modifier {
                format,
                modifier_hi,
                modifier_lo,
            } => {
                let modifier = ((modifier_hi as u64) << 32) | modifier_lo as u64;
                if let Some(entry) = state
                    .formats
                    .iter_mut()
                    .find(|f| f.format == format && f.modifier == u64::MAX)
                {
                    entry.modifier = modifier;
                } else {
                    state.formats.push(FormatModifier { format, modifier });
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
        event: zwp_linux_buffer_params_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let zwp_linux_buffer_params_v1::Event::Failed = event {
            eprintln!("dmabuf import failed (compositor rejected the buffer)");
        }
    }
}

impl Dispatch<zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1,
        event: zwp_linux_dmabuf_feedback_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use zwp_linux_dmabuf_feedback_v1::Event;
        match event {
            Event::MainDevice { device } => state.main_device = parse_dev(&device),
            Event::FormatTable { fd, size } => {
                state.table = read_format_table(fd.as_fd(), size).unwrap_or_default();
            }
            Event::TrancheTargetDevice { device } => {
                state.tranche_target = parse_dev(&device).or(state.main_device);
            }
            Event::TrancheFormats { indices } => {
                state.tranche.clear();
                for chunk in indices.as_chunks::<2>().0 {
                    let index = u16::from_le_bytes([chunk[0], chunk[1]]) as usize;
                    if let Some(entry) = state.table.get(index) {
                        state.tranche.push(*entry);
                    }
                }
            }
            Event::TrancheDone => {
                if state.tranche_target.is_none() || state.tranche_target == state.main_device {
                    state.formats.extend_from_slice(&state.tranche);
                }
                state.tranche.clear();
                state.tranche_target = None;
            }
            Event::Done => state.feedback_done = true,
            _ => {}
        }
    }
}

fn parse_dev(bytes: &[u8]) -> Option<u64> {
    let head: [u8; 8] = bytes.get(..8)?.try_into().ok()?;
    Some(u64::from_le_bytes(head))
}

/// The format table is an array of `{u32 format, u32 padding, u64 modifier}`.
fn read_format_table(fd: BorrowedFd<'_>, size: u32) -> Option<Vec<FormatModifier>> {
    let len = size as usize;
    if len == 0 || !len.is_multiple_of(16) {
        return None;
    }
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ,
            libc::MAP_PRIVATE,
            fd.as_raw_fd(),
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) }.to_vec();
    unsafe { libc::munmap(ptr, len) };

    Some(
        bytes
            .as_chunks::<16>()
            .0
            .iter()
            .filter_map(|entry| {
                let format = u32::from_le_bytes(entry[..4].try_into().ok()?);
                let modifier = u64::from_le_bytes(entry[8..16].try_into().ok()?);
                Some(FormatModifier { format, modifier })
            })
            .collect(),
    )
}

/// A few interchangeable buffers, recycled when the compositor releases them.
///
/// A wallpaper only redraws while an animation runs, but that is still ~100
/// frames per column switch at 180 Hz: allocating a 14 MB GBM buffer per frame
/// would be absurd, and a single buffer would stall the pipeline. Two slots are
/// enough — while one is on screen the other is being drawn — and when both are
/// in flight the right thing to do is skip the frame rather than queue it up.
pub struct Pool<'a> {
    slots: Vec<Slot<'a>>,
    /// The modifiers the first buffer was created with — the compositor
    /// advertised them, or the driver chose.
    chosen: Vec<u64>,
    /// Attempts that failed, for the log: a modifier the compositor advertises
    /// but the driver cannot render into is worth reporting.
    failures: Vec<(Vec<u64>, String)>,
}

struct Slot<'a> {
    frame: Frame<'a>,
    /// The `wl_buffer` on screen for this frame, until it is released.
    in_flight: Option<ObjectId>,
}

impl<'a> Pool<'a> {
    /// Allocate `depth` buffers, searching `advertised` for a modifier this
    /// driver can actually render into. The successful attempt is kept as the
    /// first slot instead of being thrown away.
    pub fn new(
        egl: &'a Egl,
        device: &'a Device,
        width: u32,
        height: u32,
        format: u32,
        advertised: &[u64],
        depth: usize,
    ) -> Result<Self, String> {
        let renderable: Vec<u64> = egl
            .dmabuf_modifiers(format)
            .into_iter()
            .filter(|(_, external_only)| !external_only)
            .map(|(modifier, _)| modifier)
            .collect();

        let mut failures = Vec::new();
        let mut first = None;
        let mut chosen = Vec::new();
        for candidate in super::dmabuf::modifier_candidates(advertised, &renderable) {
            match Frame::new(egl, device, width, height, format, &candidate) {
                Ok(frame) => {
                    first = Some(frame);
                    chosen = candidate;
                    break;
                }
                Err(err) => failures.push((candidate, err)),
            }
        }
        let Some(first) = first else {
            let mut report = String::from("no renderable buffer:");
            for (modifiers, err) in &failures {
                report.push_str(&format!("\n  {}: {err}", describe_modifiers(modifiers)));
            }
            return Err(report);
        };

        let mut slots = vec![Slot {
            frame: first,
            in_flight: None,
        }];
        for index in 1..depth.max(1) {
            let frame = Frame::new(egl, device, width, height, format, &chosen)
                .map_err(|e| format!("buffer {index}: {e}"))?;
            slots.push(Slot {
                frame,
                in_flight: None,
            });
        }
        Ok(Self {
            slots,
            chosen,
            failures,
        })
    }

    /// The modifiers the pool ended up using.
    pub fn chosen(&self) -> &[u64] {
        &self.chosen
    }

    /// Attempts that failed, for the log.
    pub fn failures(&self) -> &[(Vec<u64>, String)] {
        &self.failures
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn in_flight(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.in_flight.is_some())
            .count()
    }

    /// Recycle everything in `released`, then hand out a free slot.
    ///
    /// `None` means every buffer is still on screen: drop this frame.
    pub fn acquire(&mut self, released: &[ObjectId]) -> Option<usize> {
        for slot in &mut self.slots {
            if slot
                .in_flight
                .as_ref()
                .is_some_and(|id| released.contains(id))
            {
                slot.in_flight = None;
            }
        }
        self.slots.iter().position(|slot| slot.in_flight.is_none())
    }

    pub fn frame(&self, index: usize) -> &Frame<'a> {
        &self.slots[index].frame
    }

    /// Record that slot `index` is now on screen.
    pub fn mark_submitted(&mut self, index: usize, buffer: &wl_buffer::WlBuffer) {
        self.slots[index].in_flight = Some(buffer.id());
    }
}

/// `LINEAR`, `nv:0x…`, or "driver-chosen" — for log lines.
pub fn describe_modifiers(modifiers: &[u64]) -> String {
    if modifiers.is_empty() {
        return "driver-chosen modifier".to_owned();
    }
    modifiers
        .iter()
        .map(|m| super::gbm::modifier_name(*m))
        .collect::<Vec<_>>()
        .join("+")
}
