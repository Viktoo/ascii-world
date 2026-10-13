//! Frames blended over time (`shaders/taa.wgsl`): each world frame is drawn
//! with its rays shifted by a fraction of a pixel, and mixed into a history
//! kept at up to twice its resolution. Standing still, detail builds up over
//! a few frames; moving, the history follows the camera and is held to what
//! the newest frame shows.

use crate::render::Frame;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    cur: [[f32; 4]; 5],
    prev: [[f32; 4]; 5],
    sizes: [f32; 4],
    flags: [u32; 4],
}

const HIST: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The jitter for frame `n`: a Halton (2, 3) point, in pixels, about 0.
pub fn jitter(n: u64) -> [f32; 2] {
    fn halton(mut i: u64, b: u64) -> f32 {
        let (mut f, mut r) = (1.0f32, 0.0f32);
        while i > 0 {
            f /= b as f32;
            r += f * (i % b) as f32;
            i /= b;
        }
        r
    }
    let i = n % 8 + 1;
    [halton(i, 2) - 0.5, halton(i, 3) - 0.5]
}

pub struct Taa {
    pipe: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    uniforms: wgpu::Buffer,
    depth: wgpu::Texture,
    /// Two histories: one read, the other written; `cur` is the latest.
    hist: [wgpu::Texture; 2],
    cur: usize,
    prev_view: Option<[[f32; 4]; 5]>,
    src: (u32, u32),
}

fn texture(device: &wgpu::Device, label: &str, w: u32, h: u32, format: wgpu::TextureFormat, target: bool) -> wgpu::Texture {
    let mut usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
    if target {
        usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

impl Taa {
    pub fn new(device: &wgpu::Device) -> Taa {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("taa"), source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/taa.wgsl").into()) });
        let frag = wgpu::ShaderStages::FRAGMENT;
        let t2d = |binding: u32, filterable: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: frag,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taa"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: frag, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                t2d(1, true),
                t2d(2, false),
                t2d(3, true),
                wgpu::BindGroupLayoutEntry { binding: 4, visibility: frag, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("taa"), bind_group_layouts: &[Some(&bgl)], immediate_size: 0 });
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taa"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_taa"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: HIST, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            multiview_mask: None,
            cache: None,
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor { label: Some("taa"), size: std::mem::size_of::<Uniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        Taa {
            pipe,
            bgl,
            uniforms,
            depth: texture(device, "depth", 1, 1, wgpu::TextureFormat::R32Float, false),
            hist: [texture(device, "history", 1, 1, HIST, true), texture(device, "history", 1, 1, HIST, true)],
            cur: 0,
            prev_view: None,
            src: (0, 0),
        }
    }

    /// Start over with the next frame (blending was off for a while).
    pub fn forget(&mut self) {
        self.prev_view = None;
    }

    /// The blended picture so far.
    pub fn output(&self) -> &wgpu::Texture {
        &self.hist[self.cur]
    }

    /// Mix `frame` (its colours already in `color`) into the history, kept at
    /// `out` pixels.
    pub fn resolve(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, sampler: &wgpu::Sampler, frame: &Frame, color: &wgpu::Texture, out: (u32, u32)) {
        let (w, h) = (frame.width, frame.height);
        let mut reset = self.prev_view.is_none();
        if self.src != (w, h) {
            self.depth = texture(device, "depth", w, h, wgpu::TextureFormat::R32Float, false);
            self.src = (w, h);
        }
        let hs = self.hist[0].size();
        if (hs.width, hs.height) != out {
            self.hist = [texture(device, "history", out.0, out.1, HIST, true), texture(device, "history", out.0, out.1, HIST, true)];
            reset = true;
        }
        // A jump (a teleport, waking up elsewhere): nothing to keep.
        if let Some(p) = self.prev_view {
            let d: f32 = (0..3).map(|i| (p[0][i] - frame.view[0][i]).powi(2)).sum::<f32>().sqrt();
            reset |= d > 8.0;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.depth, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            bytemuck::cast_slice(&frame.depth),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        let u = Uniforms {
            cur: frame.view,
            prev: self.prev_view.unwrap_or(frame.view),
            sizes: [w as f32, h as f32, out.0 as f32, out.1 as f32],
            flags: [reset as u32, 0, 0, 0],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        let (read, write) = (self.cur, 1 - self.cur);
        let view = |t: &wgpu::Texture| t.create_view(&Default::default());
        let (cv, dv, hv) = (view(color), view(&self.depth), view(&self.hist[read]));
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taa"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&cv) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&dv) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&hv) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        let target = view(&self.hist[write]);
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("taa"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipe);
            pass.set_bind_group(0, Some(&bind), &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([enc.finish()]);
        self.cur = write;
        self.prev_view = Some(frame.view);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn jitter_covers_the_pixel() {
        let pts: Vec<[f32; 2]> = (0..8).map(super::jitter).collect();
        assert!(pts.iter().all(|p| p[0].abs() <= 0.5 && p[1].abs() <= 0.5));
        let mean = pts.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / 8.0, a[1] + p[1] / 8.0]);
        assert!(mean[0].abs() < 0.1 && mean[1].abs() < 0.1, "{mean:?}");
    }
}
