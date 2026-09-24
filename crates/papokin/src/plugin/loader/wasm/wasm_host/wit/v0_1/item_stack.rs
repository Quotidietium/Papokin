use crate::plugin::loader::wasm::wasm_host::state::{ItemStackResource, PluginHostState};
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::attributes::{
    Attribute as WitAttribute, AttributeModifier as WitAttributeModifier,
    ModifierOperation as WitModifierOperation,
};
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::data_components::DataComponent as WitDataComponent;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::enchantments::Enchantment as WitEnchantment;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::item_stack::{
    CustomEnchantmentValue as WitCustomEnchantmentValue,
    DataComponentValue as WitDataComponentValue, EnchantmentValue as WitEnchantmentValue,
    Host as ItemStackInterfaceHost, HostItemStack,
    ItemAttributeModifier as WitItemAttributeModifier, ItemStack as ItemStackHandle,
};
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::text::TextComponent as WitTextComponent;
use std::sync::Arc;
use tokio::sync::Mutex;
use wasmtime::component::Resource;

use super::common::{WitNbtTree, from_wit_nbt_tree, to_wit_nbt_tree};
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::player::text_component_from_resource;
use papokin_data::Enchantment;
use papokin_data::attributes::Attributes;
use papokin_data::data_component::DataComponent;
use papokin_data::data_component_impl::combat::{Modifier, Operation};
use papokin_data::data_component_impl::{
    AttributeModifiersImpl, CustomNameImpl, EnchantmentsImpl, LoreImpl,
};
use papokin_nbt::tag::NbtTag;
use papokin_protocol::codec::data_component::{deserialize, serialize};
use std::borrow::Cow;

/// 用于插件设置的自定义附魔的自定义数据命名空间，持久化在物品
/// 磁盘上的 NBT。`pumpkin` 前缀早于 Papokin 改名；重命名它
/// 会使现有世界中的数据成为孤儿。
const CUSTOM_ENCHANTMENTS_NAMESPACE: &str = "pumpkin:enchantments";

pub(crate) fn to_wit_data_component(id: DataComponent) -> WitDataComponent {
    // SAFETY: WIT 枚举按与内部枚举相同的顺序生成
    unsafe { std::mem::transmute(id as u8) }
}

pub(crate) fn from_wit_data_component(id: WitDataComponent) -> DataComponent {
    // SAFETY: WIT 枚举按与内部枚举相同的顺序生成
    unsafe { std::mem::transmute(id as u8) }
}

pub(crate) fn to_wit_enchantment(id: &Enchantment) -> WitEnchantment {
    // SAFETY: WIT 枚举按与内部枚举相同的顺序生成
    unsafe { std::mem::transmute(id.id) }
}

pub(crate) fn from_wit_enchantment(id: WitEnchantment) -> wasmtime::Result<&'static Enchantment> {
    // WIT 枚举按与内部枚举相同的顺序生成；两表失配时优雅报错，
    // 不让 panic 跨越宿主调用边界。
    Enchantment::from_id(id as u8).ok_or_else(|| wasmtime::Error::msg("无效的附魔 ID"))
}

#[must_use]
pub fn to_wit_attribute(attr: &Attributes) -> WitAttribute {
    // SAFETY: WIT 枚举按与内部 ID 相同的顺序生成
    unsafe { std::mem::transmute(attr.id) }
}

#[must_use]
pub const fn from_wit_item_operation(op: WitModifierOperation) -> Operation {
    match op {
        WitModifierOperation::Add => Operation::AddValue,
        WitModifierOperation::MultiplyBase => Operation::AddMultipliedBase,
        WitModifierOperation::MultiplyTotal => Operation::AddMultipliedTotal,
    }
}

#[must_use]
pub const fn to_wit_item_operation(op: Operation) -> WitModifierOperation {
    match op {
        Operation::AddValue => WitModifierOperation::Add,
        Operation::AddMultipliedBase => WitModifierOperation::MultiplyBase,
        Operation::AddMultipliedTotal => WitModifierOperation::MultiplyTotal,
    }
}

impl PluginHostState {
    pub fn get_item_stack(
        &self,
        res: &Resource<ItemStackHandle>,
    ) -> wasmtime::Result<Arc<Mutex<papokin_data::item_stack::ItemStack>>> {
        self.resource_table
            .get::<ItemStackResource>(&Resource::new_own(res.rep()))
            .map(|r| r.provider.clone())
            .map_err(wasmtime::Error::from)
    }
}

impl ItemStackInterfaceHost for PluginHostState {}

impl HostItemStack for PluginHostState {
    async fn new(
        &mut self,
        registry_key: String,
        count: u8,
    ) -> wasmtime::Result<Resource<ItemStackHandle>> {
        let item = papokin_data::item::Item::from_registry_key(
            registry_key
                .strip_prefix("minecraft:")
                .unwrap_or(&registry_key),
        )
        .unwrap_or(&papokin_data::item::Item::AIR);
        let stack = papokin_data::item_stack::ItemStack::new(count, item);
        self.add_item_stack(Arc::new(Mutex::new(stack)))
    }

    async fn get_registry_key(
        &mut self,
        res: Resource<ItemStackHandle>,
    ) -> wasmtime::Result<String> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;
        // papokin-data 以裸键存储原版键名（"emerald"）；WIT 契约
        // 描述带命名空间的资源位置形式（"minecraft:emerald"）。
        let key = stack.item.registry_key;
        Ok(if key.contains(':') {
            key.to_string()
        } else {
            format!("minecraft:{key}")
        })
    }

    async fn get_count(&mut self, res: Resource<ItemStackHandle>) -> wasmtime::Result<u8> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;
        Ok(stack.item_count)
    }

    async fn set_count(
        &mut self,
        res: Resource<ItemStackHandle>,
        count: u8,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;
        stack.item_count = count;
        Ok(())
    }

    async fn get_max_count(&mut self, res: Resource<ItemStackHandle>) -> wasmtime::Result<u8> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;
        // 在组件中搜索 MaxStackSize
        if let Some((_, data)) = stack
            .item
            .components
            .iter()
            .find(|(id, _)| *id == DataComponent::MaxStackSize)
            && let Some(max_size) = data
                .as_any()
                .downcast_ref::<papokin_data::data_component_impl::MaxStackSizeImpl>()
        {
            return Ok(max_size.size);
        }
        Ok(64) // 默认
    }

    async fn get_enchantments(
        &mut self,
        res: Resource<ItemStackHandle>,
    ) -> wasmtime::Result<Vec<WitEnchantmentValue>> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;
        let mut enchantments = Vec::new();
        if let Some((_, Some(data))) = stack
            .patch
            .iter()
            .find(|(id, _)| *id == DataComponent::Enchantments)
            && let Some(enc_impl) = data.as_any().downcast_ref::<EnchantmentsImpl>()
        {
            for (enc, level) in enc_impl.enchantment.iter() {
                // WIT 附魔枚举只覆盖原版条目；
                // 对已驻留的自定义 id（>= 原版数量）做 transmute
                // 会是无效的判别值，因此跳过自定义项。
                if papokin_data::data_component_impl::is_custom_enchantment(enc) {
                    continue;
                }
                enchantments.push(WitEnchantmentValue {
                    enchantment: to_wit_enchantment(enc),
                    level: *level as u32,
                });
            }
        }
        Ok(enchantments)
    }

    async fn add_enchantment(
        &mut self,
        res: Resource<ItemStackHandle>,
        enchantment: WitEnchantment,
        level: u32,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;
        let enc = from_wit_enchantment(enchantment)?;

        let mut current_encs = if let Some((_, Some(data))) = stack
            .patch
            .iter()
            .find(|(id, _)| *id == DataComponent::Enchantments)
        {
            data.as_any()
                .downcast_ref::<EnchantmentsImpl>()
                .map(|e| e.enchantment.clone().into_owned())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        current_encs.retain(|(e, _)| e.id != enc.id);
        current_encs.push((enc, level as i32));

        if let Some((_, data)) = stack
            .patch
            .iter_mut()
            .find(|(id, _)| *id == DataComponent::Enchantments)
        {
            *data = Some(Box::new(EnchantmentsImpl {
                enchantment: Cow::from(current_encs),
            }));
        } else {
            stack.patch.push((
                DataComponent::Enchantments,
                Some(Box::new(EnchantmentsImpl {
                    enchantment: Cow::from(current_encs),
                })),
            ));
        }
        Ok(())
    }

    async fn remove_enchantment(
        &mut self,
        res: Resource<ItemStackHandle>,
        enchantment: WitEnchantment,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;
        let enc = from_wit_enchantment(enchantment)?;

        if let Some((_, Some(data))) = stack
            .patch
            .iter_mut()
            .find(|(id, _)| *id == DataComponent::Enchantments)
            && let Some(enc_impl) = data.as_mut_any().downcast_mut::<EnchantmentsImpl>()
        {
            let mut encs = enc_impl.enchantment.clone().into_owned();
            encs.retain(|(e, _)| e.id != enc.id);
            enc_impl.enchantment = Cow::from(encs);
        }
        Ok(())
    }

    async fn get_custom_enchantments(
        &mut self,
        res: Resource<ItemStackHandle>,
    ) -> wasmtime::Result<Vec<WitCustomEnchantmentValue>> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;
        let mut result = Vec::new();

        if let Some(compound) = stack.custom_data_compound()
            && let Some(custom_encs) = compound
                .get(CUSTOM_ENCHANTMENTS_NAMESPACE)
                .and_then(NbtTag::extract_compound)
        {
            for (k, v) in &custom_encs.child_tags {
                if let Some(lvl) = v.extract_int() {
                    result.push(WitCustomEnchantmentValue {
                        enchantment_id: k.to_string(),
                        level: (lvl.max(1)) as u32,
                    });
                }
            }
        }

        if let Some((_, Some(data))) = stack
            .patch
            .iter()
            .find(|(id, _)| *id == DataComponent::Enchantments)
            && let Some(enc_impl) = data.as_any().downcast_ref::<EnchantmentsImpl>()
        {
            for (enc, level) in enc_impl.enchantment.iter() {
                if !result.iter().any(|e| e.enchantment_id == enc.name) {
                    result.push(WitCustomEnchantmentValue {
                        enchantment_id: enc.name.to_string(),
                        level: (*level).max(1) as u32,
                    });
                }
            }
        }

        Ok(result)
    }

    async fn add_custom_enchantment(
        &mut self,
        res: Resource<ItemStackHandle>,
        enchantment_id: String,
        level: u32,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;

        stack.set_custom_data(
            CUSTOM_ENCHANTMENTS_NAMESPACE,
            &enchantment_id,
            NbtTag::Int(level as i32),
        );

        if let Some(vanilla) = super::enchantment::find_vanilla_enchantment(&enchantment_id) {
            let mut current_encs = if let Some((_, Some(data))) = stack
                .patch
                .iter()
                .find(|(id, _)| *id == DataComponent::Enchantments)
            {
                data.as_any()
                    .downcast_ref::<EnchantmentsImpl>()
                    .map(|e| e.enchantment.clone().into_owned())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            current_encs.retain(|(e, _)| e.id != vanilla.id);
            current_encs.push((vanilla, level as i32));

            if let Some((_, data)) = stack
                .patch
                .iter_mut()
                .find(|(id, _)| *id == DataComponent::Enchantments)
            {
                *data = Some(Box::new(EnchantmentsImpl {
                    enchantment: Cow::from(current_encs),
                }));
            } else {
                stack.patch.push((
                    DataComponent::Enchantments,
                    Some(Box::new(EnchantmentsImpl {
                        enchantment: Cow::from(current_encs),
                    })),
                ));
            }
        }

        Ok(())
    }

    async fn remove_custom_enchantment(
        &mut self,
        res: Resource<ItemStackHandle>,
        enchantment_id: String,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;

        stack.remove_custom_data(CUSTOM_ENCHANTMENTS_NAMESPACE, &enchantment_id);

        if let Some(vanilla) = super::enchantment::find_vanilla_enchantment(&enchantment_id)
            && let Some((_, Some(data))) = stack
                .patch
                .iter_mut()
                .find(|(id, _)| *id == DataComponent::Enchantments)
            && let Some(enc_impl) = data.as_mut_any().downcast_mut::<EnchantmentsImpl>()
        {
            let mut encs = enc_impl.enchantment.clone().into_owned();
            encs.retain(|(e, _)| e.id != vanilla.id);
            enc_impl.enchantment = Cow::from(encs);
        }

        Ok(())
    }

    async fn get_custom_enchantment_level(
        &mut self,
        res: Resource<ItemStackHandle>,
        enchantment_id: String,
    ) -> wasmtime::Result<Option<u32>> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;

        if let Some(NbtTag::Int(level)) =
            stack.get_custom_data(CUSTOM_ENCHANTMENTS_NAMESPACE, &enchantment_id)
        {
            return Ok(Some(level.max(1) as u32));
        }

        if let Some(vanilla) = super::enchantment::find_vanilla_enchantment(&enchantment_id)
            && let Some((_, Some(data))) = stack
                .patch
                .iter()
                .find(|(id, _)| *id == DataComponent::Enchantments)
            && let Some(enc_impl) = data.as_any().downcast_ref::<EnchantmentsImpl>()
            && let Some((_, level)) = enc_impl
                .enchantment
                .iter()
                .find(|(e, _)| e.id == vanilla.id)
        {
            return Ok(Some((*level).max(1) as u32));
        }

        Ok(None)
    }

    async fn has_custom_enchantment(
        &mut self,
        res: Resource<ItemStackHandle>,
        enchantment_id: String,
    ) -> wasmtime::Result<bool> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;

        if stack.has_custom_data(CUSTOM_ENCHANTMENTS_NAMESPACE, &enchantment_id) {
            return Ok(true);
        }

        if let Some(vanilla) = super::enchantment::find_vanilla_enchantment(&enchantment_id)
            && let Some((_, Some(data))) = stack
                .patch
                .iter()
                .find(|(id, _)| *id == DataComponent::Enchantments)
            && let Some(enc_impl) = data.as_any().downcast_ref::<EnchantmentsImpl>()
        {
            return Ok(enc_impl.enchantment.iter().any(|(e, _)| e.id == vanilla.id));
        }

        Ok(false)
    }

    async fn get_attribute_modifiers(
        &mut self,
        res: Resource<ItemStackHandle>,
    ) -> wasmtime::Result<Vec<WitItemAttributeModifier>> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;
        let mut modifiers = Vec::new();
        if let Some(comp) = stack.get_data_component::<AttributeModifiersImpl>() {
            for m in comp.attribute_modifiers.iter() {
                modifiers.push(WitItemAttributeModifier {
                    attribute: to_wit_attribute(m.r#type),
                    modifier: WitAttributeModifier {
                        id: m.id.to_string(),
                        amount: m.amount,
                        operation: to_wit_item_operation(m.operation),
                    },
                    slot: super::enchantment::to_wit_slot(&m.slot),
                });
            }
        }
        Ok(modifiers)
    }

    async fn add_attribute_modifier(
        &mut self,
        res: Resource<ItemStackHandle>,
        modifier: WitItemAttributeModifier,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;
        let attr = super::living_entity::from_wit_attribute(modifier.attribute);
        let slot = super::enchantment::to_data_slot(modifier.slot);
        let op = from_wit_item_operation(modifier.modifier.operation);
        let leaked_id: &'static str = Box::leak(modifier.modifier.id.into_boxed_str());

        let mut current_mods = stack
            .get_data_component::<AttributeModifiersImpl>()
            .map_or_else(Vec::new, |comp| {
                comp.attribute_modifiers.clone().into_owned()
            });

        current_mods.retain(|m| !(m.r#type == attr && m.id == leaked_id && m.slot == slot));
        current_mods.push(Modifier {
            r#type: attr,
            id: leaked_id,
            amount: modifier.modifier.amount,
            operation: op,
            slot,
        });

        stack.set_data_component(AttributeModifiersImpl {
            attribute_modifiers: Cow::Owned(current_mods),
        });

        Ok(())
    }

    async fn remove_attribute_modifiers(
        &mut self,
        res: Resource<ItemStackHandle>,
        attribute: WitAttribute,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;
        let attr = super::living_entity::from_wit_attribute(attribute);

        if let Some(comp) = stack.get_data_component::<AttributeModifiersImpl>() {
            let mut current_mods = comp.attribute_modifiers.clone().into_owned();
            current_mods.retain(|m| m.r#type != attr);
            if current_mods.is_empty() {
                stack
                    .patch
                    .retain(|(id, _)| *id != DataComponent::AttributeModifiers);
            } else {
                stack.set_data_component(AttributeModifiersImpl {
                    attribute_modifiers: Cow::Owned(current_mods),
                });
            }
        }

        Ok(())
    }

    async fn clear_attribute_modifiers(
        &mut self,
        res: Resource<ItemStackHandle>,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;
        stack
            .patch
            .retain(|(id, _)| *id != DataComponent::AttributeModifiers);
        Ok(())
    }

    async fn get_lore(
        &mut self,
        res: Resource<ItemStackHandle>,
    ) -> wasmtime::Result<Vec<Resource<WitTextComponent>>> {
        let stack = self.get_item_stack(&res)?;
        let lines = {
            let stack = stack.lock().await;
            stack
                .get_data_component::<LoreImpl>()
                .map_or_else(Vec::new, |lore| lore.lines.clone())
        };

        lines
            .into_iter()
            .map(|line| self.add_text_component(line))
            .collect()
    }

    async fn set_lore(
        &mut self,
        res: Resource<ItemStackHandle>,
        lore: Vec<Resource<WitTextComponent>>,
    ) -> wasmtime::Result<()> {
        let lore: Vec<_> = lore
            .iter()
            .map(|line| text_component_from_resource(self, line))
            .collect::<wasmtime::Result<Vec<_>>>()?;
        let stack = self.get_item_stack(&res)?;
        stack.lock().await.set_lore(lore);
        Ok(())
    }

    async fn add_lore(
        &mut self,
        res: Resource<ItemStackHandle>,
        line: Resource<WitTextComponent>,
    ) -> wasmtime::Result<()> {
        let line = text_component_from_resource(self, &line)?;
        let stack = self.get_item_stack(&res)?;
        stack.lock().await.add_lore(line);
        Ok(())
    }

    async fn get_custom_name(
        &mut self,
        res: Resource<ItemStackHandle>,
    ) -> wasmtime::Result<Option<Resource<WitTextComponent>>> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;
        if let Some((_, Some(data))) = stack
            .patch
            .iter()
            .find(|(id, _)| *id == DataComponent::CustomName)
            && let Some(name_impl) = data.as_any().downcast_ref::<CustomNameImpl>()
        {
            return Ok(Some(self.add_text_component(name_impl.name.clone())?));
        }
        Ok(None)
    }

    async fn set_custom_name(
        &mut self,
        res: Resource<ItemStackHandle>,
        name: Option<Resource<WitTextComponent>>,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;
        if let Some(name_res) = name {
            let name = text_component_from_resource(self, &name_res)?;
            if let Some((_, data)) = stack
                .patch
                .iter_mut()
                .find(|(id, _)| *id == DataComponent::CustomName)
            {
                *data = Some(Box::new(CustomNameImpl { name }));
            } else {
                stack.patch.push((
                    DataComponent::CustomName,
                    Some(Box::new(CustomNameImpl { name })),
                ));
            }
        } else {
            stack
                .patch
                .retain(|(id, _)| *id != DataComponent::CustomName);
        }
        Ok(())
    }

    async fn set_custom_data(
        &mut self,
        res: Resource<ItemStackHandle>,
        namespace: String,
        key: String,
        value: WitNbtTree,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;
        let value = from_wit_nbt_tree(&value).map_err(wasmtime::Error::msg)?;
        stack.set_custom_data(&namespace, &key, value);
        Ok(())
    }

    async fn get_custom_data(
        &mut self,
        res: Resource<ItemStackHandle>,
        namespace: String,
        key: String,
    ) -> wasmtime::Result<Option<WitNbtTree>> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;
        Ok(stack.get_custom_data(&namespace, &key).map(to_wit_nbt_tree))
    }

    async fn remove_custom_data(
        &mut self,
        res: Resource<ItemStackHandle>,
        namespace: String,
        key: String,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;
        stack.remove_custom_data(&namespace, &key);
        Ok(())
    }

    async fn has_custom_data(
        &mut self,
        res: Resource<ItemStackHandle>,
        namespace: String,
        key: String,
    ) -> wasmtime::Result<bool> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;
        Ok(stack.has_custom_data(&namespace, &key))
    }

    async fn get_components(
        &mut self,
        res: Resource<ItemStackHandle>,
    ) -> wasmtime::Result<Vec<WitDataComponentValue>> {
        let stack = self.get_item_stack(&res)?;
        let stack = stack.lock().await;
        let mut components = Vec::new();
        for (id, data) in &stack.patch {
            if let Some(data) = data {
                let mut buf = Vec::new();
                if serialize(*id, data.as_ref(), &mut buf).is_ok() {
                    components.push(WitDataComponentValue {
                        component: to_wit_data_component(*id),
                        value: buf,
                    });
                }
            }
        }
        Ok(components)
    }

    async fn set_component(
        &mut self,
        res: Resource<ItemStackHandle>,
        component: WitDataComponent,
        value: Vec<u8>,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;
        let id = from_wit_data_component(component);
        let mut cursor = std::io::Cursor::new(value);

        if let Ok(component_impl) = deserialize(id, &mut cursor) {
            if let Some((_, data)) = stack.patch.iter_mut().find(|(pid, _)| *pid == id) {
                *data = Some(component_impl);
            } else {
                stack.patch.push((id, Some(component_impl)));
            }
        }
        Ok(())
    }

    async fn remove_component(
        &mut self,
        res: Resource<ItemStackHandle>,
        component: WitDataComponent,
    ) -> wasmtime::Result<()> {
        let stack = self.get_item_stack(&res)?;
        let mut stack = stack.lock().await;
        let id = from_wit_data_component(component);
        stack.patch.retain(|(pid, _)| *pid != id);
        Ok(())
    }

    async fn drop(&mut self, rep: Resource<ItemStackHandle>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<ItemStackResource>(Resource::new_own(rep.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}
