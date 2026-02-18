use super::DrawnTrip;
use bytemuck::cast_slice;
use eframe::egui_wgpu::{self, wgpu};
use egui::{Pos2, Rect, mutex::Mutex};
use egui_wgpu::CallbackTrait;
use std::collections::HashMap;
use std::sync::Arc;

pub struct GpuTripRendererState {
    pub(crate) batches: Vec<GpuTripBatch>,
    pub(crate) combined_vertices: Vec<SegmentInstance>,
    pub(crate) straight_count: u32,
    pub(crate) target_format: Option<wgpu::TextureFormat>,
    pub(crate) msaa_samples: u32,
}

impl Default for GpuTripRendererState {
    fn default() -> Self {
        Self {
            batches: Vec::new(),
            combined_vertices: Vec::new(),
            straight_count: 0,
            target_format: None,
            msaa_samples: 1,
        }
    }
}

pub struct GpuTripBatch {
    pub width: f32,
    pub vertices: Vec<SegmentInstance>,
    pub curve_vertices: Vec<SegmentInstance>,
}

impl Default for GpuTripBatch {
    fn default() -> Self {
        Self {
            width: 1.0,
            vertices: Vec::new(),
            curve_vertices: Vec::new(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SegmentInstance {
    a: [f32; 2],
    b: [f32; 2],
    width: f32,
    color: [f32; 4],
    curve: u32,
}

pub fn write_vertices(trips: &[DrawnTrip], is_dark: bool, state: &mut GpuTripRendererState) {
    if state.batches.len() < trips.len() {
        state.batches.resize_with(trips.len(), Default::default);
    }
    state.combined_vertices.clear();
    state.straight_count = 0;
    let mut straight_vertices: Vec<SegmentInstance> = Vec::new();
    let mut curve_vertices: Vec<SegmentInstance> = Vec::new();
    for (batch, trip) in state.batches.iter_mut().zip(trips.iter()) {
        let color = trip.stroke.color.get(is_dark).to_array();
        let color = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            color[3] as f32 / 255.0,
        ];
        batch.width = trip.stroke.width;
        batch.vertices.clear();
        batch.curve_vertices.clear();
        for group in &trip.points {
            for (idx, window) in group
                .as_flattened()
                .windows(2)
                .enumerate()
                .filter(|(_, w)| w[0] != w[1])
            {
                let a = window[0];
                let b = window[1];
                if idx % 4 == 1 {
                    push_segment_instance(&mut batch.curve_vertices, a, b, batch.width, color, 1);
                } else {
                    push_segment_instance(&mut batch.vertices, a, b, batch.width, color, 0);
                }
            }
        }
        if !batch.vertices.is_empty() {
            straight_vertices.extend_from_slice(&batch.vertices);
        }
        if !batch.curve_vertices.is_empty() {
            curve_vertices.extend_from_slice(&batch.curve_vertices);
        }
    }
    state.straight_count = straight_vertices.len() as u32;
    state
        .combined_vertices
        .extend_from_slice(&straight_vertices);
    state.combined_vertices.extend_from_slice(&curve_vertices);
    state.batches.truncate(trips.len());
}

fn push_segment_instance(
    out: &mut Vec<SegmentInstance>,
    a: Pos2,
    b: Pos2,
    width: f32,
    color: [f32; 4],
    curve: u32,
) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f32::EPSILON {
        return;
    }
    out.push(SegmentInstance {
        a: [a.x, a.y],
        b: [b.x, b.y],
        width,
        color,
        curve,
    });
}

pub fn paint_callback(rect: Rect, state: Arc<Mutex<GpuTripRendererState>>) -> egui::PaintCallback {
    egui_wgpu::Callback::new_paint_callback(rect, TripCallback { state })
}

struct TripCallback {
    state: Arc<Mutex<GpuTripRendererState>>,
}

struct TripRenderResources {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    target_format: wgpu::TextureFormat,
    msaa_samples: u32,
}

#[derive(Default)]
struct TripRenderResourceMap {
    by_state: HashMap<usize, TripRenderResources>,
}

impl TripRenderResources {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat, msaa_samples: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpu_trip_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_trip.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_trip_uniform"),
            size: (std::mem::size_of::<[f32; 4]>() as u64).max(16),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu_trip_bind_group_layout"),
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
            label: Some("gpu_trip_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu_trip_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gpu_trip_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SegmentInstance>() as u64,
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
            label: Some("gpu_trip_vertex"),
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

impl CallbackTrait for TripCallback {
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
        if state.combined_vertices.is_empty() {
            return Vec::new();
        }

        let state_key = Arc::as_ptr(&self.state) as usize;

        if resources.get::<TripRenderResourceMap>().is_none() {
            resources.insert(TripRenderResourceMap::default());
        }

        let resources_map: &mut TripRenderResourceMap =
            resources.get_mut::<TripRenderResourceMap>().unwrap();

        let needs_rebuild = match resources_map.by_state.get(&state_key) {
            Some(existing) => {
                existing.target_format != target_format
                    || existing.msaa_samples != state.msaa_samples
            }
            None => true,
        };

        if needs_rebuild {
            resources_map.by_state.insert(
                state_key,
                TripRenderResources::new(
                device,
                target_format,
                state.msaa_samples,
                ),
            );
        }

        let resources: &mut TripRenderResources =
            resources_map.by_state.get_mut(&state_key).unwrap();

        let vertex_bytes = cast_slice(state.combined_vertices.as_slice());
        let required_size = vertex_bytes.len();
        if required_size > resources.vertex_capacity {
            let new_size = required_size.next_power_of_two().max(256) as u64;
            resources.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu_trip_vertex"),
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
            0.0,
            0.0,
        ];
        let viewport_bytes = cast_slice(&screen_size);
        queue.write_buffer(&resources.uniform_buffer, 0, viewport_bytes);

        Vec::new()
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(resources_map) = resources.get::<TripRenderResourceMap>() else {
            return;
        };
        let state_key = Arc::as_ptr(&self.state) as usize;
        let Some(resources) = resources_map.by_state.get(&state_key) else {
            return;
        };

        let state = self.state.lock();
        if state.combined_vertices.is_empty() {
            return;
        }

        let clip = info.clip_rect_in_pixels();
        let clip_left = clip.left_px.max(0) as u32;
        let clip_top = clip.top_px.max(0) as u32;
        let clip_width = clip.width_px.max(0) as u32;
        let clip_height = clip.height_px.max(0) as u32;
        render_pass.set_viewport(
            0.0,
            0.0,
            info.screen_size_px[0] as f32,
            info.screen_size_px[1] as f32,
            0.0,
            1.0,
        );
        render_pass.set_scissor_rect(clip_left, clip_top, clip_width, clip_height);
        render_pass.set_pipeline(&resources.pipeline);
        render_pass.set_bind_group(0, &resources.bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
        let instance_count = state.combined_vertices.len() as u32;
        let straight_count = state.straight_count.min(instance_count);
        let curve_count = instance_count.saturating_sub(straight_count);
        if straight_count > 0 {
            render_pass.draw(0..6, 0..straight_count);
        }
        if curve_count > 0 {
            render_pass.draw(0..48, straight_count..instance_count);
        }
    }
}
