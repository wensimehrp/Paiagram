// SPDX-License-Identifier: MPL-2.0
// TODO: handle styles

use eframe::egui_wgpu::{self, wgpu};
use paiagram_core::route::DiagramCache;
use paiagram_core::{CanvasLength, RouteKey, RouteKeyHashMap, Source};

use super::gpu_trip::{ShaderEntry, gpu_trip};
use crate::tabs::Navigatable;

/// Per-frame data for one diagram, uploaded to the GPU in [`DiagramCallback::prepare`].
#[derive(Clone)]
pub(super) struct DiagramCallback {
    pub route_key: RouteKey,
    pub uniforms: gpu_trip::Uniforms,
    pub entry_segments: Vec<gpu_trip::EntrySegment>,
    pub stations: Vec<gpu_trip::CanvasLength>,
}

impl DiagramCallback {
    pub fn new(route_key: RouteKey) -> Self {
        Self {
            route_key,
            uniforms: bytemuck::Zeroable::zeroed(),
            entry_segments: Vec::new(),
            stations: Vec::new(),
        }
    }

    pub fn populate_uniforms(&mut self, navi: &super::DiagramTabNavigation, rect: egui::Rect) {
        self.uniforms = gpu_trip::Uniforms::new(
            [rect.width(), rect.height()],
            navi.offset_x() as i32,
            navi.offset_y() as f32,
            navi.x_per_screen_unit_f64() as f32,
            navi.y_per_screen_unit().0 as f32,
            1.0, // value always overwritten
        );
    }

    pub fn populate_stations(&mut self, station_heights: impl Iterator<Item = CanvasLength>) {
        self.stations.clear();
        self.stations.extend(station_heights.map(|h| gpu_trip::CanvasLength::new(h.0 as f32)));
    }

    pub fn paint_callback(self, rect: egui::Rect) -> egui::PaintCallback {
        egui_wgpu::Callback::new_paint_callback(rect, self)
    }

    pub fn populate_entry_segments(&mut self, cache: &DiagramCache, source: &Source) {
        self.entry_segments.clear();
        self.stations.clear();
        for idx in 0..100 {
            self.stations.push(gpu_trip::CanvasLength::new(idx as f32 * 10.0));
        }
        for (trip_key, polylines) in &cache.map {
            let Some(trip) = source.trips.get(trip_key) else {
                continue;
            };
            let style = trip.service_class.and_then(|key| source.service_classes.get(&key)).map_or(
                gpu_trip::Style {
                    thickness: 1.0,
                    fill_rgba: 0x808080ff,
                },
                |class| gpu_trip::Style {
                    thickness: class.style.width as f32 * 1.0,
                    fill_rgba: u32::from_be_bytes(class.style.color.to_array()),
                },
            );
            for polyline in polylines {
                // One entry per point of the polyline.
                let base = self.entry_segments.len();
                for &(estimate, _entry, index, progress) in polyline {
                    self.entry_segments.push(gpu_trip::EntrySegment {
                        arr_seconds: estimate.arr.0,
                        dep_seconds: estimate.dep.0,
                        curr_index: index,
                        curr_progress: progress,
                        connects_to_prev: 0,
                        connects_to_next: 0,
                        style,
                    });
                }
                // Consecutive points within a polyline are connected to each other.
                for (i, _) in polyline.array_windows::<2>().enumerate() {
                    self.entry_segments[base + i].connects_to_next = 1;
                    self.entry_segments[base + i + 1].connects_to_prev = 1;
                }
            }
        }
    }
}

/// GPU resources shared by every diagram callback.
///
/// Created once at startup by [`init`] and stored in the renderer's `callback_resources`, so it
/// outlives the egui render pass (the pipeline must not be dropped while it is in use). Buffers
/// are per-tab, because several diagrams can be open at once.
struct GlobalDiagramRendererResources {
    pipeline: wgpu::RenderPipeline,
    per_tab_resources: RouteKeyHashMap<PerTabDiagramRendererResources>,
}

/// `egui-wgpu` stores callback resources in a `type_map::concurrent::TypeMap`, whose `insert`
/// requires `Send + Sync`. On `wasm32` with atomic target features enabled, `wgpu`'s handles are
/// neither (its `send_sync` cfg excludes atomics; the handles reference JS objects), so the bound
/// has to be asserted manually. That is sound here because the resources only ever live on the
/// single thread that owns the egui renderer.
///
/// Spelling these impls out instead of relying on inference also keeps the compiler from
/// recursing through `wgpu`'s deeply nested types while checking the auto traits (which otherwise
/// trips `recursion_depth_exceeding_limit`).
// SAFETY: we only render in the main thread
unsafe impl Send for GlobalDiagramRendererResources {}
// SAFETY: see above.
unsafe impl Sync for GlobalDiagramRendererResources {}

struct PerTabDiagramRendererResources {
    uniforms: wgpu::Buffer,
    entry_segments: wgpu::Buffer,
    stations: wgpu::Buffer,
    bind_group: gpu_trip::WgpuBindGroup0,
    /// Number of `EntrySegment`s to draw (one instance per segment).
    instance_count: u32,
}

const UNIFORM_SIZE: u64 = std::mem::size_of::<gpu_trip::Uniforms>() as u64;
const SEGMENT_SIZE: u64 = std::mem::size_of::<gpu_trip::EntrySegment>() as u64;
const STATION_SIZE: u64 = std::mem::size_of::<gpu_trip::CanvasLength>() as u64;

/// Creates the diagram pipeline and inserts it into the renderer's callback resources.
///
/// Must be called once at startup, when the WGPU backend is available. `msaa_samples` has to
/// match the render pass egui uses (i.e. eframe's `NativeOptions::multisampling`).
pub(crate) fn init(render_state: &egui_wgpu::RenderState, msaa_samples: u32) {
    let resources = GlobalDiagramRendererResources::new(render_state, msaa_samples);
    render_state.renderer.write().callback_resources.insert(resources);
}

impl GlobalDiagramRendererResources {
    fn new(render_state: &egui_wgpu::RenderState, msaa_samples: u32) -> Self {
        let device = &render_state.device;

        // Everything here comes from wgsl_bindgen: the WGSL entry points, the pipeline layout
        // derived from `@group`/`@binding` in the compiled `gpu_trip.wesl`, and the reflected
        // vertex/fragment state.
        let shader = ShaderEntry::GpuTrip.create_shader_module_embed_source(device);
        let pipeline_layout = ShaderEntry::GpuTrip.create_pipeline_layout(device);
        let vertex = gpu_trip::vs_main_entry();
        let fragment = gpu_trip::fs_main_entry([Some(wgpu::ColorTargetState {
            format: render_state.target_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })]);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("diagram_gpu_trip"),
            layout: Some(&pipeline_layout),
            vertex: gpu_trip::vertex_state(&shader, &vertex),
            fragment: Some(gpu_trip::fragment_state(&shader, &fragment)),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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

        Self {
            pipeline,
            per_tab_resources: RouteKeyHashMap::default(),
        }
    }
}

impl PerTabDiagramRendererResources {
    fn new(device: &wgpu::Device) -> Self {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("diagram uniforms"),
            size: UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let entry_segments = storage_buffer(device, SEGMENT_SIZE, "diagram entry segments");
        let stations = storage_buffer(device, STATION_SIZE, "diagram stations");
        let bind_group = make_bind_group(device, &uniforms, &entry_segments, &stations);

        Self {
            uniforms,
            entry_segments,
            stations,
            bind_group,
            instance_count: 0,
        }
    }
}

fn storage_buffer(device: &wgpu::Device, size: u64, label: &str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        // Storage buffers must be non-empty, so allocate at least one element.
        size: size.max(4),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn make_bind_group(
    device: &wgpu::Device,
    uniforms: &wgpu::Buffer,
    entry_segments: &wgpu::Buffer,
    stations: &wgpu::Buffer,
) -> gpu_trip::WgpuBindGroup0 {
    let entries = gpu_trip::WgpuBindGroup0Entries::new(gpu_trip::WgpuBindGroup0EntriesParams {
        uniforms: uniforms.as_entire_buffer_binding(),
        entry_segments: entry_segments.as_entire_buffer_binding(),
        stations: stations.as_entire_buffer_binding(),
    });
    gpu_trip::WgpuBindGroup0::from_bindings(device, entries)
}

impl egui_wgpu::CallbackTrait for DiagramCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let global: &mut GlobalDiagramRendererResources = callback_resources.get_mut().unwrap();
        let resources = global
            .per_tab_resources
            .entry(self.route_key)
            .or_insert_with(|| PerTabDiagramRendererResources::new(device));

        // Grow the storage buffers when the data no longer fits, and rebuild the bind group so
        // it points at the new buffers.
        let needed_segments = self.entry_segments.len().max(1) as u64 * SEGMENT_SIZE;
        let needed_stations = self.stations.len().max(1) as u64 * STATION_SIZE;
        let mut needs_rebind = false;
        if resources.entry_segments.size() < needed_segments {
            resources.entry_segments =
                storage_buffer(device, needed_segments, "diagram entry segments");
            needs_rebind = true;
        }
        if resources.stations.size() < needed_stations {
            resources.stations = storage_buffer(device, needed_stations, "diagram stations");
            needs_rebind = true;
        }
        if needs_rebind {
            resources.bind_group = make_bind_group(
                device,
                &resources.uniforms,
                &resources.entry_segments,
                &resources.stations,
            );
        }

        if !self.entry_segments.is_empty() {
            queue.write_buffer(
                &resources.entry_segments,
                0,
                bytemuck::cast_slice(&self.entry_segments),
            );
        }
        if !self.stations.is_empty() {
            queue.write_buffer(&resources.stations, 0, bytemuck::cast_slice(&self.stations));
        }
        // The feather width is specified in hardware pixels, so the shader needs the device's
        // pixels-per-point, which egui hands us in the screen descriptor.
        let mut uniforms = self.uniforms;
        uniforms.pixels_per_point = screen_descriptor.pixels_per_point;
        queue.write_buffer(&resources.uniforms, 0, bytemuck::bytes_of(&uniforms));
        resources.instance_count = self.entry_segments.len() as u32;

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let global: &GlobalDiagramRendererResources = callback_resources.get().unwrap();
        let Some(resources) = global.per_tab_resources.get(&self.route_key) else {
            return;
        };
        if resources.instance_count == 0 {
            return;
        }
        render_pass.set_pipeline(&global.pipeline);
        render_pass.set_bind_group(0, resources.bind_group.inner(), &[]);
        render_pass.draw(
            0..gpu_trip::SEGMENT_MESH_LENGTH,
            0..resources.instance_count,
        );
    }
}
