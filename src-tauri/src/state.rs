use lanchat_core::Node;

pub struct AppState {
    pub node: Node,
}

impl AppState {
    pub fn new(node: Node) -> Self {
        Self { node }
    }
}
