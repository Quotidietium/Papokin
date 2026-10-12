# 轮次 45：装备存储紧凑化（判别值直索引 + 掉落概率去 Arc 盒）

> 日期：2026-10-13 ｜ 主题：内存（装备容器形态）+ CPU（读路径顺带） ｜ 闸门：4/4 PASS
> 基准：[benchmark/src/bin/equipment_slots_compact.rs](../../../benchmark/src/bin/equipment_slots_compact.rs)
> 数据：[round45-equipment-slots-compact.json](round45-equipment-slots-compact.json)

## 问题

装备相关的两个逐实体存储都以 `EquipmentSlot` 为哈希键，形态与键空间
严重失配：

1. **`EntityEquipment.equipment: HashMap<EquipmentSlot, ItemStack>`**——
   键空间全仓恒为 8 个 `const` 规范实例（`EquippableImpl.slot` 是
   `&'static`、`get_from_name` 返回静态表、寻路/掉落全部传引用
   const），却在为「任意键集合」付出哈希表代价：每次 `put` 克隆
   56B 键（`EquipmentSlot` 携 `Cow<str>` 的 `EquipmentSlotData`）、
   容量梯增长、逐操作哈希探测；且空 `HashMap` 内联 48B。
2. **`equipment_drop_chances: Arc<Mutex<FxHashMap<EquipmentSlot, f32>>>`**——
   只会显式写入少数槽位、读取回落默认值 0.085 的「稀疏覆盖层」，
   却让**每个生物**（含永远不装备的牛羊鸡）都背一个
   `Arc<Mutex<HashMap>>` 堆盒（约 72-80B：Arc 头 + SRW 锁 + 表内联），
   且全仓无任何共享（`Arc` 纯浪费——轮次 43 扫描只看了表没看盒）。

## 机制

### 1. `EntityEquipment` 判别值直索引（papokin-inventory）

```rust
pub struct EntityEquipment {
    slots: Option<Box<[Option<ItemStack>; 8]>>,
}
```

- `EquipmentSlot::discriminant()`（既有方法，0-7）作下标：`None`
  外层 = 从未装备（零堆）；装第一件时分配 8 槽数组；
- 公开 API `put/get/is_empty/clear` 签名不变；新增对齐旧
  `HashMap` 直访语义的访问器：`get_opt`/`get_mut`/`remove`/
  `iter_present`（→ `(&'static EquipmentSlot, &ItemStack)`）/
  `iter_present_mut`（`values_mut` 形态）/`iter_slots_mut`（可变
  键值迭代）；
- `static SLOT_TABLE: [EquipmentSlot; 8]` 按判别值还原规范槽实例
  （**`static` 而非 `const`**：运行时下标取引用需要 `'static`
  晋升，`const` 做不到）；papokin-data 配套补
  `EquipmentSlot::from_discriminant(i8)` 反查；
- 键空间不变式在文档钉死：同一判别值不可能对应第二种数据形态
  （全仓键出自 8 个规范实例），判别值索引是完备键。

### 2. `DropChances` 数组 + 去 Arc 盒（papokin）

```rust
pub struct DropChances([f32; 8]);   // 默认值铺底
pub equipment_drop_chances: std::sync::Mutex<DropChances>,  // 内联
```

缺键回落默认与「默认值铺底」两态合一；`write_drop_chances` 仍按
「非默认值才落盘」过滤，存档输出与旧表逐字节一致；`set_guaranteed_drop`/
装备注册表写入路径改 `set`。内联 `Mutex`（+40B 结构内）换掉
`Arc` 盒（−80B 堆），净 −40B/实体。

### 迁移面

`entity_equipment` 字段类型 `Arc<Mutex<..>>` **保留**（玩家物品栏
真实共享，`PlayerInventory` 持 Arc 句柄）；全部 ~40 个 `.lock()` 模式
调用点零改动，只迁移 ~30 处直访 `.equipment.<method>`（living/player/
combat/armor_stand/creaking/enderman/piglin/piglin_brute/piglin_ai/
campfire/magma/powder_snow/clear/world/entity_tracker/wasm 宿主/
player_inventory，含 `drain()` 清空广播点改「迭代收集 + clear」）。

## 基准与闸门

双臂结构复刻（旧臂含两个 `Arc` 盒 + `HashMap`；新臂判别值数组 +
内联 `Mutex<[f32; 8]>`；真实 `ItemStack`/`EquipmentSlot` 类型），
20000 实体（45% 敌对装备 1-4 槽 + 概率写入）+ 20 轮读电池（主手
克隆读/护甲四槽借用读/概率读/同步聚合）+ 5000 死亡掉落
（概率裁决 + remove）。预登记闸门：

| 闸门 | 结果 |
|---|---|
| 电池指纹全等 | ✅ `0xb605d2b1ed397b9d` 全等（迭代两侧按判别值规范化） |
| 驻留字节下降（主判据） | ✅ 9 753 024 → 4 481 024 B（每实体 487.7→224.1 B，**−54%**） |
| 构造+装备分配次数下降 | ✅ 62 001 → 29 001 次（余量 = 装备 `Arc` 盒 20k + 已装备槽位数组 9k） |
| 读电池耗时不劣化（≥0.91×） | ✅ 54.41 → 11.80 ms（**×4.61**，直索引免 56B 键哈希） |

构造+装备 7.45→2.15 ms（×3.47）；死亡掉落分配两臂同为 4500
（`ItemStack` 移出/回落路径本就无差异）。门禁：workspace
clippy 0 错误、fmt 干净、papokin+inventory 496 测试绿
（workspace 全量 1276 早前已绿）。

## 红线核对

- **语义**：指纹全等；`put(空物品堆)` 显式写入槽位与旧表键存在
  行为一致（`is_empty`/迭代/掉落逐点对齐）；掉落概率「缺键回落
  默认」≡「默认铺底」；存档输出（非默认过滤）逐字节一致。
- **迭代序**：旧 `HashMap` 迭代序随进程种子本就不定且无契约
  （线上 `CSetEquipment` 按条目自带索引解码、掉落独立结算）；
  新序为判别值固定序——严格更稳定，无 UX 差异。
- **并发**：锁粒度与锁对象不变（`Mutex<EntityEquipment>` 同一
  把锁）；`DropChances` 换内联 `Mutex` 同样一把锁，无新共享面。
- **UX**：装备读写/掉落/存档/广播输出完全一致。

## 同主题扫描完备性（本优化方面已做全）

以 `EquipmentSlot` 为键的容器全仓盘点：

| 容器 | 处置 |
|---|---|
| `EntityEquipment.equipment` | ✅ 本轮判别值直索引 |
| `equipment_drop_chances` | ✅ 本轮数组化 + 去 Arc 盒 |
| `equipment_slots`（usize→槽） | ✅ 轮次 43 全局共享 |
| `equipment_attribute_modifier_ids: Mutex<FxHashMap<EquipmentSlot, Vec<..>>>` | 已是内联 `Mutex`；稀疏且值侧 `Vec` 主导，数组化（8×24B 常驻）负收益，保持 |
| `attributes` 属性表 | ✅ 轮次 42 覆盖层 |

## 后续方向（轮次 46 候选）

- NBT 驻留生命周期面（轮 41 结论延续）：长期持有 `custom_data`
  的调用方盘点与落盘时机；
- 世界层区块/方块实体驻留扫描（`block_entity` 家族）；
- `active_effects: Mutex<FxHashMap<&'static StatusEffect, Effect>>`
  稀疏度与容量形态评估。
