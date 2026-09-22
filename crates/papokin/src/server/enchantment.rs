use crate::server::Server;
use papokin_data::Enchantment;
use papokin_data::data_component_impl::{
    CustomEnchantmentDefinition, custom_enchantment_by_name, custom_enchantment_names,
    intern_custom_enchantment,
};
use papokin_data::enchantment::{AttributeModifierSlot, Cost};
use papokin_data::item::Item;
use papokin_data::tag::{RegistryKey, Tag, get_tag_values};
use papokin_nbt::Nbt;
use papokin_nbt::compound::NbtCompound;
use papokin_nbt::tag::NbtTag;
use papokin_util::text::TextComponent;
use papokin_util::version::JavaMinecraftVersion;
use rustc_hash::FxHashMap;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct CustomEnchantmentEntry {
    pub id: String,
    pub description: TextComponent,
    pub max_level: u32,
    pub anvil_cost: u32,
    pub supported_items: String,
    pub weight: u32,
    pub slots: Vec<AttributeModifierSlot>,
    pub exclusive_set: Vec<String>,
}

/// 已注册自定义附魔的一致快照：按名称查找
/// 加上注册顺序，从而固定网络 id（原版数量 +
/// 注册索引），分别由驻留表、注册表同步以及
/// 以及协议侧的 id 桥接。
struct CustomEnchantmentState {
    by_name: FxHashMap<String, CustomEnchantmentEntry>,
    order: Vec<String>,
}

pub struct EnchantmentManager {
    custom_enchantments: RwLock<CustomEnchantmentState>,
}

/// 自定义附魔的占位成本曲线；仅对
/// 附魔台，它从不提供自定义条目。
const MIN_COST: Cost = Cost {
    base: 1,
    per_level_above_first: 10,
};
const MAX_COST: Cost = Cost {
    base: 41,
    per_level_above_first: 10,
};

impl Default for EnchantmentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EnchantmentManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            custom_enchantments: RwLock::new(CustomEnchantmentState {
                by_name: FxHashMap::default(),
                order: Vec::new(),
            }),
        }
    }

    /// 端到端注册自定义附魔：将其同步给客户端，通过
    /// `server.registry_manager`（追加在原版条目之后
    /// `enchantment` 注册表），并将其驻留为 `&'static Enchantment`，从而使物品
    /// 组件都能携带它，并刷新协议侧的 id 桥接。
    ///
    /// 管理器的写锁在全部三次插入期间都被持有，因此
    /// 并发注册不会交错并打乱 id 空间。
    ///
    /// # Errors
    /// 名称重复、与原版名称冲突、注册表已冻结（插件
    /// 加载已完成）、id 空间已耗尽或无法解析的
    /// `supported_items`/`exclusive_set` 引用会以 `Err` 形式报告。
    pub async fn register(
        &self,
        server: &Server,
        enchantment: CustomEnchantmentEntry,
    ) -> Result<(), String> {
        let mut state = self.custom_enchantments.write().await;
        if state.by_name.contains_key(&enchantment.id) {
            return Err(format!(
                "Enchantment '{}' is already registered",
                enchantment.id
            ));
        }
        if Enchantment::from_name(&enchantment.id).is_some() {
            return Err(format!(
                "Enchantment '{}' collides with a vanilla enchantment",
                enchantment.id
            ));
        }
        if server.registry_manager.is_frozen() {
            return Err(format!(
                "Cannot register enchantment '{}': the registry is frozen",
                enchantment.id
            ));
        }
        if Enchantment::ALL.len() + state.order.len() >= 256 {
            return Err("Custom enchantment id space (u8) is exhausted".to_string());
        }

        // 可能失败的解析在任何修改之前进行，因此此处失败
        // 使所有三个 id 空间保持不变。
        let supported_items = resolve_supported_items(&enchantment.supported_items)?;
        let exclusive_set = resolve_exclusive_set(&enchantment.exclusive_set)?;
        let nbt = custom_enchantment_nbt(&enchantment);

        server
            .registry_manager
            .register("enchantment", enchantment.id.clone(), nbt)?;

        let (namespace, path) = enchantment
            .id
            .split_once(':')
            .unwrap_or(("minecraft", enchantment.id.as_str()));
        intern_custom_enchantment(CustomEnchantmentDefinition {
            name: enchantment.id.clone(),
            description: format!("enchantment.{namespace}.{path}"),
            max_level: i32::try_from(enchantment.max_level).unwrap_or(i32::MAX),
            anvil_cost: enchantment.anvil_cost,
            supported_items,
            weight: i32::try_from(enchantment.weight).unwrap_or(i32::MAX),
            slots: enchantment.slots.clone(),
            exclusive_set,
            min_cost: MIN_COST,
            max_cost: MAX_COST,
        })?;

        state.order.push(enchantment.id.clone());
        papokin_protocol::codec::data_component::set_custom_ids(
            "enchantment",
            custom_enchantment_names(),
        );
        state.by_name.insert(enchantment.id.clone(), enchantment);
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Option<CustomEnchantmentEntry> {
        let state = self.custom_enchantments.read().await;
        state.by_name.get(id).cloned()
    }

    pub async fn has(&self, id: &str) -> bool {
        let state = self.custom_enchantments.read().await;
        state.by_name.contains_key(id)
    }

    /// 所有已注册的自定义附魔 ID，按注册（网络 ID）
    /// 顺序。
    pub async fn get_all_ids(&self) -> Vec<String> {
        let state = self.custom_enchantments.read().await;
        state.order.clone()
    }
}

/// 属性修饰符槽位的原版名称，按 `slots` 列表中的写法
/// 附魔注册表条目的。
const fn slot_name(slot: &AttributeModifierSlot) -> &'static str {
    match slot {
        AttributeModifierSlot::Any => "any",
        AttributeModifierSlot::MainHand => "mainhand",
        AttributeModifierSlot::OffHand => "offhand",
        AttributeModifierSlot::Hand => "hand",
        AttributeModifierSlot::Feet => "feet",
        AttributeModifierSlot::Legs => "legs",
        AttributeModifierSlot::Chest => "chest",
        AttributeModifierSlot::Head => "head",
        AttributeModifierSlot::Armor => "armor",
        AttributeModifierSlot::Body => "body",
        AttributeModifierSlot::Saddle => "saddle",
    }
}

/// 将 WIT 的 `supported_items` 字符串解析为静态物品 [`Tag`]：
/// `"#minecraft:enchantable/sword"` 会解析标签的成员，而普通物品
/// id（例如 `"minecraft:sword"`）会成为单项标签。
fn resolve_supported_items(spec: &str) -> Result<&'static Tag, String> {
    if let Some(tag_name) = spec.strip_prefix('#') {
        let names = get_tag_values(RegistryKey::Item, tag_name)
            .ok_or_else(|| format!("Unknown item tag '{spec}'"))?;
        let mut ids = Vec::with_capacity(names.len());
        for name in names {
            let item = Item::from_registry_key(name)
                .ok_or_else(|| format!("Unknown item '{name}' in tag '{spec}'"))?;
            ids.push(item.id);
        }
        let ids: &'static [u16] = Box::leak(ids.into_boxed_slice());
        Ok(Box::leak(Box::new((names, ids))))
    } else {
        let item = Item::from_registry_key(spec)
            .ok_or_else(|| format!("Unknown supported_items item '{spec}'"))?;
        let names: &'static [&'static str] =
            Box::leak(vec![&*Box::leak(spec.to_string().into_boxed_str())].into_boxed_slice());
        let ids: &'static [u16] = Box::leak(vec![item.id].into_boxed_slice());
        Ok(Box::leak(Box::new((names, ids))))
    }
}

/// 解析 `exclusive_set` 附魔名称（原版的或已注册的
/// 自定义）转换为静态附魔 id 标签。空列表变为 `None`。
fn resolve_exclusive_set(names: &[String]) -> Result<Option<&'static Tag>, String> {
    if names.is_empty() {
        return Ok(None);
    }
    let mut tag_names = Vec::with_capacity(names.len());
    let mut ids = Vec::with_capacity(names.len());
    for name in names {
        let enchantment = Enchantment::from_name(name)
            .or_else(|| custom_enchantment_by_name(name))
            .ok_or_else(|| format!("Unknown enchantment '{name}' in exclusive_set"))?;
        ids.push(u16::from(enchantment.id));
        tag_names.push(&*Box::leak(name.clone().into_boxed_str()));
    }
    let tag_names: &'static [&'static str] = Box::leak(tag_names.into_boxed_slice());
    let ids: &'static [u16] = Box::leak(ids.into_boxed_slice());
    Ok(Some(Box::leak(Box::new((tag_names, ids)))))
}

/// 为自定义附魔构建同步到客户端的网络 NBT 负载，
/// 镜像 `enchantment` 中原版条目的字段布局，
/// 注册表：`description`（文本组件）、`supported_items`、`weight`、
/// `max_level`、`min_cost`/`max_cost`（`{base, per_level_above_first}`）、
/// `anvil_cost`、`slots`（字符串列表）、可选的 `exclusive_set`，以及一个空的
/// `effects` 复合标签（自定义效果尚未建模；客户端编解码器
/// 将所有效果列表默认为空）。
fn custom_enchantment_nbt(entry: &CustomEnchantmentEntry) -> Vec<u8> {
    let mut nbt = NbtCompound::new();
    nbt.put(
        "description",
        entry
            .description
            .to_nbt_tag_for_version(&JavaMinecraftVersion::V_26_3),
    );
    nbt.put_string("supported_items", entry.supported_items.clone());
    let mut min_cost = NbtCompound::new();
    min_cost.put_int("base", MIN_COST.base);
    min_cost.put_int("per_level_above_first", MIN_COST.per_level_above_first);
    nbt.put("min_cost", NbtTag::Compound(min_cost));
    let mut max_cost = NbtCompound::new();
    max_cost.put_int("base", MAX_COST.base);
    max_cost.put_int("per_level_above_first", MAX_COST.per_level_above_first);
    nbt.put("max_cost", NbtTag::Compound(max_cost));
    nbt.put_int("weight", i32::try_from(entry.weight).unwrap_or(i32::MAX));
    nbt.put_int(
        "max_level",
        i32::try_from(entry.max_level).unwrap_or(i32::MAX),
    );
    nbt.put_int(
        "anvil_cost",
        i32::try_from(entry.anvil_cost).unwrap_or(i32::MAX),
    );
    let slots = entry
        .slots
        .iter()
        .map(|slot| NbtTag::String(slot_name(slot).into()))
        .collect();
    nbt.put_list("slots", slots);
    if !entry.exclusive_set.is_empty() {
        let exclusive = entry
            .exclusive_set
            .iter()
            .map(|name| NbtTag::String(name.clone().into()))
            .collect();
        nbt.put_list("exclusive_set", exclusive);
    }
    nbt.put("effects", NbtTag::Compound(NbtCompound::new()));
    Nbt::from(nbt).write().to_vec()
}
