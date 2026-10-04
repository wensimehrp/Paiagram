// SPDX-License-Identifier: MPL-2.0

use eframe::egui_wgpu::{self, wgpu};

use super::graph_intervals::{ShaderEntry as IntervalShaderEntry, graph_intervals};
use super::graph_nodes::{ShaderEntry as NodeShaderEntry, graph_nodes};
use super::trip_icons::{ShaderEntry as TripIconShaderEntry, trip_icons};

/// Per-frame data for the graph view, uploaded to the GPU in [`GraphCallback::prepare`].
pub(super) struct GraphCallback {
    pub uniforms: graph_intervals::Uniforms,
    pub interval_points: Vec<graph_intervals::IntervalPoint>,
    pub node_points: Vec<graph_nodes::NodePoint>,
    pub trip_icons: Vec<trip_icons::TripIcon>,
}

impl GraphCallback {
    pub fn new() -> Self {
        Self {
            uniforms: bytemuck::Zeroable::zeroed(),
            interval_points: Vec::new(),
            node_points: Vec::new(),
            trip_icons: Vec::new(),
        }
    }

    pub fn populate_uniforms(&mut self, rect: egui::Rect) {
        self.uniforms = graph_intervals::Uniforms::new(
            [rect.width(), rect.height()],
            1.0, // always overwritten in `prepare`
        );
    }

    pub fn paint_callback(self, rect: egui::Rect) -> egui::PaintCallback {
        egui_wgpu::Callback::new_paint_callback(rect, self)
    }
}

/// GPU resources shared by every graph callback.
struct GlobalGraphRendererResources {
    interval_pipeline: wgpu::RenderPipeline,
    node_pipeline: wgpu::RenderPipeline,
    trip_icon_pipeline: wgpu::RenderPipeline,
    buffers: GraphBuffers,
}

// SAFETY: we only render in the main thread
unsafe impl Send for GlobalGraphRendererResources {}
// SAFETY: see above.
unsafe impl Sync for GlobalGraphRendererResources {}

/// The buffers backing the passes, together with their bind groups and instance counts.
struct GraphBuffers {
    uniforms: wgpu::Buffer,
    interval_points: wgpu::Buffer,
    node_points: wgpu::Buffer,
    trip_icons: wgpu::Buffer,
    interval_bind_group: graph_intervals::WgpuBindGroup0,
    node_bind_group: graph_nodes::WgpuBindGroup0,
    trip_icon_bind_group: trip_icons::WgpuBindGroup0,
    trip_icon_view: wgpu::TextureView,
    trip_icon_sampler: wgpu::Sampler,
    interval_count: u32,
    node_count: u32,
    trip_icon_count: u32,
}

const UNIFORM_SIZE: u64 = std::mem::size_of::<graph_intervals::Uniforms>() as u64;
const INTERVAL_POINT_SIZE: u64 = std::mem::size_of::<graph_intervals::IntervalPoint>() as u64;
const NODE_POINT_SIZE: u64 = std::mem::size_of::<graph_nodes::NodePoint>() as u64;
const TRIP_ICON_SIZE: u64 = std::mem::size_of::<trip_icons::TripIcon>() as u64;

/// Creates the graph pipelines and inserts them into the renderer's callback resources.
pub(crate) fn init(render_state: &egui_wgpu::RenderState, msaa_samples: u32) {
    let resources = GlobalGraphRendererResources::new(render_state, msaa_samples);
    render_state.renderer.write().callback_resources.insert(resources);
}

impl GlobalGraphRendererResources {
    fn new(render_state: &egui_wgpu::RenderState, msaa_samples: u32) -> Self {
        let device = &render_state.device;
        let format = render_state.target_format;

        let interval_pipeline = {
            let shader =
                IntervalShaderEntry::GraphIntervals.create_shader_module_embed_source(device);
            let layout = IntervalShaderEntry::GraphIntervals.create_pipeline_layout(device);
            let vertex = graph_intervals::vs_main_entry();
            let fragment = graph_intervals::fs_main_entry([Some(color_target(format))]);
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("graph_intervals"),
                layout: Some(&layout),
                vertex: graph_intervals::vertex_state(&shader, &vertex),
                fragment: Some(graph_intervals::fragment_state(&shader, &fragment)),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: msaa(msaa_samples),
                multiview_mask: None,
                cache: None,
            })
        };

        let node_pipeline = {
            let shader = NodeShaderEntry::GraphNodes.create_shader_module_embed_source(device);
            let layout = NodeShaderEntry::GraphNodes.create_pipeline_layout(device);
            let vertex = graph_nodes::vs_main_entry();
            let fragment = graph_nodes::fs_main_entry([Some(color_target(format))]);
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("graph_nodes"),
                layout: Some(&layout),
                vertex: graph_nodes::vertex_state(&shader, &vertex),
                fragment: Some(graph_nodes::fragment_state(&shader, &fragment)),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: msaa(msaa_samples),
                multiview_mask: None,
                cache: None,
            })
        };

        let trip_icon_pipeline = {
            let shader = TripIconShaderEntry::TripIcons.create_shader_module_embed_source(device);
            let layout = TripIconShaderEntry::TripIcons.create_pipeline_layout(device);
            let vertex = trip_icons::vs_main_entry();
            let fragment = trip_icons::fs_main_entry([Some(color_target(format))]);
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("graph_trip_icons"),
                layout: Some(&layout),
                vertex: trip_icons::vertex_state(&shader, &vertex),
                fragment: Some(trip_icons::fragment_state(&shader, &fragment)),
                primitive: wgpu::PrimitiveState {
                    // The icon quad is a triangle strip, not a triangle list.
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: msaa(msaa_samples),
                multiview_mask: None,
                cache: None,
            })
        };

        Self {
            interval_pipeline,
            node_pipeline,
            trip_icon_pipeline,
            buffers: GraphBuffers::new(device, &render_state.queue),
        }
    }
}

impl GraphBuffers {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("graph uniforms"),
            size: UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let interval_points = storage_buffer(device, INTERVAL_POINT_SIZE, "graph interval points");
        let node_points = storage_buffer(device, NODE_POINT_SIZE, "graph node points");
        let trip_icons = storage_buffer(device, TRIP_ICON_SIZE, "graph trip icons");
        let (trip_icon_view, trip_icon_sampler) = load_trip_icon_texture(device, queue);
        let interval_bind_group = make_interval_bind_group(device, &uniforms, &interval_points);
        let node_bind_group = make_node_bind_group(device, &uniforms, &node_points);
        let trip_icon_bind_group = make_trip_icon_bind_group(
            device,
            &uniforms,
            &trip_icons,
            &trip_icon_view,
            &trip_icon_sampler,
        );

        Self {
            uniforms,
            interval_points,
            node_points,
            trip_icons,
            interval_bind_group,
            node_bind_group,
            trip_icon_bind_group,
            trip_icon_view,
            trip_icon_sampler,
            interval_count: 0,
            node_count: 0,
            trip_icon_count: 0,
        }
    }
}

fn color_target(format: wgpu::TextureFormat) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    }
}

fn msaa(msaa_samples: u32) -> wgpu::MultisampleState {
    wgpu::MultisampleState {
        count: msaa_samples.max(1),
        ..Default::default()
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

fn make_interval_bind_group(
    device: &wgpu::Device,
    uniforms: &wgpu::Buffer,
    interval_points: &wgpu::Buffer,
) -> graph_intervals::WgpuBindGroup0 {
    let entries =
        graph_intervals::WgpuBindGroup0Entries::new(graph_intervals::WgpuBindGroup0EntriesParams {
            uniforms: uniforms.as_entire_buffer_binding(),
            interval_points: interval_points.as_entire_buffer_binding(),
        });
    graph_intervals::WgpuBindGroup0::from_bindings(device, entries)
}

fn make_node_bind_group(
    device: &wgpu::Device,
    uniforms: &wgpu::Buffer,
    node_points: &wgpu::Buffer,
) -> graph_nodes::WgpuBindGroup0 {
    let entries =
        graph_nodes::WgpuBindGroup0Entries::new(graph_nodes::WgpuBindGroup0EntriesParams {
            uniforms: uniforms.as_entire_buffer_binding(),
            node_points: node_points.as_entire_buffer_binding(),
        });
    graph_nodes::WgpuBindGroup0::from_bindings(device, entries)
}

fn make_trip_icon_bind_group(
    device: &wgpu::Device,
    uniforms: &wgpu::Buffer,
    trip_icons: &wgpu::Buffer,
    texture: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> trip_icons::WgpuBindGroup0 {
    let entries = trip_icons::WgpuBindGroup0Entries::new(trip_icons::WgpuBindGroup0EntriesParams {
        uniforms: uniforms.as_entire_buffer_binding(),
        trip_icons: trip_icons.as_entire_buffer_binding(),
        texture_sampler: sampler,
        trip_icon_texture: texture,
    });
    trip_icons::WgpuBindGroup0::from_bindings(device, entries)
}

/// Decodes the embedded trip icon PNG and uploads it as an sRGB texture.
fn load_trip_icon_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let decoded = image::load_from_memory(include_bytes!("../../../assets/raicho_485.png"))
        .expect("the embedded trip icon must be a valid PNG")
        .into_rgba8();

    // The icon is drawn from a single square quad, so pad the decoded image to a square with
    // transparent pixels instead of stretching it. This preserves the original aspect ratio.
    let side = decoded.width().max(decoded.height());
    let mut rgba = image::RgbaImage::new(side, side);
    let x = i64::from((side - decoded.width()) / 2);
    let y = i64::from((side - decoded.height()) / 2);
    image::imageops::overlay(&mut rgba, &decoded, x, y);

    let (width, height) = rgba.dimensions();
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("trip icon"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        rgba.as_raw(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("trip icon sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });
    (view, sampler)
}

impl egui_wgpu::CallbackTrait for GraphCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let global: &mut GlobalGraphRendererResources = callback_resources.get_mut().unwrap();
        let buffers = &mut global.buffers;

        // Grow the storage buffers when the data no longer fits, and rebuild the affected bind
        // group so it points at the new buffer.
        let needed_intervals = self.interval_points.len().max(1) as u64 * INTERVAL_POINT_SIZE;
        if buffers.interval_points.size() < needed_intervals {
            buffers.interval_points =
                storage_buffer(device, needed_intervals, "graph interval points");
            buffers.interval_bind_group =
                make_interval_bind_group(device, &buffers.uniforms, &buffers.interval_points);
        }
        let needed_nodes = self.node_points.len().max(1) as u64 * NODE_POINT_SIZE;
        if buffers.node_points.size() < needed_nodes {
            buffers.node_points = storage_buffer(device, needed_nodes, "graph node points");
            buffers.node_bind_group =
                make_node_bind_group(device, &buffers.uniforms, &buffers.node_points);
        }
        let needed_icons = self.trip_icons.len().max(1) as u64 * TRIP_ICON_SIZE;
        if buffers.trip_icons.size() < needed_icons {
            buffers.trip_icons = storage_buffer(device, needed_icons, "graph trip icons");
            buffers.trip_icon_bind_group = make_trip_icon_bind_group(
                device,
                &buffers.uniforms,
                &buffers.trip_icons,
                &buffers.trip_icon_view,
                &buffers.trip_icon_sampler,
            );
        }

        if !self.interval_points.is_empty() {
            queue.write_buffer(
                &buffers.interval_points,
                0,
                bytemuck::cast_slice(&self.interval_points),
            );
        }
        if !self.node_points.is_empty() {
            queue.write_buffer(
                &buffers.node_points,
                0,
                bytemuck::cast_slice(&self.node_points),
            );
        }
        if !self.trip_icons.is_empty() {
            queue.write_buffer(
                &buffers.trip_icons,
                0,
                bytemuck::cast_slice(&self.trip_icons),
            );
        }

        // The shaders only need pixels-per-point for anti-aliasing, which egui hands us here.
        let mut uniforms = self.uniforms;
        uniforms.pixels_per_point = screen_descriptor.pixels_per_point;
        queue.write_buffer(&buffers.uniforms, 0, bytemuck::bytes_of(&uniforms));

        buffers.interval_count = self.interval_points.len() as u32;
        buffers.node_count = self.node_points.len() as u32;
        buffers.trip_icon_count = self.trip_icons.len() as u32;

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let global: &GlobalGraphRendererResources = callback_resources.get().unwrap();
        let buffers = &global.buffers;

        if buffers.interval_count > 0 {
            render_pass.set_pipeline(&global.interval_pipeline);
            render_pass.set_bind_group(0, buffers.interval_bind_group.inner(), &[]);
            render_pass.draw(
                0..graph_intervals::SQUARE_MESH_LENGTH,
                0..buffers.interval_count,
            );
        }
        if buffers.node_count > 0 {
            render_pass.set_pipeline(&global.node_pipeline);
            render_pass.set_bind_group(0, buffers.node_bind_group.inner(), &[]);
            render_pass.draw(0..graph_nodes::SQUARE_MESH_LENGTH, 0..buffers.node_count);
        }
        if buffers.trip_icon_count > 0 {
            render_pass.set_pipeline(&global.trip_icon_pipeline);
            render_pass.set_bind_group(0, buffers.trip_icon_bind_group.inner(), &[]);
            render_pass.draw(
                0..trip_icons::SQUARE_MESH_LENGTH,
                0..buffers.trip_icon_count,
            );
        }
    }
}
