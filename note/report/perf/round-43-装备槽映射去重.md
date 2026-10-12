# 轮次 43：实体装备槽映射去重（逐实体重建 → 全局共享）

> 日期：2026-10-12 ｜ 主题：内存（每实体常量表去重）+ CPU（构造路径顺带） ｜ 闸门：4/4 PASS
> 基准：[benchmark/src/bin/equipment_slots_shared.rs](../../../benchmark/src/bin/equipment_slots_shared.rs)
> 数据：[round43-equipment-slots-shared.json](round43-equipment-slots-shared.json)

## 问题

`LivingEntity::new` 对每个实体（全部生物与玩家）执行
`Arc::new(build_equipment_slots())`——而 `build_equipment_slots` 是
**无入参纯函数**，输出恒为同一张 5 键 `usize → EquipmentSlot` 映射
（脚/腿/胸/头 + 副手的 UI 槽位索引）。逐实体重建付出：

- `EquipmentSlot` 枚举携带 `EquipmentSlotData`（槽类型 + 三个 i32 +
  `Cow<str>` 名）达 48B，桶条目 56B；5 条目走容量梯 3→7，
  终态约 **520B/实体**驻留 + 3 次增长分配；
- 构造路径 3 次分配与逐键哈希插入，全部是重复劳动。

万级实体服务器仅此一项约 5MB 纯重复驻留。

## 机制

`papokin-inventory` 新增共享单实例：

```rust
pub static EQUIPMENT_SLOTS: LazyLock<Arc<FxHashMap<usize, EquipmentSlot>>> =
    LazyLock::new(|| Arc::new(build_equipment_slots()));
```

`LivingEntity::new` 改为 `EQUIPMENT_SLOTS.clone()`（一次 `Arc` 引用
计数递增，零堆分配）。字段类型 `Arc<FxHashMap<usize, EquipmentSlot>>`
不变，`PlayerInventory`（经 `player.rs` 转发 `equipment_slots.clone()`）
与全部读取点（`iter`/`get`/`values`，全仓只读）零改动迁移；
`build_equipment_slots` 保留为共享实例的构建器。

### 同主题扫描完备性（本优化方面已做全）

对「逐实体重建的常量结构」全实体族扫描，确认其余各处无同类问题：

| 结构 | 构造态 | 结论 |
|---|---|---|
| `equipment_drop_chances: Arc<Mutex<FxHashMap>>` | 空默认，仅装备/存档路径按需写入 | 本就是覆盖层，无需动 |
| `entity_equipment: Arc<Mutex<EntityEquipment>>` | 空 `HashMap`（零堆） | 未装备生物零成本，保持 |
| `Brain`/`MemoryStore` | 空默认 Vec，注册时惰性 `resize` | 已紧凑 |
| `CombatTracker` | 空条目 Vec（上限 64） | 已紧凑 |
| `Entity` 基座 | 全原子/内联单元 | 无逐实体堆分配 |
| `ItemEntity` | 原子 + `Mutex<ItemStack>` 内联 | 紧凑 |

## 基准与闸门

双臂（旧臂逐实体 `build_equipment_slots()` 重建、新臂共享 `Arc` 克隆，
两臂映射内容恒等由纯函数保证），20000 实体构造 + 10 轮读取电池
（5 键遍历 + 逐键点查 + values 收集，槽名折叠指纹）。预登记闸门：

| 闸门 | 结果 |
|---|---|
| 读取电池指纹全等 | ✅ `0xde777f49610678a5` 全等 |
| 驻留字节下降（主判据） | ✅ 10 561 544 → 161 544 B（**×65.3**，每实体 528.08→8.08 B，共享臂含单实例与语料 Vec） |
| 构造分配次数下降 | ✅ 60 001 → 4 次（每实体 3.0000→0.0002） |
| 读取耗时不劣化（≥0.91×） | ✅ 21.08 → 17.97 ms（×1.17，单实例常驻热缓存） |

构造耗时 6.67→0.07 ms（×90.4）。门禁：papokin + papokin-inventory
clippy 0 错误、papokin lib 439 测试全过。

## 红线核对

- **语义**：映射内容为纯函数输出，两臂指纹全等；所有使用点只读
  （共享不可变实例无竞争），`Arc` 字段类型不变。
- **并发**：`LazyLock` 初始化线程安全；实例构建后只读共享。
- **UX**：槽位映射结果逐字节一致，屏幕处理器/装备槽寻址无任何变化。

## 后续方向（轮次 44 候选）

- NBT 驻留生命周期面（轮 41 结论延续）：长期持有 `custom_data` 的
  调用方盘点与落盘时机；
- `EntityEquipment` 的 `HashMap<EquipmentSlot, ItemStack>` 数组化
  （仅利于已装备实体，量级中等）；
- 世界层区块/方块实体驻留的下一轮扫描。
