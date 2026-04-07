use bytemuck::{Pod, Zeroable};
use bytemuck::{bytes_of, cast_slice};
use eframe::egui_wgpu::{self, wgpu};
use egui::Color32;
use egui::{Rect, mutex::Mutex};
use egui_wgpu::CallbackTrait;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::BufferDescriptor;
use wgpu::{BufferBindingType, BufferUsages, ShaderStages};

const LANE_COUNT: u32 = 2;

pub(crate) struct GpuTripRendererState {
    entries : Vec<Entry>,
    pub trips: Vec<Trip>,
    pub stations: Vec<f32>,
    pub uniforms: Uniforms,
    pub target_format: Option<wgpu::TextureFormat>,
    pub msaa_samples: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Entry {
    arr_secs: i32,
    dep_secs: i32,
    station_index: u32,
    _track_index: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct Trip {
    pub color: [u8; 4],
    pub width: f32,
    len: u32,
    start_idx: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Uniforms {
    pub ticks_min: i64,
    pub y_min: f64,
    pub screen_size: [f32; 2],
    pub x_per_unit: f32,
    pub y_per_unit: f32,
    pub screen_origin: [f32; 2],
    pub repeat_interval_ticks: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Default)]
struct GpuUniforms {
    ticks_min: i32,
    y_min: f32,
    screen_size: [f32; 2],
    x_per_unit: f32,
    y_per_unit: f32,
    screen_origin: [f32; 2],
    repeat_interval_ticks: i32,
    repeat_from: i32,
    repeat_to: i32,
    source_instance_count: u32,
    feathering_radius: f32,
    pixels_per_point: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct InstanceMapEntry {
    trip_index: u32,
    local_segment: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct VisibleSegment {
    p0: [f32; 2],
    p1: [f32; 2],
    half_width: f32,
    curve_a_or_nan: f32,
    curve_b: f32,
    color: [u8; 4],
}

const SEGMENT_MESH_INDEX_COUNT: u32 = 6;

impl Default for GpuTripRendererState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            trips: Vec::new(),
            stations: Vec::new(),
            uniforms: Uniforms::default(),
            target_format: None,
            msaa_samples: 1,
        }
    }
}

pub fn rewrite_trip_cache(
    cache: &super::TripCache,
    stations: impl Iterator<Item = f32>,
    state: &mut GpuTripRendererState,
) {
    state.trips.clear();
    state.entries.clear();
    state.stations.clear();
    state.stations.extend(stations);
    for (_trip_entity, lines) in cache.iter() {
        for line in lines {
            state.trips.push(Trip {
                color: Color32::MAGENTA.to_array(),
                width: 1.0,
                len: line.len() as u32,
                start_idx: state.entries.len() as u32 + 1,
            });
            for entry in line {
                state.entries.push(Entry {
                    dep_secs: entry.dep.seconds(),
                    arr_secs: entry.arr.seconds(),
                    station_index: entry.station_index as u32,
                    _track_index: 0,
                });
            }
            if let Some(mut last) = state.entries.last().copied() {
                last.arr_secs = last.dep_secs;
                state.entries.push(last);
            }
        }
    }
}

pub fn paint_callback(rect: Rect, state: Arc<Mutex<GpuTripRendererState>>) -> egui::PaintCallback {
    egui_wgpu::Callback::new_paint_callback(rect, TripCallback { state })
}

struct TripCallback {
    state: Arc<Mutex<GpuTripRendererState>>,
}

struct TripRenderResources {
    pipeline: wgpu::RenderPipeline,
    compute_pipeline: wgpu::ComputePipeline,
    compute_bind_group_layout: wgpu::BindGroupLayout,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group_layout: wgpu::BindGroupLayout,
    render_bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    entry_buffer: wgpu::Buffer,
    trip_buffer: wgpu::Buffer,
    station_buffer: wgpu::Buffer,
    visible_segment_buffer: wgpu::Buffer,
    source_instance_map_buffer: wgpu::Buffer,
    draw_instance_count: u32,
    target_format: wgpu::TextureFormat,
    msaa_samples: u32,
}

#[derive(Default)]
struct TripRenderResourceMap {
    by_state: HashMap<usize, TripRenderResources>,
}

fn make_storage_buffer_entry(
    label: &'static str,
    size: u64,
    device: &wgpu::Device,
) -> wgpu::Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn make_rw_storage_buffer_entry(
    label: &'static str,
    size: u64,
    extra_usage: BufferUsages,
    device: &wgpu::Device,
) -> wgpu::Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | extra_usage,
        mapped_at_creation: false,
    })
}

impl TripRenderResources {
    fn rebuild_bind_groups(&mut self, device: &wgpu::Device) {
        self.compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_trip_compute_bind_group"),
            layout: &self.compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.entry_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.trip_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.station_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.source_instance_map_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.visible_segment_buffer.as_entire_binding(),
                },
            ],
        });

        self.render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_trip_render_bind_group"),
            layout: &self.render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.visible_segment_buffer.as_entire_binding(),
                },
            ],
        });
    }

    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat, msaa_samples: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpu_trip_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_trip.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_trip_uniform"),
            size: std::mem::size_of::<GpuUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let entry_buffer = make_storage_buffer_entry("entries", 256, device);
        let trip_buffer = make_storage_buffer_entry("trips", 256, device);
        let station_buffer = make_storage_buffer_entry("stations", 256, device);
        let visible_segment_buffer =
            make_rw_storage_buffer_entry("visible_segment", 256, BufferUsages::empty(), device);
        let source_instance_map_buffer =
            make_storage_buffer_entry("source_instance_map", 256, device);

        let ro_storage_buffer_layout_entry =
            |binding: u32, visibility: ShaderStages| wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            };

        let rw_storage_buffer_layout_entry =
            |binding: u32, visibility: ShaderStages| wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            };

        let compute_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu_trip_compute_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // entry_buffer
                ro_storage_buffer_layout_entry(1, ShaderStages::COMPUTE),
                // trip_buffer
                ro_storage_buffer_layout_entry(2, ShaderStages::COMPUTE),
                // station_buffer
                ro_storage_buffer_layout_entry(3, ShaderStages::COMPUTE),
                // source_instance_map_buffer
                ro_storage_buffer_layout_entry(4, ShaderStages::COMPUTE),
                // visible_segment_buffer (compute write view)
                rw_storage_buffer_layout_entry(5, ShaderStages::COMPUTE),
            ],
        });

        let render_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu_trip_render_bind_group_layout"),
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
                ro_storage_buffer_layout_entry(6, ShaderStages::VERTEX),
            ],
        });

        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_trip_compute_bind_group"),
            layout: &compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: entry_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: trip_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: station_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: source_instance_map_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: visible_segment_buffer.as_entire_binding(),
                },
            ],
        });

        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_trip_render_bind_group"),
            layout: &render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: visible_segment_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu_trip_pipeline_layout"),
            bind_group_layouts: &[Some(&render_bind_group_layout)],
            ..Default::default()
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gpu_trip_pipeline"),
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

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gpu_trip_compute_pipeline_layout"),
                bind_group_layouts: &[Some(&compute_bind_group_layout)],
                ..Default::default()
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("gpu_trip_compute_pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Self {
            pipeline,
            compute_pipeline,
            compute_bind_group_layout,
            compute_bind_group,
            render_bind_group_layout,
            render_bind_group,
            uniform_buffer,
            entry_buffer,
            trip_buffer,
            station_buffer,
            visible_segment_buffer,
            source_instance_map_buffer,
            draw_instance_count: 0,
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
        egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let state = self.state.lock();
        let Some(target_format) = state.target_format else {
            return Vec::new();
        };

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
                TripRenderResources::new(device, target_format, state.msaa_samples),
            );
        }

        let resources: &mut TripRenderResources =
            resources_map.by_state.get_mut(&state_key).unwrap();

        let mut needs_rebind = false;

        let mut data_tick_min = i32::MAX;
        let mut data_tick_max = i32::MIN;
        for entry in &state.entries {
            let arr_ticks = entry.arr_secs.saturating_mul(100);
            let dep_ticks = entry.dep_secs.saturating_mul(100);
            data_tick_min = data_tick_min.min(arr_ticks.min(dep_ticks));
            data_tick_max = data_tick_max.max(arr_ticks.max(dep_ticks));
        }
        if data_tick_min > data_tick_max {
            data_tick_min = 0;
            data_tick_max = 0;
        }

        // Entries
        {
            let entry_bytes = cast_slice(state.entries.as_slice());
            let required_size = entry_bytes.len();
            if required_size as u64 > resources.entry_buffer.size() {
                let new_size = required_size.next_power_of_two().max(256) as u64;
                resources.entry_buffer = make_storage_buffer_entry("entries", new_size, device);
                needs_rebind = true;
            }
            queue.write_buffer(&resources.entry_buffer, 0, entry_bytes);
        }

        // Trips
        {
            let trip_bytes = cast_slice(state.trips.as_slice());
            let required_size = trip_bytes.len();
            if required_size as u64 > resources.trip_buffer.size() {
                let new_size = required_size.next_power_of_two().max(256) as u64;
                resources.trip_buffer = make_storage_buffer_entry("trips", new_size, device);
                needs_rebind = true;
            }
            queue.write_buffer(&resources.trip_buffer, 0, trip_bytes);
        }

        // Stations
        {
            let station_bytes = cast_slice(state.stations.as_slice());
            let required_size = station_bytes.len();
            if required_size as u64 > resources.station_buffer.size() {
                let new_size = required_size.next_power_of_two().max(256) as u64;
                resources.station_buffer = make_storage_buffer_entry("stations", new_size, device);
                needs_rebind = true;
            }
            queue.write_buffer(&resources.station_buffer, 0, station_bytes);
        }

        // Direct index map source: instance_index -> (trip_index, local_segment)
        let mut instance_map: Vec<InstanceMapEntry> = Vec::new();
        for (trip_index, trip) in state.trips.iter().enumerate() {
            let seg_count = trip.len.saturating_sub(1);
            for local_segment in 0..seg_count {
                instance_map.push(InstanceMapEntry {
                    trip_index: trip_index as u32,
                    local_segment,
                });
            }
        }
        let map_bytes = cast_slice(instance_map.as_slice());
        let required_size = map_bytes.len();
        if required_size as u64 > resources.source_instance_map_buffer.size() {
            let new_size = required_size.next_power_of_two().max(256) as u64;
            resources.source_instance_map_buffer =
                make_storage_buffer_entry("source_instance_map", new_size, device);
            needs_rebind = true;
        }
        queue.write_buffer(&resources.source_instance_map_buffer, 0, map_bytes);

        if needs_rebind {
            resources.rebuild_bind_groups(device);
        }

        // uniforms
        let uniforms = GpuUniforms {
            ticks_min: state
                .uniforms
                .ticks_min
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y_min: state.uniforms.y_min as f32,
            screen_size: [
                screen_descriptor.size_in_pixels[0] as f32 / screen_descriptor.pixels_per_point,
                screen_descriptor.size_in_pixels[1] as f32 / screen_descriptor.pixels_per_point,
            ],
            x_per_unit: state.uniforms.x_per_unit,
            y_per_unit: state.uniforms.y_per_unit,
            screen_origin: state.uniforms.screen_origin,
            ..Default::default()
        };
        let visible_ticks_min = uniforms.ticks_min;
        let visible_ticks_max = uniforms
            .ticks_min
            .saturating_add((uniforms.screen_size[0] * uniforms.x_per_unit) as i32);
        let repeat_interval = state.uniforms.repeat_interval_ticks.max(0);
        let (repeat_from, repeat_to) = if repeat_interval > 0 {
            (
                (visible_ticks_min - data_tick_max).div_euclid(repeat_interval),
                (visible_ticks_max - data_tick_min).div_euclid(repeat_interval),
            )
        } else {
            (0, 0)
        };
        let repeat_count = if repeat_interval > 0 {
            (repeat_to - repeat_from + 1).max(1) as usize
        } else {
            1usize
        };
        let lane_count = LANE_COUNT as usize;
        // in case if we exceed the 128MB VRAM limitation
        let source_slot_count = instance_map.len().saturating_mul(lane_count);
        let max_storage_binding_bytes = device.limits().max_storage_buffer_binding_size as usize;
        let max_visible_segments =
            (max_storage_binding_bytes / std::mem::size_of::<VisibleSegment>()).max(1);
        let max_source_slots_per_repeat = (max_visible_segments / repeat_count / lane_count) * lane_count;
        let source_slots_per_repeat = source_slot_count.min(max_source_slots_per_repeat);
        let visible_capacity = source_slots_per_repeat.saturating_mul(repeat_count);
        resources.draw_instance_count = visible_capacity.min(u32::MAX as usize) as u32;
        let visible_required_size = visible_capacity
            .saturating_mul(std::mem::size_of::<VisibleSegment>())
            .max(256) as u64;
        if visible_required_size > resources.visible_segment_buffer.size() {
            let new_size = visible_required_size.next_power_of_two().max(256);
            resources.visible_segment_buffer = make_rw_storage_buffer_entry(
                "visible_segment",
                new_size,
                BufferUsages::empty(),
                device,
            );
            needs_rebind = true;
        }
        if needs_rebind {
            resources.rebuild_bind_groups(device);
        }

        let uniforms = GpuUniforms {
            repeat_interval_ticks: repeat_interval,
            repeat_from,
            repeat_to,
            source_instance_count: source_slots_per_repeat.min(u32::MAX as usize) as u32,
            feathering_radius: 1.2 / screen_descriptor.pixels_per_point,
            pixels_per_point: screen_descriptor.pixels_per_point,
            ..uniforms
        };
        let uniform_bytes = bytes_of(&uniforms);
        queue.write_buffer(&resources.uniform_buffer, 0, uniform_bytes);

        // With direct draw we render a fixed slot count, so clear unused slots each frame.
        egui_encoder.clear_buffer(&resources.visible_segment_buffer, 0, None);

        {
            let mut pass = egui_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_trip_cull_compute_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&resources.compute_pipeline);
            pass.set_bind_group(0, &resources.compute_bind_group, &[]);
            let workgroup_size = 64u32;
            let workgroup_count_x =
                (uniforms.source_instance_count.saturating_add(workgroup_size - 1)) / workgroup_size;
            let workgroup_count_y = repeat_count.min(u32::MAX as usize) as u32;
            if workgroup_count_x > 0 && workgroup_count_y > 0 {
                pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
            }
        }

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
        render_pass.set_bind_group(0, &resources.render_bind_group, &[]);
        render_pass.draw(0..SEGMENT_MESH_INDEX_COUNT, 0..resources.draw_instance_count);
    }
}
