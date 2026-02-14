use bytemuck::cast_slice;
use eframe::egui_wgpu::{self, wgpu};
use egui::{Color32, Pos2, Rect, mutex::Mutex};
use egui_wgpu::CallbackTrait;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct ShapeSpec {
    pub a: Pos2,
    pub b: Pos2,
    pub size: f32,
    pub color: Color32,
    pub kind: u32,
}

impl ShapeSpec {
    pub fn segment(a: Pos2, b: Pos2, width: f32, color: Color32) -> Self {
        Self {
            a,
            b,
            size: width,
            color,
            kind: 0,
        }
    }
    pub fn circle(center: Pos2, radius: f32, color: Color32) -> Self {
        Self {
            a: center,
            b: center,
            size: radius,
            color,
            kind: 1,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ShapeInstance {
    a: [f32; 2],
    b: [f32; 2],
    size: f32,
    color: [f32; 4],
    kind: u32,
}

pub struct GpuGraphRendererState {
    pub target_format: Option<wgpu::TextureFormat>,
    pub msaa_samples: u32,
    instances: Vec<ShapeInstance>,
}

impl Default for GpuGraphRendererState {
    fn default() -> Self {
        Self {
            target_format: None,
            msaa_samples: 1,
            instances: Vec::new(),
        }
    }
}

pub fn write_instances(shapes: &[ShapeSpec], state: &mut GpuGraphRendererState) {
    state.instances.clear();
    state.instances.reserve(shapes.len());
    for s in shapes {
        if s.size <= f32::EPSILON {
            continue;
        }
        let rgba = s.color.to_array();
        state.instances.push(ShapeInstance {
            a: [s.a.x, s.a.y],
            b: [s.b.x, s.b.y],
            size: s.size,
            color: [
                rgba[0] as f32 / 255.0,
                rgba[1] as f32 / 255.0,
                rgba[2] as f32 / 255.0,
                rgba[3] as f32 / 255.0,
            ],
            kind: s.kind,
        });
    }
}

pub fn paint_callback(rect: Rect, state: Arc<Mutex<GpuGraphRendererState>>) -> egui::PaintCallback {
    egui_wgpu::Callback::new_paint_callback(rect, GraphCallback { state })
}

struct GraphCallback {
    state: Arc<Mutex<GpuGraphRendererState>>,
}

struct GraphRenderResources {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    target_format: wgpu::TextureFormat,
    msaa_samples: u32,
}

impl GraphRenderResources {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat, msaa_samples: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpu_graph_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_graph.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_graph_uniform"),
            size: (std::mem::size_of::<[f32; 4]>() as u64).max(16),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu_graph_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_graph_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu_graph_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gpu_graph_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ShapeInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: (2 * std::mem::size_of::<f32>()) as u64,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: (4 * std::mem::size_of::<f32>()) as u64,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: (5 * std::mem::size_of::<f32>()) as u64,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint32,
                            offset: (9 * std::mem::size_of::<f32>()) as u64,
                            shader_location: 4,
                        },
                    ],
                }],
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
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_graph_vertex"),
            size: 4,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            vertex_buffer,
            vertex_capacity: 0,
            target_format,
            msaa_samples,
        }
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
        if state.instances.is_empty() {
            return Vec::new();
        }

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
            ));
        }

        let resources: &mut GraphRenderResources =
            resources.get_mut::<GraphRenderResources>().unwrap();

        let vertex_bytes = cast_slice(state.instances.as_slice());
        let required_size = vertex_bytes.len();
        if required_size > resources.vertex_capacity {
            let new_size = required_size.next_power_of_two().max(256) as u64;
            resources.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu_graph_vertex"),
                size: new_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            resources.vertex_capacity = new_size as usize;
        }
        queue.write_buffer(&resources.vertex_buffer, 0, vertex_bytes);

        let screen_size = [
            screen_descriptor.size_in_pixels[0] as f32 / screen_descriptor.pixels_per_point,
            screen_descriptor.size_in_pixels[1] as f32 / screen_descriptor.pixels_per_point,
        ];
        queue.write_buffer(&resources.uniform_buffer, 0, cast_slice(&screen_size));

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

        let state = self.state.lock();
        if state.instances.is_empty() {
            return;
        }

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
        render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
        render_pass.draw(0..6, 0..(state.instances.len() as u32));
    }
}
