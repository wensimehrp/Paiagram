use petgraph::visit::{EdgeRef, IntoEdgeReferences, IntoNodeReferences, NodeIndexable, NodeRef};

pub trait HasXY {
    fn x(&self) -> f32;
    fn y(&self) -> f32;
}

impl<T> HasXY for &T
where
    T: HasXY + ?Sized,
{
    fn x(&self) -> f32 {
        (*self).x()
    }

    fn y(&self) -> f32 {
        (*self).y()
    }
}

pub fn soap_pre<G>(graph: &G, iterations: u32) -> impl Fn(G::NodeId) -> (f32, f32) + '_
where
    G: IntoNodeReferences + IntoEdgeReferences + NodeIndexable,
    G::NodeRef: NodeRef,
    <G::NodeRef as NodeRef>::Weight: HasXY,
{
    let mut xy = vec![(0.0f32, 0.0f32); graph.node_bound()];
    for node_ref in graph.node_references() {
        let idx = graph.to_index(node_ref.id());
        xy[idx] = (node_ref.weight().x(), node_ref.weight().y())
    }
    for edge in graph.edge_references() {
        let ui = graph.to_index(edge.source());
        let vi = graph.to_index(edge.target());
        if ui == vi {
            continue;
        }
        let (ux, uy) = xy[ui];
        let (vx, vy) = xy[vi];

        let angle_uv = (vy - uy).atan2(vx - ux);
        let angle_vu = (uy - vy).atan2(ux - vx);
    }
    move |_| (0.0, 0.0)
}

#[derive(Clone, Copy, Default)]
struct SoapState {
    x: f32,
    y: f32,
    fx: f32,
    fy: f32,
    vx: f32,
    vy: f32,
}

pub fn soap_solve<G>(graph: &G, iterations: u32) -> impl Fn(G::NodeId) -> (f32, f32) + '_
where
    G: IntoNodeReferences + IntoEdgeReferences + NodeIndexable,
    G::NodeRef: NodeRef,
    <G::NodeRef as NodeRef>::Weight: HasXY,
{
    let mut positions = vec![(0.5f32, 0.5f32); graph.node_bound()];

    // create the state buffer
    let mut states = vec![SoapState::default(); graph.node_bound()];
    let mut active_indices = Vec::new();

    // populate state
    for node_ref in graph.node_references() {
        let idx = graph.to_index(node_ref.id());
        active_indices.push(idx);

        states[idx].x = node_ref.weight().x();
        states[idx].y = node_ref.weight().y();
        positions[idx] = (states[idx].x, states[idx].y);
    }

    for it in 0..iterations {
        for idx in active_indices.iter().copied() {
            states[idx].fx = 0.0;
            states[idx].fx = 0.0
        }
        for face in [0.0, 0.0] {}
    }

    move |node_id| positions[NodeIndexable::to_index(graph, node_id)]
}
