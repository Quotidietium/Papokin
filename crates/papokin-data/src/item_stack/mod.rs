use crate::data_component::DataComponent;
use crate::data_component::DataComponent::Enchantments;
use crate::data_component_impl::{
    BlocksAttacksImpl, ConsumableImpl, CustomDataImpl, DamageImpl, DataComponentImpl,
    EnchantmentsImpl, IDSet, MaxDamageImpl, MaxStackSizeImpl, Rarity, RarityImpl,
    SwingAnimationImpl, ToolImpl, UnbreakableImpl, UseCooldownImpl, get, get_mut, read_data,
};

use crate::item::Item;
use crate::recipes::RecipeResultStruct;
use crate::tag::Taggable;
use crate::{Block, Enchantment};
use papokin_nbt::compound::NbtCompound;
use papokin_nbt::tag::NbtTag;
use papokin_util::GameMode;
use rand;
use std::borrow::Cow;
use std::cmp::{max, min};
use std::num::NonZero;
use std::sync::atomic::{AtomicU32, Ordering};

mod categories;

/// [`ItemStack::damage_item`] 调用的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageResult {
    /// 未施加伤害（数量为零/负数、不可受伤害、无法破坏、
    /// 或耐久附魔（Unbreaking）抵消了每一点伤害）。
    Untouched,
    /// 伤害已应用且物品仍然存在。
    Damaged,
    /// 物品损坏：物品堆中消耗了一个物品（耐久度重置为 0），
    /// 若原来只有一个物品则物品堆现在为空。调用方应始终
    /// 广播破坏状态——客户端都能正确处理这两种情况。
    Broken,
}

#[derive(Clone)]
pub struct ItemStack {
    pub item_count: u8,
    pub item: &'static Item,
    pub patch: Vec<(DataComponent, Option<Box<dyn DataComponentImpl>>)>,

    /// 生成代数 ID，从不序列化。`copy_with_count` 会重新生成它，因此
    /// 乐观并发检查（物品实体合并）能够检测到
    /// 物品堆在可取消事件进行途中被替换。
    pub uid: NonZero<i32>,
}

// impl Hash for ItemStack {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.item_count.hash(state);
//         self.item.id.hash(state);
//         self.patch.hash(state);
//     }
// }

/*
impl PartialEq for ItemStack {
    fn eq(&self, other: &Self) -> bool {
        self.item.id == other.item.id
    }
} */

pub struct ItemStackIdGenerator {
    counter: AtomicU32,
}

impl ItemStackIdGenerator {
    pub const fn new() -> Self {
        Self {
            counter: AtomicU32::new(1),
        }
    }

    pub fn next_id(&self) -> NonZero<i32> {
        // 溢出时回绕，这正是我们想要的。
        let value = self.counter.fetch_add(1, Ordering::Relaxed);

        // 负值无效；在正值间循环
        let masked = value & 0x7FFFFFFF;

        if let Some(id) = NonZero::new(masked as i32) {
            id
        } else {
            // 如果取到 0 或 0x80000000，则按 0 屏蔽
            // 零是无效的，因此我们直接请求下一个 ID，以保持简单/正确。
            self.next_id()
        }
    }
}

impl Default for ItemStackIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

static ITEM_STACK_ID_GEN: ItemStackIdGenerator = ItemStackIdGenerator::new();

impl ItemStack {
    #[must_use]
    pub fn new(item_count: u8, item: &'static Item) -> Self {
        Self {
            item_count,
            item,
            patch: Vec::new(),

            uid: ITEM_STACK_ID_GEN.next_id(),
        }
    }

    #[must_use]
    pub fn new_with_component(
        item_count: u8,
        item: &'static Item,
        component: Vec<(DataComponent, Option<Box<dyn DataComponentImpl>>)>,
    ) -> Self {
        Self {
            item_count,
            item,
            patch: component,

            uid: ITEM_STACK_ID_GEN.next_id(),
        }
    }

    /// 用于静态/模板物品堆（例如进度图标）的 const 构造函数。
    /// 此处创建的所有物品堆共享固定的生成 ID；它们绝不会
    /// 转换为世界物品栏中用于比较 ID 的表示形式。
    #[must_use]
    pub const fn static_new_java(item_count: u8, item: &'static Item) -> Self {
        Self {
            item_count,
            item,
            patch: Vec::new(),

            uid: match NonZero::new(1) {
                Some(v) => v,
                None => panic!("1 非零"),
            },
        }
    }

    #[must_use]
    pub fn get_data_component<T: DataComponentImpl + 'static>(&self) -> Option<&T> {
        let to_get_id = &T::get_enum();
        for (id, component) in &self.patch {
            if id == to_get_id {
                return component
                    .as_ref()
                    .map(|component| get::<T>(component.as_ref()));
            }
        }
        for (id, component) in self.item.components {
            if id == to_get_id {
                return Some(get::<T>(*component));
            }
        }
        None
    }
    #[must_use]
    pub fn get_data_component_mut<T: DataComponentImpl + 'static>(&mut self) -> Option<&mut T> {
        let to_get_id = T::get_enum();
        if let Some(index) = self.patch.iter().position(|(id, _)| *id == to_get_id) {
            return self.patch[index]
                .1
                .as_mut()
                .map(|component| get_mut::<T>(component.as_mut()));
        }

        // 如果不在 patch 中，则从 item 克隆到 patch 并返回 mut
        let mut cloned = None;
        for (id, component) in self.item.components {
            if *id == to_get_id {
                cloned = Some((*id, Some(component.clone_dyn())));
                break;
            }
        }
        if let Some((id, component)) = cloned {
            self.patch.push((id, component));
            return self
                .patch
                .last_mut()?
                .1
                .as_mut()
                .map(|c| get_mut::<T>(c.as_mut()));
        }
        None
    }

    #[must_use]
    pub fn has_data_component(&self, to_get_id: DataComponent) -> bool {
        for (id, component) in &self.patch {
            if *id == to_get_id {
                return component.is_some();
            }
        }
        for (id, _) in self.item.components {
            if *id == to_get_id {
                return true;
            }
        }
        false
    }

    pub fn has_enchantments(&self) -> bool {
        self.get_data_component::<EnchantmentsImpl>()
            .is_some_and(|e| !e.enchantment.is_empty())
    }

    pub fn add_enchantment(&mut self, enchantment: &'static Enchantment, level: u16) {
        if let Some(enchantments) = self.get_data_component_mut::<EnchantmentsImpl>() {
            let mut new_vec = enchantments.enchantment.to_vec();
            new_vec.push((enchantment, level as i32));
            enchantments.enchantment = Cow::Owned(new_vec);
        } else {
            let enchantments = EnchantmentsImpl {
                enchantment: Cow::Owned(vec![(enchantment, level as i32)]),
            };
            self.patch
                .push((DataComponent::Enchantments, Some(Box::new(enchantments))));
        }
    }

    pub fn set_lore(&mut self, lines: Vec<papokin_util::text::TextComponent>) {
        let lore = Some(Box::new(crate::data_component_impl::LoreImpl { lines }) as _);
        if let Some((_, component)) = self
            .patch
            .iter_mut()
            .find(|(id, _)| *id == DataComponent::Lore)
        {
            *component = lore;
        } else {
            self.patch.push((DataComponent::Lore, lore));
        }
    }

    pub fn set_data_component<T: DataComponentImpl + 'static>(&mut self, component: T) {
        let to_set_id = T::get_enum();
        let boxed = Some(Box::new(component) as _);
        if let Some((_, c)) = self.patch.iter_mut().find(|(id, _)| *id == to_set_id) {
            *c = boxed;
        } else {
            self.patch.push((to_set_id, boxed));
        }
    }

    pub fn remove_data_component(&mut self, to_remove_id: DataComponent) {
        if let Some((_, c)) = self.patch.iter_mut().find(|(id, _)| *id == to_remove_id) {
            *c = None;
        } else {
            self.patch.push((to_remove_id, None));
        }
    }

    pub fn add_lore(&mut self, line: papokin_util::text::TextComponent) {
        let mut lines = self
            .get_data_component::<crate::data_component_impl::LoreImpl>()
            .map_or_else(Vec::new, |lore| lore.lines.clone());
        lines.push(line);
        self.set_lore(lines);
    }

    pub const EMPTY: &'static Self = &Self {
        item_count: 0,
        item: &Item::AIR,
        patch: Vec::new(),

        uid: NonZero::<i32>::MIN, // 哨兵；空物品堆的 ID 永不参与比较
    };

    #[must_use]
    pub fn split_off(&mut self, amount: u8) -> Self {
        let count = amount.min(self.item_count);
        let result = self.copy_with_count(count);
        self.decrement(count);
        result
    }

    #[must_use]
    pub fn get_max_stack_size(&self) -> u8 {
        self.get_data_component::<MaxStackSizeImpl>()
            .map_or(1, |value| value.size)
    }

    #[must_use]
    pub fn get_max_damage(&self) -> Option<i32> {
        self.get_data_component::<MaxDamageImpl>()
            .map(|value| value.max_damage)
    }

    #[must_use]
    pub fn get_use_cooldown(&self) -> Option<&UseCooldownImpl> {
        self.get_data_component::<UseCooldownImpl>()
    }

    #[must_use]
    pub fn get_damage(&self) -> i32 {
        self.get_data_component::<DamageImpl>()
            .map_or(0, |value| value.damage)
    }

    #[must_use]
    pub fn get_enchantment_level(&self, enchantment: &'static Enchantment) -> i32 {
        let Some(data) = self.get_data_component::<EnchantmentsImpl>() else {
            return 0;
        };
        for (enc, level) in data.enchantment.iter() {
            if *enc == enchantment {
                return *level;
            }
        }
        0
    }

    #[must_use]
    pub fn is_unbreakable(&self) -> bool {
        self.get_data_component::<UnbreakableImpl>().is_some()
    }

    pub fn set_damage(&mut self, damage: i32) {
        let damage = damage.max(0);
        if damage == 0 {
            self.patch.retain(|(id, _)| *id != DataComponent::Damage);
            return;
        }

        for (id, component) in &mut self.patch {
            if *id == DataComponent::Damage {
                *component = Some(DamageImpl { damage }.to_dyn());
                return;
            }
        }

        self.patch
            .push((DataComponent::Damage, Some(DamageImpl { damage }.to_dyn())));
    }

    #[must_use]
    pub fn is_damageable(&self) -> bool {
        self.get_max_damage().unwrap_or(0) > 0
    }

    pub fn repair_item(&mut self, amount: i32) -> i32 {
        if amount <= 0 {
            return 0;
        }
        let damage = self.get_damage();
        if damage <= 0 {
            return 0;
        }
        let repaired = amount.min(damage);
        self.set_damage(damage - repaired);
        repaired
    }

    /// 核心逻辑：使用预计算的护甲类别和等级应用耐久（Unbreaking）概率。
    /// 提取出来供 damage_item 使用，这些值在该处被提升到循环之外。
    /// 设为私有以防止误用；只能通过 damage_item 调用。
    fn should_apply_durability_damage_with(is_armor: bool, unbreaking_level: i32) -> bool {
        if unbreaking_level <= 0 {
            return true;
        }

        // `#minecraft:enchantable/armor` 使用盔甲公式；其余全部使用工具公式。
        if is_armor {
            let chance = 0.6 + (0.4 / (unbreaking_level as f32 + 1.0));
            rand::random::<f32>() < chance
        } else {
            rand::random::<u32>().is_multiple_of(unbreaking_level as u32 + 1)
        }
    }

    /// 对此物品施加耐久损耗并返回结果。
    /// 调用者必须检查返回值，以处理破坏广播和物品堆更新。
    /// TODO: 待所有调用点（尤其是工具/生物的方块命中/伤害处）处理完成后恢复 `#[must_use]`
    /// 实现正确的 `DamageResult::Broken` 处理，而不是用 `let _ =` 压制。
    /// 若缺少此项强制约束，相对于原版的破坏行为，该修复并不完整。
    #[must_use]
    pub fn damage_item(&mut self, amount: i32) -> DamageResult {
        if amount <= 0 || !self.is_damageable() || self.is_unbreakable() {
            return DamageResult::Untouched;
        }

        let max_damage = self.get_max_damage().unwrap_or(0);
        if max_damage <= 0 {
            return DamageResult::Untouched;
        }

        // 将护甲检查和附魔等级检查提升到循环外，以避免重复查找。
        let is_armor = self.is_armor();
        let unbreaking_level = self.get_enchantment_level(&Enchantment::UNBREAKING);
        let mut applied = 0;
        // TODO: 当已应用量 >= (max_damage - current_damage) 时短路返回，以避免
        // 对高耐久物品的高伤害打击进行全额迭代。
        for _ in 0..amount {
            if Self::should_apply_durability_damage_with(is_armor, unbreaking_level) {
                applied += 1;
            }
        }

        if applied <= 0 {
            return DamageResult::Untouched;
        }

        let new_damage = self.get_damage().saturating_add(applied);
        if new_damage >= max_damage {
            // 原版行为：破坏时消耗物品堆中的一个物品并重置
            // 耐久度归零。单次 damage 调用绝不会破坏多于一件物品，
            // 而不论伤害量多少。这与原版物品堆的行为一致。
            if self.item_count > 1 {
                self.item_count = self.item_count.saturating_sub(1);
                self.set_damage(0);
            } else {
                *self = Self::EMPTY.clone();
            }
            return DamageResult::Broken;
        }

        self.set_damage(new_damage);
        DamageResult::Damaged
    }

    #[must_use]
    pub fn get_max_use_time(&self) -> i32 {
        if let Some(value) = self.get_data_component::<ConsumableImpl>() {
            return value.consume_ticks();
        }
        if self.get_data_component::<BlocksAttacksImpl>().is_some() {
            return 72000;
        }
        0
    }

    #[must_use]
    pub const fn get_item(&self) -> &'static Item {
        if self.is_empty() {
            &Item::AIR
        } else {
            self.item
        }
    }

    #[must_use]
    pub fn is_stackable(&self) -> bool {
        self.get_max_stack_size() > 1 // TODO: && (!this.isDamageable() || !this.isDamaged());
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.item_count == 0 || self.item.id == Item::AIR.id
    }

    pub fn set_custom_name(&mut self, name: String) {
        use crate::data_component_impl::CustomNameImpl;
        let component = Some(
            CustomNameImpl {
                name: papokin_util::text::TextComponent::text(name),
            }
            .to_dyn(),
        );
        if let Some(pos) = self
            .patch
            .iter()
            .position(|(id, _)| *id == DataComponent::CustomName)
        {
            self.patch[pos].1 = component;
        } else {
            self.patch.push((DataComponent::CustomName, component));
        }
    }

    #[must_use]
    pub fn has_custom_name(&self) -> bool {
        self.get_data_component::<crate::data_component_impl::CustomNameImpl>()
            .is_some()
    }

    #[must_use]
    pub fn get_custom_name(&self) -> Option<&papokin_util::text::TextComponent> {
        self.get_data_component::<crate::data_component_impl::CustomNameImpl>()
            .map(|c| &c.name)
    }

    pub fn remove_custom_name(&mut self) {
        self.patch
            .retain(|(id, _)| *id != DataComponent::CustomName);
    }

    #[must_use]
    pub fn get_hover_name(&self) -> String {
        if let Some(custom_name) =
            self.get_data_component::<crate::data_component_impl::CustomNameImpl>()
        {
            return custom_name.name.clone().get_text();
        }
        if let Some(item_name) =
            self.get_data_component::<crate::data_component_impl::ItemNameImpl>()
        {
            return item_name.name.to_string();
        }
        self.item.registry_key.to_string()
    }

    #[must_use]
    pub fn get_repair_cost(&self) -> i32 {
        self.get_data_component::<crate::data_component_impl::RepairCostImpl>()
            .map_or(0, |value| value.cost)
    }

    pub fn set_repair_cost(&mut self, cost: i32) {
        if cost <= 0 {
            self.patch
                .retain(|(id, _)| *id != DataComponent::RepairCost);
            return;
        }
        self.set_data_component(crate::data_component_impl::RepairCostImpl { cost });
    }

    #[must_use]
    pub fn is_valid_repair_item(&self, repair_item: &ItemStack) -> bool {
        let repairable = self.get_data_component::<crate::data_component_impl::RepairableImpl>();
        repairable.is_some_and(|r| r.is_valid_repair_item(repair_item))
    }

    #[must_use]
    pub fn get_swing_animation(&self) -> SwingAnimationImpl {
        self.get_data_component::<SwingAnimationImpl>()
            .copied()
            .unwrap_or(SwingAnimationImpl::DEFAULT)
    }

    #[must_use]
    pub fn get_rarity(&self) -> Rarity {
        let base = self
            .get_data_component::<RarityImpl>()
            .map_or(Rarity::Common, |r| r.rarity);
        if !self.has_enchantments() {
            return base;
        }
        match base {
            Rarity::Common | Rarity::Uncommon => Rarity::Rare,
            Rarity::Rare => Rarity::Epic,
            Rarity::Epic => Rarity::Epic,
        }
    }

    #[must_use]
    pub fn custom_data_compound(&self) -> Option<&NbtCompound> {
        self.get_data_component::<CustomDataImpl>()
            .map(|custom_data| &custom_data.data)
    }

    pub fn set_custom_data(&mut self, namespace: &str, key: &str, value: NbtTag) {
        let mut custom_data = self
            .get_data_component::<CustomDataImpl>()
            .map_or_else(NbtCompound::new, |custom_data| custom_data.data.clone());

        let mut namespace_data = custom_data
            .child_tags
            .remove(namespace)
            .and_then(|tag| match tag {
                NbtTag::Compound(compound) => Some(compound),
                _ => None,
            })
            .unwrap_or_default();

        namespace_data.child_tags.insert(key.into(), value);
        custom_data
            .child_tags
            .insert(namespace.into(), NbtTag::Compound(namespace_data));

        self.set_custom_data_component(custom_data);
    }

    fn set_custom_data_component(&mut self, custom_data: NbtCompound) {
        let component = Some(CustomDataImpl { data: custom_data }.to_dyn());
        if let Some((_, data)) = self
            .patch
            .iter_mut()
            .find(|(id, _)| *id == DataComponent::CustomData)
        {
            *data = component;
        } else {
            self.patch.push((DataComponent::CustomData, component));
        }
    }

    pub fn get_custom_data(&self, namespace: &str, key: &str) -> Option<NbtTag> {
        self.custom_data_compound()?
            .get(namespace)?
            .extract_compound()?
            .get(key)
            .cloned()
    }

    pub fn remove_custom_data(&mut self, namespace: &str, key: &str) {
        let Some(mut custom_data) = self
            .get_data_component::<CustomDataImpl>()
            .map(|custom_data| custom_data.data.clone())
        else {
            return;
        };

        let Some(NbtTag::Compound(mut namespace_data)) = custom_data.child_tags.remove(namespace)
        else {
            return;
        };

        namespace_data.child_tags.remove(key);
        if !namespace_data.is_empty() {
            custom_data
                .child_tags
                .insert(namespace.into(), NbtTag::Compound(namespace_data));
        }

        if custom_data.is_empty() {
            self.patch
                .retain(|(id, _)| *id != DataComponent::CustomData);
        } else {
            self.set_custom_data_component(custom_data);
        }
    }

    #[must_use]
    pub fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
        self.get_custom_data(namespace, key).is_some()
    }

    #[must_use]
    pub fn split(&mut self, amount: u8) -> Self {
        let min = amount.min(self.item_count);
        let stack = self.copy_with_count(min);
        self.decrement(min);
        stack
    }

    #[must_use]
    pub fn split_unless_creative(&mut self, gamemode: GameMode, amount: u8) -> Self {
        let min = amount.min(self.item_count);
        let stack = self.copy_with_count(min);
        if gamemode != GameMode::Creative {
            self.decrement(min);
        }
        stack
    }

    #[must_use]
    pub fn copy_with_count(&self, count: u8) -> Self {
        let mut stack = self.clone();
        stack.uid = ITEM_STACK_ID_GEN.next_id();
        stack.item_count = count;
        stack
    }

    pub const fn set_count(&mut self, count: u8) {
        self.item_count = count;
    }

    pub fn decrement_unless_creative(&mut self, gamemode: GameMode, amount: u8) {
        if gamemode != GameMode::Creative {
            self.item_count = self.item_count.saturating_sub(amount);
            if self.item_count == 0 {
                self.clear();
            }
        }
    }

    pub const fn decrement(&mut self, amount: u8) {
        self.item_count = self.item_count.saturating_sub(amount);
    }

    pub const fn increment(&mut self, amount: u8) {
        self.item_count = self.item_count.saturating_add(amount);
    }

    /// 将物品堆完全重置为空气
    pub fn clear(&mut self) {
        *self = Self::EMPTY.clone();
    }

    pub fn enchant(&mut self, enchantment: &'static Enchantment, level: i32) {
        if level <= 0 {
            return;
        }
        let level = min(level, 255);
        if let Some(data) = self.get_data_component_mut::<EnchantmentsImpl>() {
            for (enc, old_level) in data.enchantment.to_mut() {
                if *enc == enchantment {
                    *old_level = max(*old_level, level);
                    return;
                }
            }
            data.enchantment.to_mut().push((enchantment, level));
        } else {
            self.patch.push((
                Enchantments,
                Some(
                    EnchantmentsImpl {
                        enchantment: Cow::Owned(vec![(enchantment, level)]),
                    }
                    .to_dyn(),
                ),
            ));
        }
    }

    #[must_use]
    pub fn are_items_and_components_equal(&self, other: &Self) -> bool {
        // 物品必须匹配
        if self.item != other.item {
            return false;
        }

        if self.patch.len() != other.patch.len() {
            return false;
        }

        for (id, data) in &self.patch {
            let mut not_found = true;
            'out: for (other_id, other_data) in &other.patch {
                if id == other_id {
                    if let (Some(data), Some(other_data)) = (data, other_data) {
                        if !data.equal(other_data.as_ref()) {
                            return false;
                        }
                        not_found = false;
                        break 'out;
                    } else if data.is_none() && other_data.is_none() {
                        not_found = false;
                        break 'out;
                    }
                    return false;
                }
            }
            if not_found {
                return false;
            }
        }

        true
    }

    #[must_use]
    pub fn are_equal(&self, other: &Self) -> bool {
        self.item_count == other.item_count && self.are_items_and_components_equal(other)
    }

    /// 根据工具规则确定方块的开采速度。
    /// 直接匹配立即返回，带标签的方块单独检查。
    /// 若未找到匹配项，则返回工具的默认挖掘速度或 `1.0`。
    #[must_use]
    pub fn get_speed(&self, block: &'static Block) -> f32 {
        // 无工具？使用默认速度
        if let Some(tool) = self.get_data_component::<ToolImpl>() {
            for rule in tool.rules.iter() {
                // 若速度未设置则跳过
                let Some(speed) = rule.speed else {
                    continue;
                };
                match &rule.blocks {
                    IDSet::Tag(tag) => {
                        if block.is_tagged_with(tag).unwrap_or(false) {
                            return speed;
                        }
                    }
                    IDSet::IDs(blocks) => {
                        if blocks.contains(&block) {
                            return speed;
                        }
                    }
                }
            }
            tool.default_mining_speed
        } else {
            1.0
        }
    }

    /// 根据工具规则判断工具是否对方块掉落有效。
    /// 直接匹配立即返回，而带标签的方块则单独检查。
    #[must_use]
    pub fn is_correct_for_drops(&self, block: &'static Block) -> bool {
        if let Some(tool) = self.get_data_component::<ToolImpl>() {
            for rule in tool.rules.iter() {
                // 若速度未设置则跳过
                let Some(correct) = rule.correct_for_drops else {
                    continue;
                };
                match &rule.blocks {
                    IDSet::Tag(tag) => {
                        if block.is_tagged_with(tag).unwrap_or(false) {
                            return correct;
                        }
                    }
                    IDSet::IDs(blocks) => {
                        if blocks.contains(&block) {
                            return correct;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn write_item_stack(&self, compound: &mut NbtCompound) {
        // Minecraft 1.21.4 将 "id" 用作带命名空间 ID 的字符串（minecraft:diamond_sword）
        compound.put_string("id", format!("minecraft:{}", self.item.registry_key));
        compound.put_int("count", self.item_count as i32);

        // 为附加数据创建 tag compound
        let mut tag = NbtCompound::new();

        for (id, data) in &self.patch {
            if let Some(data) = data {
                tag.put(id.to_name(), data.write_data());
            } else {
                let name = '!'.to_string() + id.to_name();
                tag.put(name.as_str(), NbtCompound::new());
            }
        }

        // 存储自定义数据（如附魔、显示名称等）将放在此处
        compound.put_compound("components", tag);
    }

    #[must_use]
    pub fn read_item_stack(compound: &NbtCompound) -> Option<Self> {
        // 获取 ID，即类似 "minecraft:diamond_sword" 的字符串
        let full_id = compound.get_string("id")?;

        // 如存在则移除 "minecraft:" 前缀
        let registry_key = full_id.strip_prefix("minecraft:").unwrap_or(full_id);

        // 尝试通过注册表键获取物品
        let item = Item::from_registry_key(registry_key)?;

        let count = compound.get_int("count")? as u8;

        // 创建物品堆
        let mut item_stack = Self::new(count, item);

        // 处理组件复合标签中的附加数据
        if let Some(tag) = compound.get_compound("components") {
            for (name, data) in &tag.child_tags {
                if let Some(name) = name.strip_prefix("!") {
                    item_stack
                        .patch
                        .push((DataComponent::try_from_name(name)?, None));
                } else {
                    let id = DataComponent::try_from_name(name)?;
                    item_stack.patch.push((id, Some(read_data(id, data)?)));
                }
            }
        }

        Some(item_stack)
    }
}

impl From<&RecipeResultStruct> for ItemStack {
    fn from(value: &RecipeResultStruct) -> Self {
        Self::new(
            value.count,
            Item::from_registry_key(value.id.strip_prefix("minecraft:").unwrap_or(value.id))
                .unwrap_or(&Item::AIR),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_component::DataComponent;
    use crate::data_component_impl::{
        CustomDataImpl, CustomNameImpl, DataComponentImpl, EnchantmentsImpl, ItemNameImpl,
        LoreImpl, UnbreakableImpl,
    };

    /// 辅助函数：创建一把全新的铁剑（max_damage 250，damage 0）。
    fn iron_sword() -> ItemStack {
        ItemStack::new(1, &Item::IRON_SWORD)
    }

    #[test]
    fn items_with_different_components_are_not_equal_in_either_direction() {
        let plain = ItemStack::new(1, &Item::COAL);

        let mut customized = ItemStack::new(1, &Item::COAL);
        customized
            .patch
            .push((DataComponent::Unbreakable, Some(UnbreakableImpl.to_dyn())));

        assert!(!plain.are_items_and_components_equal(&customized));
        assert!(!customized.are_items_and_components_equal(&plain));
        assert!(customized.are_items_and_components_equal(&customized.clone()));
    }

    #[test]
    fn custom_data_sets_and_reads_typed_values() {
        let mut stack = ItemStack::new(1, &Item::WOODEN_AXE);

        stack.set_custom_data("test_plugin", "marker", NbtTag::Byte(1));
        stack.set_custom_data("test_plugin", "mode", NbtTag::String("pos1".into()));
        stack.set_custom_data(
            "test_plugin",
            "payload",
            NbtTag::ByteArray(vec![0, 1, 127, -128, -1].into()),
        );

        assert_eq!(
            stack.get_custom_data("test_plugin", "marker"),
            Some(NbtTag::Byte(1))
        );
        assert_eq!(
            stack.get_custom_data("test_plugin", "mode"),
            Some(NbtTag::String("pos1".into()))
        );
        assert_eq!(
            stack.get_custom_data("test_plugin", "payload"),
            Some(NbtTag::ByteArray(vec![0, 1, 127, -128, -1].into()))
        );
        assert!(stack.has_custom_data("test_plugin", "marker"));
    }

    #[test]
    fn custom_data_sets_and_reads_full_nbt_tags() {
        let mut stack = ItemStack::new(1, &Item::WOODEN_AXE);
        let mut compound = NbtCompound::new();
        compound.child_tags.insert("byte".into(), NbtTag::Byte(1));
        compound.child_tags.insert("short".into(), NbtTag::Short(2));
        compound.child_tags.insert("int".into(), NbtTag::Int(3));
        compound.child_tags.insert("long".into(), NbtTag::Long(4));
        compound
            .child_tags
            .insert("float".into(), NbtTag::Float(5.0));
        compound
            .child_tags
            .insert("double".into(), NbtTag::Double(6.0));
        compound
            .child_tags
            .insert("string".into(), NbtTag::String("value".into()));
        compound.child_tags.insert(
            "list".into(),
            NbtTag::List(vec![NbtTag::Int(1), NbtTag::Int(2)]),
        );
        compound
            .child_tags
            .insert("byte_array".into(), NbtTag::ByteArray(vec![1, 2].into()));
        compound
            .child_tags
            .insert("int_array".into(), NbtTag::IntArray(vec![3, 4]));
        compound
            .child_tags
            .insert("long_array".into(), NbtTag::LongArray(vec![5, 6]));

        let tag = NbtTag::Compound(compound);
        stack.set_custom_data("test_plugin", "full", tag.clone());

        assert_eq!(stack.get_custom_data("test_plugin", "full"), Some(tag));
    }

    #[test]
    fn custom_data_preserves_sibling_namespaces_and_keys() {
        let mut stack = ItemStack::new(1, &Item::WOODEN_AXE);

        stack.set_custom_data("test_plugin", "marker", NbtTag::Byte(1));
        stack.set_custom_data("test_plugin", "mode", NbtTag::String("pos1".into()));
        stack.set_custom_data("other_plugin", "flag", NbtTag::Byte(1));
        stack.set_custom_data("test_plugin", "marker", NbtTag::Byte(0));

        assert_eq!(
            stack.get_custom_data("test_plugin", "marker"),
            Some(NbtTag::Byte(0))
        );
        assert_eq!(
            stack.get_custom_data("test_plugin", "mode"),
            Some(NbtTag::String("pos1".into()))
        );
        assert_eq!(
            stack.get_custom_data("other_plugin", "flag"),
            Some(NbtTag::Byte(1))
        );
    }

    #[test]
    fn remove_custom_data_removes_only_target_key_and_cleans_empty_component() {
        let mut stack = ItemStack::new(1, &Item::WOODEN_AXE);

        stack.set_custom_data("test_plugin", "marker", NbtTag::Byte(1));
        stack.set_custom_data("test_plugin", "mode", NbtTag::String("pos1".into()));
        stack.set_custom_data("other_plugin", "flag", NbtTag::Byte(1));

        stack.remove_custom_data("test_plugin", "marker");
        assert!(!stack.has_custom_data("test_plugin", "marker"));
        assert_eq!(
            stack.get_custom_data("test_plugin", "mode"),
            Some(NbtTag::String("pos1".into()))
        );
        assert_eq!(
            stack.get_custom_data("other_plugin", "flag"),
            Some(NbtTag::Byte(1))
        );
        assert!(stack.get_data_component::<CustomDataImpl>().is_some());

        stack.remove_custom_data("test_plugin", "mode");
        stack.remove_custom_data("other_plugin", "flag");
        assert!(stack.get_data_component::<CustomDataImpl>().is_none());
    }

    #[test]
    fn custom_data_preserves_other_item_components() {
        let mut stack = ItemStack::new(1, &Item::WOODEN_AXE);
        stack.patch.push((
            DataComponent::CustomName,
            Some(
                CustomNameImpl {
                    name: papokin_util::text::TextComponent::text("标记的物品"),
                }
                .to_dyn(),
            ),
        ));
        stack
            .patch
            .push((DataComponent::Unbreakable, Some(UnbreakableImpl.to_dyn())));

        stack.set_custom_data("test_plugin", "marker", NbtTag::Byte(1));
        stack.remove_custom_data("test_plugin", "missing");

        assert!(stack.get_data_component::<CustomNameImpl>().is_some());
        assert!(stack.get_data_component::<UnbreakableImpl>().is_some());
        assert_eq!(
            stack.get_custom_data("test_plugin", "marker"),
            Some(NbtTag::Byte(1))
        );
    }

    #[test]
    fn lore_can_be_set_and_appended() {
        let mut stack = ItemStack::new(1, &Item::WOODEN_AXE);
        stack.set_lore(vec![papokin_util::text::TextComponent::text("第一行")]);
        stack.add_lore(papokin_util::text::TextComponent::text("第二行"));

        let lore = stack
            .get_data_component::<LoreImpl>()
            .expect("应存在 lore 组件");
        assert_eq!(lore.lines.len(), 2);
        assert_eq!(lore.lines[0].clone().get_text(), "第一行");
        assert_eq!(lore.lines[1].clone().get_text(), "第二行");
    }

    #[test]
    fn custom_data_survives_item_stack_nbt_roundtrip() {
        let mut stack = ItemStack::new(1, &Item::WOODEN_AXE);
        stack.set_custom_data("test_plugin", "marker", NbtTag::Byte(1));
        stack.set_custom_data("test_plugin", "mode", NbtTag::String("pos1".into()));
        stack
            .patch
            .push((DataComponent::Unbreakable, Some(UnbreakableImpl.to_dyn())));

        let mut compound = NbtCompound::new();
        stack.write_item_stack(&mut compound);
        let decoded = ItemStack::read_item_stack(&compound).expect("堆叠应能解码");

        assert_eq!(
            decoded.get_custom_data("test_plugin", "marker"),
            Some(NbtTag::Byte(1))
        );
        assert_eq!(
            decoded.get_custom_data("test_plugin", "mode"),
            Some(NbtTag::String("pos1".into()))
        );
        assert!(decoded.get_data_component::<UnbreakableImpl>().is_some());
    }

    #[test]
    fn translated_item_name_survives_item_stack_nbt_roundtrip() {
        let mut stack = ItemStack::new(1, &Item::FILLED_MAP);
        stack.patch.push((
            DataComponent::ItemName,
            Some(
                ItemNameImpl {
                    name: Cow::Borrowed("filled_map.mansion"),
                }
                .to_dyn(),
            ),
        ));

        let mut compound = NbtCompound::new();
        stack.write_item_stack(&mut compound);
        let decoded = ItemStack::read_item_stack(&compound).expect("堆叠应能解码");

        assert_eq!(
            decoded
                .get_data_component::<ItemNameImpl>()
                .expect("物品名称应能解码")
                .name,
            "filled_map.mansion"
        );
    }

    // ── damage_item ───────────────────────────────────────────────

    #[test]
    fn damage_zero_amount_is_noop() {
        let mut stack = iron_sword();
        assert_eq!(stack.damage_item(0), DamageResult::Untouched);
        assert_eq!(stack.get_damage(), 0);
    }

    #[test]
    fn damage_negative_amount_is_noop() {
        let cases: &[i32] = &[-1, -5, -10, -100];
        for &amount in cases {
            let mut stack = iron_sword();
            assert_eq!(
                stack.damage_item(amount),
                DamageResult::Untouched,
                "expected no damage for amount={amount}"
            );
            assert_eq!(stack.get_damage(), 0, "damage mismatch for amount={amount}");
        }
    }

    #[test]
    fn damage_non_damageable_item_is_noop() {
        // AIR 没有 MaxDamage 组件。
        let mut stack = ItemStack::new(1, &Item::AIR);
        assert_eq!(stack.damage_item(1), DamageResult::Untouched);
    }

    #[test]
    fn damage_unbreakable_item_is_noop() {
        let cases: &[i32] = &[1, 5, 10, 100, 250];
        for &amount in cases {
            let mut stack = iron_sword();
            stack
                .patch
                .push((DataComponent::Unbreakable, Some(UnbreakableImpl.to_dyn())));
            assert_eq!(
                stack.damage_item(amount),
                DamageResult::Untouched,
                "expected no damage for unbreakable item, amount={amount}"
            );
            assert_eq!(
                stack.get_damage(),
                0,
                "damage mismatch for unbreakable item, amount={amount}"
            );
        }
    }

    #[test]
    fn damage_increases_damage_value() {
        // 没有耐久（Unbreaking）时，每一点伤害都会生效。
        // 每个子数组为 (amount, expected_damage)；每种情况使用一把全新的 iron_sword
        let cases: &[(i32, i32)] = &[(1, 1), (5, 5), (10, 10), (100, 100), (249, 249)];
        for &(amount, expected) in cases {
            let mut stack = iron_sword();
            assert_eq!(
                stack.damage_item(amount),
                DamageResult::Damaged,
                "expected damage_item to return Damaged for amount={amount}"
            );
            assert_eq!(
                stack.get_damage(),
                expected,
                "damage mismatch for amount={amount}"
            );
        }
    }

    #[test]
    fn damage_accumulates() {
        // 每条记录：(first_amount, second_amount, expected_total)
        let cases: &[(i32, i32, i32)] = &[(100, 50, 150), (10, 20, 30), (1, 1, 2), (50, 100, 150)];
        for &(first, second, expected) in cases {
            let mut stack = iron_sword();
            let _ = stack.damage_item(first);
            let _ = stack.damage_item(second);
            assert_eq!(
                stack.get_damage(),
                expected,
                "accumulated damage mismatch for first={first}, second={second}"
            );
        }
    }

    #[test]
    fn damage_breaks_item_when_exceeding_max() {
        // 铁剑 max_damage = 250；任何 >= 250 的数值都应将其摧毁。
        let cases: &[i32] = &[250, 260, 300, 1000];
        for &amount in cases {
            let mut stack = iron_sword();
            assert_eq!(
                stack.damage_item(amount),
                DamageResult::Broken,
                "expected item to break for amount={amount}"
            );
            assert!(
                stack.is_empty(),
                "item should be destroyed for amount={amount}"
            );
        }
    }

    #[test]
    fn damage_item_changes_component_equality() {
        let mut stack = iron_sword();
        let original = stack.clone();
        assert_eq!(stack.damage_item(1), DamageResult::Damaged);
        assert!(
            !stack.are_equal(&original),
            "durability patch must differ so inventory sync sends SET_SLOT"
        );
        assert_eq!(stack.get_damage(), 1);
    }

    #[test]
    fn damage_breaks_single_item_to_empty() {
        let mut stack = iron_sword();
        let _ = stack.damage_item(300);
        assert!(stack.is_empty());
        assert_eq!(stack.item_count, 0);
    }

    // ── repair_item ──────────────────────────────────────────────────

    #[test]
    fn repair_zero_amount_is_noop() {
        let initial_damages: &[i32] = &[1, 5, 10, 100, 249];
        for &initial in initial_damages {
            let mut stack = iron_sword();
            stack.set_damage(initial);
            assert_eq!(
                stack.repair_item(0),
                0,
                "repair(0) should return 0 for initial={initial}"
            );
            assert_eq!(
                stack.get_damage(),
                initial,
                "damage should be unchanged for initial={initial}"
            );
        }
    }

    #[test]
    fn repair_negative_amount_is_noop() {
        let cases: &[i32] = &[-1, -5, -10, -100];
        for &amount in cases {
            let mut stack = iron_sword();
            stack.set_damage(10);
            assert_eq!(
                stack.repair_item(amount),
                0,
                "repair({amount}) should return 0"
            );
            assert_eq!(
                stack.get_damage(),
                10,
                "damage should be unchanged for repair({amount})"
            );
        }
    }

    #[test]
    fn repair_undamaged_item_is_noop() {
        let amounts: &[i32] = &[1, 5, 10, 100];
        for &amount in amounts {
            let mut stack = iron_sword();
            assert_eq!(
                stack.repair_item(amount),
                0,
                "repair({amount}) on undamaged item should return 0"
            );
            assert_eq!(
                stack.get_damage(),
                0,
                "undamaged item should remain at 0 after repair({amount})"
            );
        }
    }

    #[test]
    fn repair_partial() {
        // 每条记录：(initial_damage, repair_amount, expected_repaired, expected_remaining)
        let cases: &[(i32, i32, i32, i32)] = &[
            (20, 8, 8, 12),
            (50, 25, 25, 25),
            (100, 30, 30, 70),
            (249, 1, 1, 248),
        ];
        for &(initial, repair, exp_repaired, exp_remaining) in cases {
            let mut stack = iron_sword();
            stack.set_damage(initial);
            let repaired = stack.repair_item(repair);
            assert_eq!(
                repaired, exp_repaired,
                "repaired amount mismatch for initial={initial}, repair={repair}"
            );
            assert_eq!(
                stack.get_damage(),
                exp_remaining,
                "remaining damage mismatch for initial={initial}, repair={repair}"
            );
        }
    }

    #[test]
    fn repair_capped_at_current_damage() {
        // 每条记录：(initial_damage, repair_amount)；修复量超过损伤量，因此 repaired == initial
        let cases: &[(i32, i32)] = &[(5, 6), (5, 100), (10, 11), (100, 200)];
        for &(initial, repair) in cases {
            let mut stack = iron_sword();
            stack.set_damage(initial);
            let repaired = stack.repair_item(repair);
            assert_eq!(
                repaired, initial,
                "repaired amount mismatch for initial={initial}, repair={repair}"
            );
            assert_eq!(
                stack.get_damage(),
                0,
                "damage should be 0 after over-repair for initial={initial}"
            );
        }
    }

    #[test]
    fn repair_fully_clears_damage_component() {
        let mut stack = iron_sword();
        stack.set_damage(10);
        stack.repair_item(10);
        assert_eq!(stack.get_damage(), 0);
        // set_damage(0) 会移除 Damage 补丁条目。
        assert!(
            !stack
                .patch
                .iter()
                .any(|(id, _)| *id == DataComponent::Damage)
        );
    }

    // ── 叠放物品的损坏 ────────────────────────────────────────

    #[test]
    fn damage_stacked_item_breaks_one_and_resets_durability() {
        // 两把铁剑（max_damage 250），损伤值 249——再受一次打击就会损坏。
        // 没有耐久（Unbreaking）时，伤害判定总会生效，因此这是确定性的。
        let mut stack = ItemStack::new(2, &Item::IRON_SWORD);
        stack.set_damage(249);

        let result = stack.damage_item(1);

        assert_eq!(
            result,
            DamageResult::Broken,
            "stacked item at max damage should return Broken"
        );
        assert_eq!(stack.item_count, 1, "stack count should drop from 2 to 1");
        assert_eq!(
            stack.get_damage(),
            0,
            "remaining sword's durability should reset to 0 after breaking"
        );
        assert!(
            !stack.is_empty(),
            "one sword should still remain in the stack"
        );
    }

    // ── 武器类别判定谓词 ───────────────────────────────────

    /// 拥有 2 点耐久的战斗武器（斧/镐/锹/锄）必须匹配其类别谓词。
    #[test]
    fn weapon_categories_identify_2_cost_items() {
        // 应将 is_axe / is_pickaxe / is_shovel / is_hoe 设为 true 的物品。
        let axes: &[&Item] = &[
            &Item::WOODEN_AXE,
            &Item::STONE_AXE,
            &Item::IRON_AXE,
            &Item::GOLDEN_AXE,
            &Item::DIAMOND_AXE,
            &Item::NETHERITE_AXE,
        ];
        let pickaxes: &[&Item] = &[
            &Item::WOODEN_PICKAXE,
            &Item::STONE_PICKAXE,
            &Item::IRON_PICKAXE,
            &Item::GOLDEN_PICKAXE,
            &Item::DIAMOND_PICKAXE,
            &Item::NETHERITE_PICKAXE,
        ];
        let shovels: &[&Item] = &[
            &Item::WOODEN_SHOVEL,
            &Item::STONE_SHOVEL,
            &Item::IRON_SHOVEL,
            &Item::GOLDEN_SHOVEL,
            &Item::DIAMOND_SHOVEL,
            &Item::NETHERITE_SHOVEL,
        ];
        let hoes: &[&Item] = &[
            &Item::WOODEN_HOE,
            &Item::STONE_HOE,
            &Item::IRON_HOE,
            &Item::GOLDEN_HOE,
            &Item::DIAMOND_HOE,
            &Item::NETHERITE_HOE,
        ];

        for item in axes {
            let stack = ItemStack::new(1, item);
            assert!(stack.is_axe(), "{} should be an axe", item.registry_key);
            assert!(
                !stack.is_sword(),
                "{} should not be a sword",
                item.registry_key
            );
        }
        for item in pickaxes {
            let stack = ItemStack::new(1, item);
            assert!(
                stack.is_pickaxe(),
                "{} should be a pickaxe",
                item.registry_key
            );
        }
        for item in shovels {
            let stack = ItemStack::new(1, item);
            assert!(
                stack.is_shovel(),
                "{} should be a shovel",
                item.registry_key
            );
        }
        for item in hoes {
            let stack = ItemStack::new(1, item);
            assert!(stack.is_hoe(), "{} should be a hoe", item.registry_key);
        }

        // 剑的花费应为 1，因此它们绝不能匹配任何花费为 2 的谓词。
        let swords: &[&Item] = &[
            &Item::IRON_SWORD,
            &Item::DIAMOND_SWORD,
            &Item::NETHERITE_SWORD,
        ];
        for item in swords {
            let stack = ItemStack::new(1, item);
            assert!(stack.is_sword(), "{} should be a sword", item.registry_key);
            assert!(
                !stack.is_axe(),
                "{} should not be an axe",
                item.registry_key
            );
            assert!(
                !stack.is_pickaxe(),
                "{} should not be a pickaxe",
                item.registry_key
            );
        }
    }

    // ── 耐久（Unbreaking，统计方式）────────────────────────────────────

    /// 辅助函数：附带 `level` 级耐久附魔的铁剑。
    fn with_unbreaking(item: &'static Item, level: i32) -> ItemStack {
        let mut s = ItemStack::new(1, item);
        s.patch.push((
            DataComponent::Enchantments,
            Some(
                EnchantmentsImpl {
                    enchantment: std::borrow::Cow::Owned(vec![(&Enchantment::UNBREAKING, level)]),
                }
                .to_dyn(),
            ),
        ));
        s
    }

    /// 耐久 III 工具：25% 的生效概率。4 000 次试验，期望约 1 000 次命中（区间 865–1135）。
    /// ±5σ 置信窗口确保能捕捉到性能回退；对 CI 安全且在统计上有意义。
    /// 注意：使用线程本地的 rand::random()。
    /// 通过重构 should_apply_durability_damage_with 使其接受 RNG 参数，即可实现完全确定性。
    #[test]
    fn unbreaking_iii_tool_applies_roughly_25_percent_of_hits() {
        let mut stack = with_unbreaking(&Item::NETHERITE_PICKAXE, 3);
        let mut applied: u32 = 0;
        for _ in 0..4_000 {
            if stack.damage_item(1) != DamageResult::Untouched {
                applied += 1;
            }
        }
        assert!(
            (865..=1_135).contains(&applied),
            "Unbreaking III tool: expected ~1 000 applications in 4 000 trials, got {applied}"
        );
    }

    /// 耐久 III 盔甲：70% 的生效概率。500 次试验，期望约 350 次命中（区间 300–400）。
    /// 参见工具测试中关于线程局部 RNG 的说明；经过重构即可通过带种子的 RNG 参数实现完全确定性。
    #[test]
    fn unbreaking_iii_armor_applies_roughly_70_percent_of_hits() {
        let mut stack = with_unbreaking(&Item::DIAMOND_CHESTPLATE, 3);
        let mut applied: u32 = 0;
        for _ in 0..500 {
            if stack.damage_item(1) != DamageResult::Untouched {
                applied += 1;
            }
        }
        // 预期约 350，70% 概率，±5σ 置信度（300–400 区间，约 99.7% 不抖动）。
        assert!(
            (300..=400).contains(&applied),
            "Unbreaking III armor: expected ~350 applications in 500 trials, got {applied}"
        );
    }

    // ── set_damage ───────────────────────────────────────────────────

    #[test]
    fn set_damage_negative_clamps_to_zero() {
        let cases: &[i32] = &[-1, -10, -100, i32::MIN];
        for &amount in cases {
            let mut stack = iron_sword();
            stack.set_damage(amount);
            assert_eq!(
                stack.get_damage(),
                0,
                "damage should clamp to 0 for set_damage({amount})"
            );
        }
    }
}
