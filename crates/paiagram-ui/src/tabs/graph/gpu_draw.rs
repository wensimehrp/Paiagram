use std::sync::Arc;

use eframe::egui_wgpu::{self, wgpu};
use egui::mutex::Mutex;
use egui::{Color32, Pos2, Rect, Vec2};
use egui_wgpu::CallbackTrait;
use vello::kurbo::{Affine, BezPath, Circle, Line};
use vello::peniko::Color;
use vello::peniko::kurbo::Stroke;
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};

pub(crate) struct GpuGraphRendererState {
    pub(crate) target_format: Option<wgpu::TextureFormat>,
    pub(crate) msaa_samples: u32,
    pub(crate) scene: Scene,
}

impl Default for GpuGraphRendererState {
    fn default() -> Self {
        Self {
            target_format: None,
            msaa_samples: 1,
            scene: Scene::new(),
        }
    }
}

pub(crate) fn paint_callback(
    rect: Rect,
    state: Arc<Mutex<GpuGraphRendererState>>,
) -> egui::PaintCallback {
    egui_wgpu::Callback::new_paint_callback(rect, GraphCallback { state })
}

struct GraphCallback {
    state: Arc<Mutex<GpuGraphRendererState>>,
}

struct GraphRenderResources {
    renderer: Mutex<Renderer>,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,

    texture_extent: wgpu::Extent3d,
    texture_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,

    target_format: wgpu::TextureFormat,
    msaa_samples: u32,
}

impl GraphRenderResources {
    fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        msaa_samples: u32,
        width: u32,
        height: u32,
    ) -> Self {
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::all(),
                num_init_threads: None,
                pipeline_cache: None,
            },
        )
        .expect("Failed to create Vello renderer");

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpu_graph_blit_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blit.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu_graph_blit_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu_graph_blit_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            ..Default::default()
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gpu_graph_blit_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: msaa_samples.max(1),
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("gpu_graph_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let (texture_extent, texture_view, bind_group) =
            Self::create_texture(device, &bind_group_layout, &sampler, width, height);

        Self {
            renderer: Mutex::new(renderer),
            pipeline,
            bind_group_layout,
            sampler,
            texture_extent,
            texture_view,
            bind_group,
            target_format,
            msaa_samples,
        }
    }

    fn create_texture(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) -> (wgpu::Extent3d, wgpu::TextureView, wgpu::BindGroup) {
        let texture_extent = wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpu_graph_vello_target"),
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_graph_blit_bind_group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        (texture_extent, texture_view, bind_group)
    }
}

impl CallbackTrait for GraphCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let state = self.state.lock();
        let Some(target_format) = state.target_format else {
            return Vec::new();
        };

        let width = screen_descriptor.size_in_pixels[0];
        let height = screen_descriptor.size_in_pixels[1];

        let needs_rebuild = match resources.get::<GraphRenderResources>() {
            Some(existing) => {
                existing.target_format != target_format
                    || existing.msaa_samples != state.msaa_samples
            }
            None => true,
        };

        if needs_rebuild {
            resources.insert(GraphRenderResources::new(
                device,
                target_format,
                state.msaa_samples,
                width,
                height,
            ));
        }

        let resources = resources.get_mut::<GraphRenderResources>().unwrap();

        if resources.texture_extent.width != width || resources.texture_extent.height != height {
            let (ext, view, bg) = GraphRenderResources::create_texture(
                device,
                &resources.bind_group_layout,
                &resources.sampler,
                width,
                height,
            );
            resources.texture_extent = ext;
            resources.texture_view = view;
            resources.bind_group = bg;
        }

        let render_params = RenderParams {
            base_color: Color::TRANSPARENT,
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };

        let mut transformed_scene = Scene::new();
        transformed_scene.append(
            &state.scene,
            Some(Affine::scale(screen_descriptor.pixels_per_point as f64)),
        );

        resources
            .renderer
            .lock()
            .render_to_texture(
                device,
                queue,
                &transformed_scene,
                &resources.texture_view,
                &render_params,
            )
            .expect("Failed to render Vello scene to texture");

        Vec::new()
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(resources) = resources.get::<GraphRenderResources>() else {
            return;
        };

        let clip = info.clip_rect_in_pixels();
        render_pass.set_viewport(
            0.0,
            0.0,
            info.screen_size_px[0] as f32,
            info.screen_size_px[1] as f32,
            0.0,
            1.0,
        );
        render_pass.set_scissor_rect(
            clip.left_px.max(0) as u32,
            clip.top_px.max(0) as u32,
            clip.width_px.max(0) as u32,
            clip.height_px.max(0) as u32,
        );
        render_pass.set_pipeline(&resources.pipeline);
        render_pass.set_bind_group(0, &resources.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

pub(crate) fn to_vello_color(color: Color32) -> Color {
    Color::from_rgba8(color.r(), color.g(), color.b(), color.a())
}

pub(crate) fn draw_segment(scene: &mut Scene, a: Pos2, b: Pos2, width: f32, color: Color32) {
    let line = Line::new((a.x as f64, a.y as f64), (b.x as f64, b.y as f64));
    scene.stroke(
        &Stroke::new(width as f64),
        Affine::IDENTITY,
        to_vello_color(color),
        None,
        &line,
    );
}

pub(crate) fn draw_circle(scene: &mut Scene, center: Pos2, radius: f32, color: Color32) {
    let circle = Circle::new((center.x as f64, center.y as f64), radius as f64);
    scene.fill(
        vello::peniko::Fill::NonZero,
        Affine::IDENTITY,
        to_vello_color(color),
        None,
        &circle,
    );
}

pub(crate) fn draw_stealth_arrow(
    scene: &mut Scene,
    from: Pos2,
    to: Pos2,
    center: Pos2,
    color: Color32,
) {
    let direction = to - from;
    let direction_len = direction.length();
    let unit_direction = if direction_len > f32::EPSILON {
        direction / direction_len
    } else {
        Vec2::X
    };

    let dir = vello::kurbo::Vec2::new(unit_direction.x as f64, unit_direction.y as f64);
    let n = vello::kurbo::Vec2::new(-dir.y, dir.x);

    let arrow_len = 14.0;
    let arrow_width = arrow_len * (12.0 / 14.0);
    let stealth = 0.2;
    let tip_x = arrow_len * (1.0 - stealth) * 0.5;
    let left_x = -arrow_len * (1.0 + stealth) * 0.5;
    let indent_x = -arrow_len * (1.0 - stealth) * 0.5;
    let half_w = arrow_width * 0.5;

    let points = [
        (tip_x, 0.0),
        (left_x, half_w),
        (indent_x, 0.0),
        (left_x, -half_w),
    ];

    let center_pt = vello::kurbo::Point::new(center.x as f64, center.y as f64);

    let mut path = BezPath::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        let pt = center_pt + dir * x + n * y;
        if i == 0 {
            path.move_to(pt);
        } else {
            path.line_to(pt);
        }
    }
    path.close_path();

    scene.fill(
        vello::peniko::Fill::NonZero,
        Affine::IDENTITY,
        to_vello_color(color),
        None,
        &path,
    );
}
