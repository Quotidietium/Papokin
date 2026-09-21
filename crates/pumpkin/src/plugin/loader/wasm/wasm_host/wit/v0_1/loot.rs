use std::sync::Arc;

use tokio::sync::Mutex;
use wasmtime::component::Resource;

use crate::plugin::loader::wasm::wasm_host::{
    state::{InventoryProvider, InventoryResource, PluginHostState},
    wit::v0_1::pumpkin::plugin::{
        inventory::Inventory as WitInventory,
        item_stack::ItemStack as WitItemStack,
        loot::{Host, LootContext as WitLootContext},
    },
};
use crate::world::loot::{LootContextParameters, fill_chest_inventory};

fn lookup_table(key: &str) -> Result<&'static pumpkin_util::loot_table::LootTable, String> {
    pumpkin_data::loot_table::get_loot_table(key)
        .ok_or_else(|| format!("unknown loot table: {key}"))
}

impl PluginHostState {
    fn stacks_to_wit(
        &mut self,
        items: Vec<pumpkin_data::item_stack::ItemStack>,
    ) -> wasmtime::Result<Vec<Resource<WitItemStack>>> {
        items
            .into_iter()
            .map(|stack| self.add_item_stack(Arc::new(Mutex::new(stack))))
            .collect()
    }
}

impl Host for PluginHostState {
    async fn has_loot_table(&mut self, key: String) -> wasmtime::Result<bool> {
        Ok(pumpkin_data::loot_table::get_loot_table(&key).is_some())
    }

    async fn generate_loot(
        &mut self,
        key: String,
        seed: i64,
    ) -> wasmtime::Result<Result<Vec<Resource<WitItemStack>>, String>> {
        let table = match lookup_table(&key) {
            Ok(table) => table,
            Err(err) => return Ok(Err(err)),
        };
        let items = crate::world::loot::generate_loot(table, seed);
        Ok(Ok(self.stacks_to_wit(items)?))
    }

    async fn generate_loot_with_context(
        &mut self,
        key: String,
        seed: i64,
        context: WitLootContext,
    ) -> wasmtime::Result<Result<Vec<Resource<WitItemStack>>, String>> {
        let table = match lookup_table(&key) {
            Ok(table) => table,
            Err(err) => return Ok(Err(err)),
        };

        let tool = match &context.tool {
            Some(res) => Some(self.get_item_stack(res)?.lock().await.clone()),
            None => None,
        };

        let params = LootContextParameters {
            explosion_radius: context.explosion_radius,
            killed_by_player: Some(context.killed_by_player),
            luck: context.luck,
            tool,
            ..Default::default()
        };

        let items = crate::world::loot::generate_loot_with_context(table, seed, &params);
        Ok(Ok(self.stacks_to_wit(items)?))
    }

    async fn fill_inventory(
        &mut self,
        key: String,
        seed: i64,
        target: Resource<WitInventory>,
    ) -> wasmtime::Result<Result<(), String>> {
        let table = match lookup_table(&key) {
            Ok(table) => table,
            Err(err) => return Ok(Err(err)),
        };

        let provider = self
            .resource_table
            .get::<InventoryResource>(&Resource::new_own(target.rep()))
            .map_err(wasmtime::Error::from)?
            .provider
            .clone();

        let inventory: Arc<dyn pumpkin_inventory::Inventory> = match provider {
            InventoryProvider::Generic(inventory) => inventory,
            // Player inventories need per-slot client sync packets that the
            // vanilla chest-fill path does not emit; refuse them rather than
            // silently desyncing the client.
            InventoryProvider::PlayerMain(_) | InventoryProvider::PlayerEnderChest(_) => {
                return Ok(Err(
                    "fill-inventory only supports generic container inventories".to_string(),
                ));
            }
        };

        fill_chest_inventory(&inventory, table, seed);
        Ok(Ok(()))
    }
}
