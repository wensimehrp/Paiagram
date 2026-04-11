use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::Entity;
use bytemuck::{Pod, Zeroable};
use bytemuck::{bytes_of, cast_slice};
use eframe::egui_wgpu::{self, wgpu};
use eframe::wgpu::include_wgsl;
use egui::{Rect, mutex::Mutex};
use egui_wgpu::CallbackTrait;
use paiagram_core::settings::{AntialiasingMode, LevelOfDetailMode};
use paiagram_core::trip::TripClass;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::BufferDescriptor;
use wgpu::{BufferBindingType, BufferUsages, ShaderStages};

pub(crate) struct GpuTripRendererState {
    entries: Vec<Entry>,
    styles: Vec<u32>,
    class_style_index: EntityHashMap<u16>,
    data_tick_min: i32,
    data_tick_max: i32,
    entries_dirty: bool,
    stations_dirty: bool,
    pub stations: Vec<f32>,
    pub uniforms: Uniforms,
    pub target_format: Option<wgpu::TextureFormat>,
    pub msaa_samples: u32,
    pub antialiasing_mode: AntialiasingMode,
    pub level_of_detail_mode: LevelOfDetailMode,
}

/// field0: .......A AAAAAAAA AAAAAAAA AAAAAAAA
/// field1: ......ND DDDDDDDD DDDDDDDD DDDDDDDD
/// field2: SSSSSSSS SSSSSSSS RRRRRRRR RRRRRRRR
/// field3: ........ ........ ........ IIIIIIII
///
/// A: arrival seconds (signed). 2^25 ~= 388 days (194 days on each side)
/// D: departure seconds (signed). Same as arrival seconds.
/// S: station index
/// R: track index
/// I: style table index (8-bit, 0..=255).
///    style data (width + colour) is stored in uniform buffer.
/// N: whether the current entry connects to the next entry
///    when this bit is set it connects to the next entry.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Entry {
    pub field0: u32,
    pub field1: u32,
    pub field2: u32,
    pub field3: u32,
}

impl Entry {
    fn signed_25(value: u32) -> i32 {
        ((value << 7) as i32) >> 7
    }

    fn arr_secs(self) -> i32 {
        Self::signed_25(self.field0 & 0x01ff_ffff)
    }

    fn dep_secs(self) -> i32 {
        Self::signed_25(self.field1 & 0x01ff_ffff)
    }

    fn pack_signed_25(value: i32) -> u32 {
        const MIN: i32 = -(1 << 24);
        const MAX: i32 = (1 << 24) - 1;
        (value.clamp(MIN, MAX) as u32) & 0x01ff_ffff
    }

    fn new(
        arr_secs: i32,
        dep_secs: i32,
        station_index: u16,
        track_index: u16,
        connects_to_next: bool,
        style_index: u16,
    ) -> Self {
        let field0 = Self::pack_signed_25(arr_secs);
        let field1 = Self::pack_signed_25(dep_secs) | ((connects_to_next as u32) << 25);
        let field2 = ((station_index as u32) << 16) | (track_index as u32);
        let field3 = (style_index as u32) & 0xFF;

        Self {
            field0,
            field1,
            field2,
            field3,
        }
    }
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
#[derive(Copy, Clone, Pod, Zeroable)]
struct GpuUniforms {
    ticks_min: i32,
    y_min: f32,
    screen_size: [f32; 2],
    x_per_unit: f32,
    y_per_unit: f32,
    screen_origin: [f32; 2],
    repeat_interval_ticks: i32,
    repeat_from: i32,
    repeat_count: u32,
    source_instance_count: u32,
    style_count: u32,
    feathering_radius: f32,
    lod_stride: u32,
    _pad0: u32,
    styles: [[u32; 4]; STYLE_TABLE_CAPACITY],
}

impl Default for GpuUniforms {
    fn default() -> Self {
        Self {
            ticks_min: 0,
            y_min: 0.0,
            screen_size: [0.0, 0.0],
            x_per_unit: 0.0,
            y_per_unit: 0.0,
            screen_origin: [0.0, 0.0],
            repeat_interval_ticks: 0,
            repeat_from: 0,
            repeat_count: 1,
            source_instance_count: 0,
            style_count: 0,
            feathering_radius: 0.0,
            lod_stride: 1,
            _pad0: 0,
            styles: [[0, 0, 0, 0]; STYLE_TABLE_CAPACITY],
        }
    }
}

const STYLE_TABLE_CAPACITY: usize = 256;
const COMPUTE_WORKGROUP_SIZE: u32 = 64;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GpuSegment {
    p0: [f32; 2],
    p1: [f32; 2],
    half_width: f32,
    nx: f32,
    ny: f32,
    pad0: f32,
    color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Default, Pod, Zeroable)]
struct DrawIndirectArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

const fn pack_style(width_steps: u8, color_rgb: [u8; 3]) -> u32 {
    ((width_steps as u32) << 24)
        | (color_rgb[0] as u32)
        | ((color_rgb[1] as u32) << 8)
        | ((color_rgb[2] as u32) << 16)
}

impl Default for GpuTripRendererState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            styles: vec![pack_style(4, [0, 0, 0])],
            class_style_index: EntityHashMap::new(),
            data_tick_min: 0,
            data_tick_max: 0,
            entries_dirty: true,
            stations_dirty: true,
            stations: Vec::new(),
            uniforms: Uniforms::default(),
            target_format: None,
            msaa_samples: 1,
            antialiasing_mode: AntialiasingMode::default(),
            level_of_detail_mode: LevelOfDetailMode::default(),
        }
    }
}

pub fn upload_trip_strokes(
    strokes: impl Iterator<Item = (Entity, f32, [u8; 3])>,
    state: &mut GpuTripRendererState,
) {
    for (class_entity, width, color_rgb) in strokes {
        let width_steps = (width * 4.0).round().clamp(0.0, 255.0) as u8;
        let packed = pack_style(width_steps, color_rgb);

        let style_index =
            if let Some(existing) = state.class_style_index.get(&class_entity).copied() {
                if let Some(slot) = state.styles.get_mut(existing as usize) {
                    *slot = packed;
                }
                existing
            } else {
                let next = state.styles.len();
                if next >= STYLE_TABLE_CAPACITY {
                    0
                } else {
                    let idx = next as u16;
                    state.styles.push(packed);
                    state.class_style_index.insert(class_entity, idx);
                    idx
                }
            };

        if style_index == 0 {
            if let Some(slot) = state.styles.get_mut(0) {
                *slot = packed;
            }
        }
    }
}

pub fn rewrite_trip_cache(
    cache: &super::TripCache,
    stations: impl Iterator<Item = f32>,
    class_lookup: &bevy::prelude::Query<&TripClass>,
    state: &mut GpuTripRendererState,
) {
    const MAX_STATION_COUNT: usize = (u16::MAX as usize) + 1;
    const DEFAULT_STYLE_INDEX: u16 = 0;

    state.entries.clear();
    state.stations.clear();
    state.stations.extend(stations);
    state.entries_dirty = true;
    state.stations_dirty = true;

    if state.styles.is_empty() {
        state.styles.push(pack_style(4, [0, 0, 0]));
    }
    if state.stations.len() > MAX_STATION_COUNT {
        state.stations.truncate(MAX_STATION_COUNT);
    }

    for (trip_entity, lines) in cache.iter() {
        let style_index = class_lookup
            .get(*trip_entity)
            .ok()
            .and_then(|class_entity| state.class_style_index.get(&class_entity.0))
            .copied()
            .unwrap_or(DEFAULT_STYLE_INDEX);

        for (last, rest) in lines.iter().filter_map(|it| it.split_last()) {
            for entry in rest {
                let Ok(station_index) = u16::try_from(entry.station_index) else {
                    continue;
                };

                state.entries.push(Entry::new(
                    entry.arr.seconds(),
                    entry.dep.seconds(),
                    station_index,
                    0,
                    true,
                    style_index,
                ));
            }

            let Ok(station_index) = u16::try_from(last.station_index) else {
                continue;
            };

            let last_entry = Entry::new(
                last.arr.seconds(),
                last.dep.seconds(),
                station_index,
                0,
                false,
                style_index,
            );

            let empty_entry = Entry::new(0, 0, 0, 0, false, 0);

            match state.entries.len() % 4 {
                0 => {
                    state.entries.push(last_entry); // <-
                    state.entries.push(empty_entry);
                    state.entries.push(empty_entry);
                    state.entries.push(empty_entry);
                    // next entry start
                }
                1 => {
                    state.entries.push(last_entry);
                    state.entries.push(last_entry);
                    state.entries.push(last_entry);
                    state.entries.push(last_entry); // <-
                    state.entries.push(empty_entry);
                    state.entries.push(empty_entry);
                    state.entries.push(empty_entry);
                    // next entry start
                }
                2 => {
                    state.entries.push(last_entry);
                    state.entries.push(last_entry);
                    state.entries.push(last_entry); // <-
                    state.entries.push(empty_entry);
                    state.entries.push(empty_entry);
                    state.entries.push(empty_entry);
                    // next entry start
                }
                3 => {
                    state.entries.push(last_entry);
                    state.entries.push(last_entry); // <-
                    state.entries.push(empty_entry);
                    state.entries.push(empty_entry);
                    state.entries.push(empty_entry);
                    // next entry start
                }
                _ => unreachable!("mod 4 cannot produce numbers greater than 4"),
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
    uniform_buffer: wgpu::Buffer,
    entry_buffer: wgpu::Buffer,
    station_buffer: wgpu::Buffer,
    segment_buffer: wgpu::Buffer,
    indirect_buffer: wgpu::Buffer,
    render_bind_group_layout: wgpu::BindGroupLayout,
    compute_bind_group_layout: wgpu::BindGroupLayout,
    render_bind_group: wgpu::BindGroup,
    compute_bind_group: wgpu::BindGroup,
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
    usages: BufferUsages,
    device: &wgpu::Device,
) -> wgpu::Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size,
        usage: usages,
        mapped_at_creation: false,
    })
}

impl TripRenderResources {
    fn rebuild_bind_groups(&mut self, device: &wgpu::Device) {
        self.render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_trip_render_bind_group"),
            layout: &self.render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.segment_buffer.as_entire_binding(),
                },
            ],
        });

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
                    resource: self.station_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.segment_buffer.as_entire_binding(),
                },
            ],
        });
    }

    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat, msaa_samples: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpu_trip_shader"),
            source: include_wgsl!("gpu_trip.wgsl").source,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_trip_uniform"),
            size: std::mem::size_of::<GpuUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let entry_buffer = make_storage_buffer_entry(
            "entries",
            256,
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            device,
        );
        let station_buffer = make_storage_buffer_entry(
            "stations",
            256,
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            device,
        );
        let segment_buffer = make_storage_buffer_entry(
            "segments",
            256,
            BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_DST,
            device,
        );
        let indirect_buffer = make_storage_buffer_entry(
            "gpu_trip_indirect",
            std::mem::size_of::<DrawIndirectArgs>() as u64,
            BufferUsages::INDIRECT | BufferUsages::COPY_DST,
            device,
        );

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

        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gpu_trip_render_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    ro_storage_buffer_layout_entry(4, ShaderStages::VERTEX),
                ],
            });

        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                    ro_storage_buffer_layout_entry(1, ShaderStages::COMPUTE),
                    ro_storage_buffer_layout_entry(2, ShaderStages::COMPUTE),
                    rw_storage_buffer_layout_entry(3, ShaderStages::COMPUTE),
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
                    binding: 4,
                    resource: segment_buffer.as_entire_binding(),
                },
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
                    resource: station_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: segment_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu_trip_pipeline_layout"),
            bind_group_layouts: &[Some(&render_bind_group_layout)],
            ..Default::default()
        });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gpu_trip_compute_pipeline_layout"),
                bind_group_layouts: &[Some(&compute_bind_group_layout)],
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

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("gpu_trip_compute"),
            layout: Some(&compute_pipeline_layout),
            module: &shader,
            entry_point: Some("cs_main"),
            cache: None,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        });

        Self {
            pipeline,
            compute_pipeline,
            uniform_buffer,
            entry_buffer,
            station_buffer,
            segment_buffer,
            indirect_buffer,
            render_bind_group_layout,
            compute_bind_group_layout,
            render_bind_group,
            compute_bind_group,
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
        let mut state = self.state.lock();
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

        if state.entries_dirty {
            let mut data_tick_min = i32::MAX;
            let mut data_tick_max = i32::MIN;
            for entry in &state.entries {
                let arr_ticks = entry.arr_secs().saturating_mul(100);
                let dep_ticks = entry.dep_secs().saturating_mul(100);
                data_tick_min = data_tick_min.min(arr_ticks.min(dep_ticks));
                data_tick_max = data_tick_max.max(arr_ticks.max(dep_ticks));
            }
            if data_tick_min > data_tick_max {
                data_tick_min = 0;
                data_tick_max = 0;
            }
            state.data_tick_min = data_tick_min;
            state.data_tick_max = data_tick_max;
        }
        let data_tick_min = state.data_tick_min;
        let data_tick_max = state.data_tick_max;

        // Entries
        {
            let entry_bytes = cast_slice(state.entries.as_slice());
            let required_size = entry_bytes.len();
            let mut should_upload = state.entries_dirty;
            if required_size as u64 > resources.entry_buffer.size() {
                let new_size = required_size.next_power_of_two().max(256) as u64;
                resources.entry_buffer = make_storage_buffer_entry(
                    "entries",
                    new_size,
                    BufferUsages::STORAGE | BufferUsages::COPY_DST,
                    device,
                );
                needs_rebind = true;
                should_upload = true;
            }
            if should_upload {
                queue.write_buffer(&resources.entry_buffer, 0, entry_bytes);
                state.entries_dirty = false;
            }
        }

        // Stations
        {
            let station_bytes = cast_slice(state.stations.as_slice());
            let required_size = station_bytes.len();
            let mut should_upload = state.stations_dirty;
            if required_size as u64 > resources.station_buffer.size() {
                let new_size = required_size.next_power_of_two().max(256) as u64;
                resources.station_buffer = make_storage_buffer_entry(
                    "stations",
                    new_size,
                    BufferUsages::STORAGE | BufferUsages::COPY_DST,
                    device,
                );
                needs_rebind = true;
                should_upload = true;
            }
            if should_upload {
                queue.write_buffer(&resources.station_buffer, 0, station_bytes);
                state.stations_dirty = false;
            }
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
        let lod_stride = state.level_of_detail_mode.as_u8() as usize;
        let repeat_from = if repeat_interval > 0 {
            (visible_ticks_min - data_tick_max).div_euclid(repeat_interval)
        } else {
            0
        };
        let repeat_count = if repeat_interval > 0 {
            let repeat_to = (visible_ticks_max - data_tick_min).div_euclid(repeat_interval);
            (repeat_to - repeat_from + 1).max(1) as usize
        } else {
            1usize
        };

        let logical_entry_count = state.entries.len() / lod_stride;
        let rendered_segment_count = logical_entry_count.saturating_mul(2);
        let total_instances = rendered_segment_count.saturating_mul(repeat_count);

        let required_segment_count = rendered_segment_count.max(1);
        let required_segment_size =
            (required_segment_count * std::mem::size_of::<GpuSegment>()) as u64;
        if required_segment_size > resources.segment_buffer.size() {
            let new_size = required_segment_size.next_power_of_two().max(256);
            resources.segment_buffer = make_storage_buffer_entry(
                "segments",
                new_size,
                BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_DST,
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
            repeat_count: repeat_count.min(u32::MAX as usize) as u32,
            lod_stride: lod_stride as u32,
            source_instance_count: state.entries.len() as u32,
            style_count: state.styles.len().min(STYLE_TABLE_CAPACITY) as u32,
            feathering_radius: match state.antialiasing_mode {
                AntialiasingMode::On => 1.2 / screen_descriptor.pixels_per_point,
                AntialiasingMode::Off => 0.0,
            },
            ..uniforms
        };
        let mut uniforms = uniforms;
        for (idx, style) in state.styles.iter().take(STYLE_TABLE_CAPACITY).enumerate() {
            uniforms.styles[idx][0] = *style;
        }
        let uniform_bytes = bytes_of(&uniforms);
        queue.write_buffer(&resources.uniform_buffer, 0, uniform_bytes);

        let indirect_init = DrawIndirectArgs {
            vertex_count: match state.antialiasing_mode {
                AntialiasingMode::On => 18,
                AntialiasingMode::Off => 6,
            },
            instance_count: total_instances.min(u32::MAX as usize) as u32,
            first_vertex: 0,
            first_instance: 0,
        };
        queue.write_buffer(&resources.indirect_buffer, 0, bytes_of(&indirect_init));

        if logical_entry_count > 0 {
            let dispatch_x = (logical_entry_count.min(u32::MAX as usize) as u32)
                .saturating_add(COMPUTE_WORKGROUP_SIZE - 1)
                / COMPUTE_WORKGROUP_SIZE;
            let mut compute_pass = egui_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_trip_prepare_segments"),
                timestamp_writes: None,
            });
            compute_pass.set_bind_group(0, &resources.compute_bind_group, &[]);
            compute_pass.set_pipeline(&resources.compute_pipeline);
            compute_pass.dispatch_workgroups(dispatch_x, 1, 1);
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
        render_pass.draw_indirect(&resources.indirect_buffer, 0);
    }
}
