//! The compositor: draws one screen (world frame + cells + text) into any
//! texture: a window's surface, or an offscreen one read back for a PNG.

use super::text::{ATLAS, Quad, Text, shape};

/// A soft shape for the GPU, in window pixels.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DecoGpu {
    rect: [f32; 4],
    fill: [f32; 4],
    params: [f32; 4],
    border: [f32; 4],
}
use crate::render::Frame;
use crate::term::Screen;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen: [f32; 2],
    cell: [f32; 2],
    grid: [u32; 2],
    view_rows: u32,
    has_world: u32,
    /// 1: the world comes blended over time (no edge smoothing needed).
    taa: u32,
    _pad: [u32; 3],
}

pub struct Compositor {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
    grid_pipe: wgpu::RenderPipeline,
    text_pipe: wgpu::RenderPipeline,
    deco_pipe: wgpu::RenderPipeline,
    decos: wgpu::Buffer,
    deco_list: Vec<DecoGpu>,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    world: wgpu::Texture,
    /// Which frame the world texture holds.
    world_id: u64,
    /// Frames blended over time, when the frames come with depth.
    taa: Option<super::taa::Taa>,
    taa_on: bool,
    cell_bg: wgpu::Texture,
    cell_fg: wgpu::Texture,
    atlas: wgpu::Texture,
    quads: wgpu::Buffer,
    bind: Option<wgpu::BindGroup>,
    pub text: Text,
    quad_list: Vec<Quad>,
    cell_bytes: (Vec<u8>, Vec<u8>),
}

fn tex(device: &wgpu::Device, label: &str, w: u32, h: u32, format: wgpu::TextureFormat) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn write(queue: &wgpu::Queue, t: &wgpu::Texture, x: u32, y: u32, w: u32, h: u32, bpp: u32, data: &[u8]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: t, mip_level: 0, origin: wgpu::Origin3d { x, y, z: 0 }, aspect: wgpu::TextureAspect::All },
        data,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * bpp), rows_per_image: Some(h) },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
}

impl Compositor {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Compositor {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("window"), source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/window.wgsl").into()) });
        let frag = wgpu::ShaderStages::FRAGMENT;
        let t2d = |binding: u32, filterable: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: frag,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("window"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                t2d(1, true),
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: frag, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                t2d(3, false),
                t2d(4, false),
                t2d(5, false),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("window"), bind_group_layouts: &[Some(&bgl)], immediate_size: 0 });
        let target = [Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        let grid_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grid"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_grid"), compilation_options: Default::default(), targets: &target }),
            multiview_mask: None,
            cache: None,
        });
        let blended = [Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })];
        let attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];
        let text_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_text"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Quad>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &attrs })],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_text"), compilation_options: Default::default(), targets: &blended }),
            multiview_mask: None,
            cache: None,
        });
        let deco_attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
        let deco_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("deco"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_deco"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<DecoGpu>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &deco_attrs })],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_deco"), compilation_options: Default::default(), targets: &blended }),
            multiview_mask: None,
            cache: None,
        });
        let decos = device.create_buffer(&wgpu::BufferDescriptor { label: Some("decos"), size: 64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("world"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor { label: Some("window"), size: std::mem::size_of::<Uniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let world = tex(&device, "world", 1, 1, wgpu::TextureFormat::Rgba8Unorm);
        let cell_bg = tex(&device, "cell bg", 1, 1, wgpu::TextureFormat::Rgba8Unorm);
        let cell_fg = tex(&device, "cell fg", 1, 1, wgpu::TextureFormat::Rgba8Unorm);
        let atlas = tex(&device, "atlas", ATLAS, ATLAS, wgpu::TextureFormat::R8Unorm);
        let quads = device.create_buffer(&wgpu::BufferDescriptor { label: Some("glyphs"), size: 64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let taa = Some(super::taa::Taa::new(&device));
        Compositor {
            device,
            queue,
            format,
            grid_pipe,
            text_pipe,
            deco_pipe,
            decos,
            deco_list: Vec::new(),
            bgl,
            sampler,
            uniforms,
            world,
            world_id: u64::MAX,
            taa,
            taa_on: false,
            cell_bg,
            cell_fg,
            atlas,
            quads,
            bind: None,
            text: Text::new(),
            quad_list: Vec::new(),
            cell_bytes: (Vec::new(), Vec::new()),
        }
    }

    /// Draw `screen` (rows above `view_rows` over `frame`) into `target`, `size` pixels.
    pub fn draw(&mut self, target: &wgpu::TextureView, size: (u32, u32), screen: &Screen, view_rows: u16, frame: Option<&Frame>, cell: (f32, f32)) {
        let (gw, gh) = (screen.w as u32, screen.h as u32);
        // World pixels, when a new frame came.
        if let Some(f) = frame.filter(|f| f.id != self.world_id && f.width > 0) {
            let s = self.world.size();
            if s.width != f.width || s.height != f.height {
                self.world = tex(&self.device, "world", f.width, f.height, wgpu::TextureFormat::Rgba8Unorm);
                self.bind = None;
            }
            write(&self.queue, &self.world, 0, 0, f.width, f.height, 4, bytemuck::cast_slice(&f.pixels));
            self.world_id = f.id;
            // Blend it into the history, at up to twice its size (no more than the view has).
            let can = f.depth.len() == f.pixels.len() && f.view[4][2] > 0.5;
            if !can && let Some(t) = self.taa.as_mut() {
                t.forget();
            }
            match self.taa.as_mut().filter(|_| can) {
                Some(taa) => {
                    let (vw, vh) = ((screen.w as f32 * cell.0).round() as u32, (view_rows as f32 * cell.1).round() as u32);
                    let out = ((f.width * 2).min(vw).max(1), (f.height * 2).min(vh).max(1));
                    taa.resolve(&self.device, &self.queue, &self.sampler, f, &self.world, out);
                    self.taa_on = true;
                }
                None => self.taa_on = false,
            }
            self.bind = None;
        }
        // Cells.
        let s = self.cell_bg.size();
        if s.width != gw || s.height != gh {
            self.cell_bg = tex(&self.device, "cell bg", gw, gh, wgpu::TextureFormat::Rgba8Unorm);
            self.cell_fg = tex(&self.device, "cell fg", gw, gh, wgpu::TextureFormat::Rgba8Unorm);
            self.bind = None;
        }
        let (bg, fg) = &mut self.cell_bytes;
        bg.clear();
        fg.clear();
        for (c, see) in screen.cells.iter().zip(&screen.see) {
            bg.extend_from_slice(&[c.bg[0], c.bg[1], c.bg[2], *see as u8]);
            fg.extend_from_slice(&[c.fg[0], c.fg[1], c.fg[2], shape(c.ch)]);
        }
        write(&self.queue, &self.cell_bg, 0, 0, gw, gh, 4, bg);
        write(&self.queue, &self.cell_fg, 0, 0, gw, gh, 4, fg);
        // Text.
        for _ in 0..2 {
            self.text.reset = false;
            self.text.layout(screen, cell.0, cell.1, &mut self.quad_list);
            if !self.text.reset {
                break;
            }
        }
        for (x, y, w, h, px) in self.text.uploads.drain(..) {
            write(&self.queue, &self.atlas, x, y, w, h, 1, &px);
        }
        let need = (self.quad_list.len().max(1) * std::mem::size_of::<Quad>()) as u64;
        if self.quads.size() < need {
            self.quads = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("glyphs"), size: need.next_power_of_two(), usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        if !self.quad_list.is_empty() {
            self.queue.write_buffer(&self.quads, 0, bytemuck::cast_slice(&self.quad_list));
        }
        // Soft shapes: each shadow first, then its shape.
        self.deco_list.clear();
        let (cw, ch) = cell;
        let col = |c: [u8; 3], a: f32| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, a];
        let mut split = 0;
        for (i, d) in screen.decor.iter().chain(&screen.over_decor).enumerate() {
            if i == screen.decor.len() {
                split = self.deco_list.len();
            }
            let rect = [d.x * cw, d.y * ch, d.w * cw, d.h * ch];
            let r = d.radius * ch;
            if d.shadow {
                let blur = ch * 0.45;
                self.deco_list.push(DecoGpu { rect: [rect[0], rect[1] + ch * 0.25, rect[2], rect[3]], fill: [0.0, 0.0, 0.0, 0.45], params: [r, blur, 0.0, 0.0], border: [0.0; 4] });
            }
            let bw = if d.border.is_some() { (ch * 0.07).max(1.0) } else { 0.0 };
            self.deco_list.push(DecoGpu { rect, fill: col(d.fill, d.alpha), params: [r, 0.0, bw, 0.0], border: col(d.border.unwrap_or(d.fill), 1.0) });
        }
        if screen.over_decor.is_empty() {
            split = self.deco_list.len();
        }
        let need = (self.deco_list.len().max(1) * std::mem::size_of::<DecoGpu>()) as u64;
        if self.decos.size() < need {
            self.decos = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("decos"), size: need.next_power_of_two(), usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        if !self.deco_list.is_empty() {
            self.queue.write_buffer(&self.decos, 0, bytemuck::cast_slice(&self.deco_list));
        }
        let u = Uniforms {
            screen: [size.0 as f32, size.1 as f32],
            cell: [cell.0, cell.1],
            grid: [gw, gh],
            view_rows: view_rows as u32,
            has_world: (self.world_id != u64::MAX) as u32,
            taa: self.taa_on as u32,
            _pad: [0; 3],
        };
        self.queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        if self.bind.is_none() {
            let view = |t: &wgpu::Texture| t.create_view(&Default::default());
            let shown = match (&self.taa, self.taa_on) {
                (Some(t), true) => t.output(),
                _ => &self.world,
            };
            let (w, b, f, a) = (view(shown), view(&self.cell_bg), view(&self.cell_fg), view(&self.atlas));
            self.bind = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("window"),
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.uniforms.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&w) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&b) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&f) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&a) },
                ],
            }));
        }
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("window"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, self.bind.as_ref(), &[]);
            pass.set_pipeline(&self.grid_pipe);
            pass.draw(0..3, 0..1);
            // Shapes, then words; then what lies over them, the same way.
            let (ds, ts) = (split as u32, self.text.split as u32);
            for (decos, quads) in [(0..ds, 0..ts), (ds..self.deco_list.len() as u32, ts..self.quad_list.len() as u32)] {
                if !decos.is_empty() {
                    pass.set_pipeline(&self.deco_pipe);
                    pass.set_vertex_buffer(0, self.decos.slice(..));
                    pass.draw(0..6, decos);
                }
                if !quads.is_empty() {
                    pass.set_pipeline(&self.text_pipe);
                    pass.set_vertex_buffer(0, self.quads.slice(..));
                    pass.draw(0..6, quads);
                }
            }
        }
        self.queue.submit([enc.finish()]);
    }

    /// An offscreen target to draw into (for `--shot`), read back with `read_back`.
    pub fn offscreen(&self, size: (u32, u32)) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shot"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// The pixels of an offscreen target (RGB).
    pub fn read_back(&mut self, target: &wgpu::Texture) -> Vec<u8> {
        let (w, h) = (target.width(), target.height());
        let row = (w * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("shot"), size: (row * h) as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let data = buf.slice(..).get_mapped_range().expect("mapped shot");
        let bgra = matches!(self.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let i = (y * row + x * 4) as usize;
                let p = &data[i..i + 4];
                if bgra {
                    rgb.extend_from_slice(&[p[2], p[1], p[0]]);
                } else {
                    rgb.extend_from_slice(&p[..3]);
                }
            }
        }
        rgb
    }
}
