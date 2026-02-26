pub enum WasmCommand {
    CreateStation { name: String, id: u32 },
    CreateEntry { station_id: u32 },
}

pub struct PluginState {
    /// Wasm pushes events here via host functions
    pub command_queue: Vec<WasmCommand>,
}

// TODO: add wasm plugin support. Pass in a &[u8], pass out a cbor
