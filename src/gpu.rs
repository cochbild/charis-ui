//! GPU renderer (wgpu).
//!
//! The whole [`Scene`] becomes a single instanced draw call. Rounded rects,
//! borders and focus rings are shaded with signed distance functions, box
//! shadows are computed analytically (no blur passes), and glyphs / vector
//! paths are rasterized once into atlases. Glyph coverage gets the same
//! DirectWrite-style contrast and gamma correction as the CPU renderer.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use tiny_skia::{FillRule, LineCap, LineJoin, Paint, Pixmap, Stroke, Transform};

use crate::color::{Color, Fill};
use crate::geometry::Rect;
use crate::scene::{Cmd, Scene};
use crate::style::Corners;
use crate::text::{GlyphInst, TextSystem};

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct Instance {
    rect: [f32; 4],
    shape: [f32; 4],
    radii: [f32; 4],
    border: [f32; 4],
    color: [f32; 4],
    color2: [f32; 4],
    extra: [f32; 4],
    uv: [f32; 4],
    clip: [f32; 4],
    clip_radii: [f32; 4],
    params: [f32; 4],
    extra_radii: [f32; 4],
}

const KIND_FILL: f32 = 0.0;
const KIND_BORDER: f32 = 1.0;
const KIND_SHADOW: f32 = 2.0;
const KIND_MASK: f32 = 3.0;
const KIND_COLOR: f32 = 4.0;
const KIND_IMAGE: f32 = 5.0;
const FLAG_GRADIENT: f32 = 1.0;
const FLAG_TEXT: f32 = 2.0;

fn as_bytes<T: Copy>(v: &[T]) -> &[u8] {
    // SAFETY: `T` is a plain-old-data `repr(C)` struct of f32s (or f32s),
    // with no padding or pointers; viewing it as bytes is sound.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

/// A shelf-packing texture atlas.
struct Atlas {
    texture: wgpu::Texture,
    size: u32,
    bpp: u32,
    x: u32,
    y: u32,
    shelf_h: u32,
}

impl Atlas {
    fn new(device: &wgpu::Device, size: u32, format: wgpu::TextureFormat, bpp: u32, label: &str) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        Self { texture, size, bpp, x: 0, y: 0, shelf_h: 0 }
    }

    fn clear(&mut self) {
        self.x = 0;
        self.y = 0;
        self.shelf_h = 0;
    }

    /// Reserve a `w`×`h` region (1px padding). Returns its origin.
    fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let (pw, ph) = (w + 1, h + 1);
        if pw > self.size || ph > self.size {
            return None;
        }
        if self.x + pw > self.size {
            self.x = 0;
            self.y += self.shelf_h;
            self.shelf_h = 0;
        }
        if self.y + ph > self.size {
            return None;
        }
        let pos = (self.x, self.y);
        self.x += pw;
        self.shelf_h = self.shelf_h.max(ph);
        Some(pos)
    }

    fn upload(&self, queue: &wgpu::Queue, x: u32, y: u32, w: u32, h: u32, data: &[u8]) {
        if w == 0 || h == 0 {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * self.bpp), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    }
}

#[derive(Clone, Copy)]
struct Slot {
    /// Atlas position.
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    /// Offset of the bitmap's top-left from the glyph/path origin.
    left: i32,
    top: i32,
    color: bool,
}

/// Renders scenes with wgpu.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniforms: wgpu::Buffer,
    instances: Option<wgpu::Buffer>,
    mask: Atlas,
    color: Atlas,
    glyphs: HashMap<cosmic_text::CacheKey, Option<Slot>>,
    paths: HashMap<u64, (Option<Slot>, u64)>,
    /// Image rasters in the color atlas.
    images: HashMap<crate::image::RasterKey, Slot>,
    frame: u64,
    format: wgpu::TextureFormat,
    healthy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Name of the GPU adapter in use.
    pub adapter_name: String,
}

impl GpuRenderer {
    /// Create a renderer on any available adapter, without a window
    /// (for tests and screenshots). Returns `None` when no GPU is available.
    pub fn headless() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        Self::with_adapter(&adapter, wgpu::TextureFormat::Rgba8Unorm)
    }

    pub(crate) fn with_adapter(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> Option<Self> {
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rust-ui"),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .ok()?;
        let adapter_name = adapter.get_info().name;
        Some(Self::new(device, queue, format, adapter_name))
    }

    fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat, adapter_name: String) -> Self {
        // wgpu panics on uncaught errors by default; record them instead so the
        // app can fall back to the CPU renderer.
        let healthy = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let h = healthy.clone();
        device.on_uncaptured_error(std::sync::Arc::new(move |e: wgpu::Error| {
            eprintln!("rust-ui: GPU error, falling back to CPU rendering: {e}");
            h.store(false, std::sync::atomic::Ordering::Relaxed);
        }));
        let h = healthy.clone();
        device.set_device_lost_callback(move |reason, msg| {
            eprintln!("rust-ui: GPU device lost ({reason:?}): {msg}");
            h.store(false, std::sync::atomic::Ordering::Relaxed);
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rust-ui shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rust-ui bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rust-ui layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let attrs: Vec<wgpu::VertexAttribute> = (0..12)
            .map(|i| wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: (i * 16) as u64,
                shader_location: i as u32,
            })
            .collect();
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rust-ui pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attrs,
                })],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rust-ui globals"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let max = device.limits().max_texture_dimension_2d.min(4096);
        let mask = Atlas::new(&device, max.min(2048), wgpu::TextureFormat::R8Unorm, 1, "rust-ui mask atlas");
        // Color emoji and images.
        let color = Atlas::new(&device, max.min(2048), wgpu::TextureFormat::Rgba8Unorm, 4, "rust-ui color atlas");
        // Images drawn larger than their raster in the atlas (very big ones) stretch smoothly.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rust-ui image sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mask_view = mask.texture.create_view(&Default::default());
        let color_view = color.texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rust-ui bind group"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&mask_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&color_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        Self {
            device,
            queue,
            pipeline,
            bind_group,
            uniforms,
            instances: None,
            mask,
            color,
            glyphs: HashMap::new(),
            paths: HashMap::new(),
            images: HashMap::new(),
            frame: 0,
            format,
            healthy,
            adapter_name,
        }
    }

    pub(crate) fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Render a scene into a texture view and submit.
    pub fn render_to_view(&mut self, scene: &Scene, text: &mut TextSystem, view: &wgpu::TextureView) {
        self.frame += 1;
        let mut inst = self.build(scene, text);
        if inst.is_none() {
            // Atlas overflow: start fresh and rebuild once.
            self.reset_atlases();
            inst = self.build(scene, text);
        }
        let inst = inst.unwrap_or_default();
        let g = [scene.width as f32, scene.height as f32, 0.0, 0.0];
        self.queue.write_buffer(&self.uniforms, 0, as_bytes(&g));
        let bytes = (inst.len().max(1) * std::mem::size_of::<Instance>()) as u64;
        if self.instances.as_ref().is_none_or(|b| b.size() < bytes) {
            self.instances = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rust-ui instances"),
                size: bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let Some(buf) = self.instances.as_ref() else { return };
        if !inst.is_empty() {
            self.queue.write_buffer(buf, 0, as_bytes(&inst));
        }
        let bg = scene.background;
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rust-ui") });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rust-ui pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Premultiplied, so a transparent background (a
                        // window with a system backdrop) composes correctly.
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: (bg.r * bg.a) as f64,
                            g: (bg.g * bg.a) as f64,
                            b: (bg.b * bg.a) as f64,
                            a: bg.a as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if !inst.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..4, 0..inst.len() as u32);
            }
        }
        self.queue.submit([enc.finish()]);
        // Evict path masks unused for a while.
        if self.frame.is_multiple_of(240) {
            let f = self.frame;
            self.paths.retain(|_, v| f - v.1 < 240);
        }
    }

    /// Render a scene offscreen and read it back (for tests and screenshots).
    /// Returns `None` if the GPU failed.
    pub fn render_to_pixmap(&mut self, scene: &Scene, text: &mut TextSystem) -> Option<Pixmap> {
        let (w, h) = (scene.width.max(1), scene.height.max(1));
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rust-ui offscreen"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        self.render_to_view(scene, text, &view);
        let row = (w * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rust-ui readback"),
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let data = buf.slice(..).get_mapped_range().ok()?;
        let mut pm = Pixmap::new(w, h)?;
        let bgra = matches!(self.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        for y in 0..h as usize {
            let src = &data[y * row as usize..y * row as usize + w as usize * 4];
            let dst = &mut pm.data_mut()[y * w as usize * 4..(y + 1) * w as usize * 4];
            dst.copy_from_slice(src);
            if bgra {
                for px in dst.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
            }
        }
        drop(data);
        buf.unmap();
        self.is_healthy().then_some(pm)
    }

    /// False after a device loss or an uncaught GPU error; callers should
    /// switch to the CPU renderer.
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn reset_atlases(&mut self) {
        self.mask.clear();
        self.color.clear();
        self.glyphs.clear();
        self.paths.clear();
        self.images.clear();
    }

    /// Convert the scene into instances, rasterizing new glyphs/paths into the
    /// atlases. Returns `None` if an atlas overflowed.
    fn build(&mut self, scene: &Scene, text: &mut TextSystem) -> Option<Vec<Instance>> {
        let s = scene.scale;
        let mut out: Vec<Instance> = Vec::with_capacity(scene.cmds.len() * 2);
        let full = [-1.0e6, -1.0e6, 2.0e6, 2.0e6];
        let mut clips: Vec<(Rect, Corners)> = Vec::new();
        let mut opacity: Vec<f32> = vec![1.0];
        let phys = |r: Rect| [r.x * s, r.y * s, r.w * s, r.h * s];
        let rad = |c: Corners| [c.tl * s, c.tr * s, c.br * s, c.bl * s];
        for cmd in &scene.cmds {
            let (clip, clip_radii) = match clips.last() {
                Some((r, c)) => (phys(*r), rad(*c)),
                None => (full, [0.0; 4]),
            };
            let op = opacity.last().copied().unwrap_or(1.0);
            let col = |c: Color| [c.r, c.g, c.b, c.a * op];
            let base = Instance { clip, clip_radii, ..Default::default() };
            match cmd {
                Cmd::Fill { rect, radius, fill } => {
                    let r = phys(*rect);
                    let mut i =
                        Instance { rect: r, shape: r, radii: rad(*radius), params: [KIND_FILL, 0.0, 0.0, 0.0], ..base };
                    match fill {
                        Fill::Solid(c) => i.color = col(*c),
                        Fill::LinearGradient { angle, stops } => {
                            let (first, last) = (stops.first().copied(), stops.last().copied());
                            let (Some(a), Some(b)) = (first, last) else { continue };
                            let ang = angle.to_radians();
                            let (dx, dy) = (ang.sin(), -ang.cos());
                            let half = (r[2] * dx.abs() + r[3] * dy.abs()) / 2.0;
                            let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                            i.color = col(a.1);
                            i.color2 = col(b.1);
                            i.extra = [cx - dx * half, cy - dy * half, cx + dx * half, cy + dy * half];
                            i.uv = [a.0, b.0, 0.0, 0.0];
                            i.params[1] = FLAG_GRADIENT;
                        }
                    }
                    out.push(i);
                }
                Cmd::Border { rect, radius, widths, color } => {
                    let r = phys(*rect);
                    out.push(Instance {
                        rect: r,
                        shape: r,
                        radii: rad(*radius),
                        border: [widths.top * s, widths.right * s, widths.bottom * s, widths.left * s],
                        color: col(*color),
                        params: [KIND_BORDER, 0.0, 0.0, 0.0],
                        ..base
                    });
                }
                Cmd::Stroke { rect, radius, width, color } => {
                    let r = phys(rect.outset(width / 2.0));
                    let w = width * s;
                    out.push(Instance {
                        rect: r,
                        shape: r,
                        radii: rad(radius.grow(width / 2.0)),
                        border: [w, w, w, w],
                        color: col(*color),
                        params: [KIND_BORDER, 0.0, 0.0, 0.0],
                        ..base
                    });
                }
                Cmd::Shadow { rect, radius, shadow } => {
                    let sh = shadow;
                    let shape = rect.translate(sh.x, sh.y).outset(sh.spread);
                    if shape.is_empty() {
                        continue;
                    }
                    let reach = sh.blur * 1.5 + 2.0;
                    out.push(Instance {
                        rect: phys(shape.outset(reach)),
                        shape: phys(shape),
                        radii: rad(radius.grow(sh.spread)),
                        color: col(sh.color),
                        extra: phys(*rect),
                        extra_radii: rad(*radius),
                        params: [KIND_SHADOW, 0.0, (sh.blur * s / 2.0).max(0.1), 0.0],
                        ..base
                    });
                }
                Cmd::Glyphs { glyphs, color } => {
                    for g in glyphs {
                        let slot = self.glyph_slot(g, text)?;
                        let Some(slot) = slot else { continue };
                        let x = (g.x + slot.left) as f32;
                        let y = (g.y - slot.top) as f32;
                        out.push(Instance {
                            rect: [x, y, slot.w as f32, slot.h as f32],
                            uv: [slot.x as f32, slot.y as f32, 0.0, 0.0],
                            color: if slot.color { [1.0, 1.0, 1.0, color.a * op] } else { col(*color) },
                            params: [if slot.color { KIND_COLOR } else { KIND_MASK }, FLAG_TEXT, 0.0, 0.0],
                            ..base
                        });
                    }
                }
                Cmd::Path { path, transform, color, stroke, .. } => {
                    let Some((slot, ox, oy)) = self.path_slot(path, *transform, *stroke)? else { continue };
                    out.push(Instance {
                        rect: [ox as f32 + slot.left as f32, oy as f32 + slot.top as f32, slot.w as f32, slot.h as f32],
                        uv: [slot.x as f32, slot.y as f32, 0.0, 0.0],
                        color: col(*color),
                        params: [KIND_MASK, 0.0, 0.0, 0.0],
                        ..base
                    });
                }
                Cmd::Image { source, crop, dest, radius, tint } => {
                    let r = phys(*dest);
                    let (w, h) = (r[2].round().max(1.0), r[3].round().max(1.0));
                    let Some(slot) = self.image_slot(source, *crop, w as u32, h as u32, *tint)? else { continue };
                    out.push(Instance {
                        rect: r,
                        shape: r,
                        radii: rad(*radius),
                        uv: [slot.x as f32, slot.y as f32, slot.w as f32, slot.h as f32],
                        color: [1.0, 1.0, 1.0, op],
                        params: [KIND_IMAGE, 0.0, 0.0, 0.0],
                        ..base
                    });
                }
                Cmd::PushClip { rect, radius } => clips.push((*rect, *radius)),
                Cmd::PopClip => {
                    clips.pop();
                }
                Cmd::SetClips(c) => clips = c.clone(),
                Cmd::PushLayer { opacity: o, .. } => {
                    let cur = opacity.last().copied().unwrap_or(1.0);
                    opacity.push(cur * o);
                }
                Cmd::PopLayer => {
                    if opacity.len() > 1 {
                        opacity.pop();
                    }
                }
            }
        }
        Some(out)
    }

    /// `Err`-like `None` = atlas full; `Some(None)` = glyph has no image.
    fn glyph_slot(&mut self, g: &GlyphInst, text: &mut TextSystem) -> Option<Option<Slot>> {
        if let Some(s) = self.glyphs.get(&g.key) {
            return Some(*s);
        }
        let Some(img) = text.glyph_image(g.key) else {
            self.glyphs.insert(g.key, None);
            return Some(None);
        };
        let (w, h) = (img.placement.width, img.placement.height);
        if w == 0 || h == 0 {
            self.glyphs.insert(g.key, None);
            return Some(None);
        }
        let slot = match img.content {
            cosmic_text::SwashContent::Color => {
                let (x, y) = self.color.alloc(w, h)?;
                // Premultiply for the shader.
                let mut data = img.data.clone();
                for px in data.chunks_exact_mut(4) {
                    let a = px[3] as u32;
                    for c in &mut px[..3] {
                        *c = ((*c as u32 * a + 127) / 255) as u8;
                    }
                }
                self.color.upload(&self.queue, x, y, w, h, &data);
                Slot { x, y, w, h, left: img.placement.left, top: img.placement.top, color: true }
            }
            cosmic_text::SwashContent::Mask => {
                let (x, y) = self.mask.alloc(w, h)?;
                self.mask.upload(&self.queue, x, y, w, h, &img.data);
                Slot { x, y, w, h, left: img.placement.left, top: img.placement.top, color: false }
            }
            cosmic_text::SwashContent::SubpixelMask => {
                let (x, y) = self.mask.alloc(w, h)?;
                let data: Vec<u8> = img.data.chunks_exact(4).map(|p| p[1]).collect();
                self.mask.upload(&self.queue, x, y, w, h, &data);
                Slot { x, y, w, h, left: img.placement.left, top: img.placement.top, color: false }
            }
        };
        self.glyphs.insert(g.key, Some(slot));
        Some(Some(slot))
    }

    /// Put an image raster in the color atlas (or reuse it). Images too big
    /// for the atlas are rasterized smaller and stretched by the sampler.
    /// `None` = atlas full; `Some(None)` = nothing to draw.
    fn image_slot(
        &mut self,
        source: &crate::image::ImageSource,
        crop: Rect,
        w: u32,
        h: u32,
        tint: Option<Color>,
    ) -> Option<Option<Slot>> {
        let cap = self.color.size / 2;
        let k = (cap as f32 / w.max(h) as f32).min(1.0);
        let (w, h) = (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1));
        let key = crate::image::key(source, crop, w, h, tint);
        if let Some(s) = self.images.get(&key) {
            return Some(Some(*s));
        }
        let Some(img) = crate::image::raster(source, crop, w, h, tint) else { return Some(None) };
        let (x, y) = self.color.alloc(w, h)?;
        self.color.upload(&self.queue, x, y, w, h, img.data());
        let slot = Slot { x, y, w, h, left: 0, top: 0, color: true };
        self.images.insert(key, slot);
        Some(Some(slot))
    }

    /// Rasterize (or reuse) a path mask. Returns the slot and the integer
    /// pixel origin the slot's offsets are relative to.
    fn path_slot(
        &mut self,
        path: &tiny_skia::Path,
        t: Transform,
        stroke: Option<f32>,
    ) -> Option<Option<(Slot, i32, i32)>> {
        // Split translation into integer placement + quantized subpixel offset.
        let (ix, iy) = (t.tx.floor(), t.ty.floor());
        let (fx, fy) = (((t.tx - ix) * 4.0).round() / 4.0, ((t.ty - iy) * 4.0).round() / 4.0);
        let local = Transform::from_row(t.sx, t.ky, t.kx, t.sy, fx, fy);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for p in path.points() {
            p.x.to_bits().hash(&mut h);
            p.y.to_bits().hash(&mut h);
        }
        for v in path.verbs() {
            (*v as u8).hash(&mut h);
        }
        for f in [t.sx, t.ky, t.kx, t.sy, fx, fy, stroke.unwrap_or(-1.0)] {
            f.to_bits().hash(&mut h);
        }
        let key = h.finish();
        let frame = self.frame;
        if let Some(e) = self.paths.get_mut(&key) {
            e.1 = frame;
            return Some(e.0.map(|s| (s, ix as i32, iy as i32)));
        }
        let tp = path.clone().transform(local)?;
        let b = tp.bounds();
        let pad = stroke.map(|w| w * t.sx.abs().max(t.sy.abs()) / 2.0 + 1.0).unwrap_or(1.0);
        let x0 = (b.left() - pad).floor();
        let y0 = (b.top() - pad).floor();
        let w = ((b.right() + pad).ceil() - x0).max(1.0) as u32;
        let hh = ((b.bottom() + pad).ceil() - y0).max(1.0) as u32;
        if w > 1024 || hh > 1024 {
            self.paths.insert(key, (None, frame));
            return Some(None);
        }
        let mut pm = Pixmap::new(w, hh)?;
        let mut paint = Paint { anti_alias: true, ..Default::default() };
        paint.set_color(tiny_skia::Color::WHITE);
        let tt = local.post_translate(-x0, -y0);
        match stroke {
            Some(sw) => {
                let st =
                    Stroke { width: sw, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
                pm.stroke_path(path, &paint, &st, tt, None);
            }
            None => pm.fill_path(path, &paint, FillRule::Winding, tt, None),
        }
        let alpha: Vec<u8> = pm.data().chunks_exact(4).map(|p| p[3]).collect();
        let (ax, ay) = self.mask.alloc(w, hh)?;
        self.mask.upload(&self.queue, ax, ay, w, hh, &alpha);
        let slot = Slot { x: ax, y: ay, w, h: hh, left: x0 as i32, top: y0 as i32, color: false };
        self.paths.insert(key, (Some(slot), frame));
        Some(Some((slot, ix as i32, iy as i32)))
    }
}

/// A GPU renderer bound to a window surface.
pub struct GpuSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pub renderer: GpuRenderer,
}

impl GpuSurface {
    /// Create a surface for a window. Returns `None` if no suitable GPU is available.
    ///
    /// `transparent`: the window shows a system backdrop through transparent
    /// pixels (Windows Mica / Acrylic). That takes a DirectComposition
    /// swapchain on DX12 with premultiplied alpha; `None` if unavailable.
    pub fn new<W>(window: std::sync::Arc<W>, width: u32, height: u32, transparent: bool) -> Option<Self>
    where
        W: wgpu::WindowHandle + wgpu::wgt::WgpuHasDisplayHandle + 'static,
    {
        let mut desc = wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone()));
        if transparent {
            if !cfg!(windows) {
                return None;
            }
            desc.backends = wgpu::Backends::DX12;
            desc.backend_options.dx12.presentation_system = wgpu::Dx12SwapchainKind::DxgiFromVisual;
        }
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(window).ok()?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .ok()?;
        let caps = surface.get_capabilities(&adapter);
        // Blend in sRGB-encoded space like browsers do: prefer a non-sRGB format.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| matches!(f, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm))
            .or_else(|| caps.formats.first().copied())?;
        let renderer = GpuRenderer::with_adapter(&adapter, format)?;
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            caps.present_modes[0]
        };
        let alpha_mode = if transparent {
            caps.alpha_modes.iter().copied().find(|m| *m == wgpu::CompositeAlphaMode::PreMultiplied)?
        } else {
            caps.alpha_modes[0]
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(renderer.device(), &config);
        Some(Self { surface, config, renderer })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if (width, height) != (self.config.width, self.config.height) && width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(self.renderer.device(), &self.config);
        }
    }

    /// Render and present a scene. Returns false if the frame was skipped.
    pub fn present(&mut self, scene: &Scene, text: &mut TextSystem) -> bool {
        self.resize(scene.width, scene.height);
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                self.surface.configure(self.renderer.device(), &self.config);
                return false;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        self.renderer.render_to_view(scene, text, &view);
        self.renderer.queue.present(frame);
        true
    }
}
