use crate::plugin::loader::wasm::wasm_host::{state::PluginHostState, wit::v0_1::papokin};

impl papokin::plugin::permission::Host for PluginHostState {}
