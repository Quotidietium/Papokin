# 轮次 32 · 装备广播构造路径的克隆消除（借用序列化 + 事件订阅者先行）

## 背景：`send_equipment_changes` 的三层浪费

装备广播（`CSetEquipment`）是实体装备可见性的高频面：玩家切
快捷栏、穿脱装备、AI 换武器、观察者进入视野（tracker 初始同步、
`World` 广播）全部触发。旧形态三处浪费：

1. **双重深克隆**：先逐槽 `clone()` 进中间列表（跨锁/跨 await
   发送必需的一层），再对列表逐槽 `stack.clone()` 构造
   `ItemStackSerializer`（`From<ItemStack>` 按值）——第二层
   纯粹因为包字段是 `ItemStackSerializer<'static'>` 而存在；
2. **事件构造先于订阅者检查**：`fire_blocking` 内部虽有无处理器
   短路（`has_handlers` 在 `block_on` 之前），但调用方在调用之前
   已为每个槽位付了 `slot.to_name().to_string()`（`String`
   分配）+ 事件结构构造（含非空槽 `stack.clone()` 深克隆）——
   无插件监听该事件时全部白付；
3. `ItemStackSerializer` 本体是 `Cow<ItemStack>`，天然支持
   `Cow::Borrowed` 借用形态，但缺少 `From<&ItemStack>` 入口。

## 改动

- `papokin-protocol`：
  - `impl<'a> From<&'a ItemStack> for ItemStackSerializer<'a>`
    （`Cow::Borrowed` 借用构造，纯新增 API；编码输出与 Owned
    形态逐位一致——序列化只读）；
  - `CSetEquipment<'a>` 生命周期参数化
    （`equipment: Vec<(i8, ItemStackSerializer<'a>)>`；
    `java_packet` 宏经 `split_for_impl` 对泛型透明，`ClientPacket`
    impl 加 `'_`、`ServerPacket<'a>` 解码侧保持 Owned 语义）；
- `crates/papokin/src/entity/living.rs`：
  `send_equipment_changes` 的 `equipment_java` 借用构造（包生命
  周期绑定入参切片，省第二层深克隆）；事件通知改为
  `has_handlers::<EntityEquipmentChangedEvent>()` 先行——无监听
  时零构造零分配，有监听时构造与 fire 逐字不变；
- `world/entity_tracker.rs`：tracker 装备同步原时序本就在锁内
  序列化，改为锁内直接借用构造（空槽过滤随迭代进行，中间列表
  与双重克隆一并消除，锁持有语义零变化）；
- `world/mod.rs` 两处装备广播：跨锁发送保留中间列表（锁语义），
  serializer 从列表借用构造（第二层深克隆省去）；
- `entity/player.rs` 三处热点（`sync_hand_slot` 等）：
  `&ItemStackSerializer::from(stack.clone())` → `from(&stack)`
  （`CSetPlayerInventory::new` 参数本就是任意生命周期引用，
  即刻序列化，借用安全）。

### 红线核对（行为/UX 零变化）

- 编码输出逐位一致：`Cow::Borrowed` 与 `Cow::Owned` 在只读编码
  路径输出相同字节（基准 G1 序列化指纹全等直接实证）；
- 事件语义：无监听时旧路径构造后也被 `fire_blocking` 短路丢弃
  （无处理器即无副作用），先行检查等价；有监听时构造与派发
  逐字不变；
- 锁语义：tracker 路径原时序锁内 serialize，改造后相同；world
  两处保留锁外发送、列表生命周期覆盖包发送全程；
- 协议兼容：`CSetEquipment<'a>` 为 crate 内类型参数化，线上
  格式零变化（Packet ID 与编码不变）。

## 双臂基准（6 槽 × 每段 10 000 次）

`benchmark/src/bin/equipment_broadcast.rs`（第 32 个 bin）。
真类型负载：真 `ItemStack`（`DamageImpl`/`MaxDamageImpl`/
`RepairCostImpl` 三组件补丁）×5 槽 + 1 空槽，真
`ItemStackSerializer`/`CSetEquipment`/`serialize_packet` 编码，
指纹按编码输出字节折叠。

### 预登记门与结果（全部 PASS，EXIT=0）

| 门 | 判据 | 结果 |
|---|---|---|
| G1 | 构造段两臂序列化输出指纹一致 | PASS（0x3dd31b07ef2cee05） |
| G2 | 构造段旧臂分配次数 > 新臂 | PASS（490 000 > 80 000） |
| G3 | 构造段分配差 ≥ 广播数×非空槽 | PASS（410 000 ≥ 50 000） |
| G4 | 事件段无订阅者新臂零分配、旧臂非零 | PASS（0 vs 260 000） |
| G5 | 事件段负载有效性 | PASS |

### 数据

| 段 | 旧臂 | 新臂 | 收益 |
|---|---|---|---|
| 构造+序列化 | 490 000 allocs / 19.28 MB / 18.16 ms | 80 000 / 8.48 MB / 4.77 ms | 分配 **6.13×**、字节 2.27×、耗时 **3.81×** |
| 事件通知（无订阅者） | 260 000 allocs / 4.52 MB / 8.22 ms | **0** / **0** B / 0.7 µs | 分配 −100% |

构造段新臂残余 80 000 分配 = 每 5 槽广播 8 次列表/序列化缓冲
Vec 分配 + 序列化输出缓冲——结构性下限。

## 负面发现与后续清单

- 投射物（splash/lingering/egg）、展示实体、物品展示框、loot/
  item 命令等 `from(x.clone())` 调用点为低频路径，本轮未逐点
  改造（`From<&ItemStack>` 已就位，后续按需借用化）；
- `EntityEquipmentChangedEvent` 的 `old_item` 恒 `None`（调用方
  已更新装备表后才广播）——事件 API 语义问题，非性能面，另立
  评估。

## 提交

- `perf(protocol)`：`From<&ItemStack>` 借用构造 +
  `CSetEquipment<'a>` 参数化；
- `perf(entity)`：装备广播三路借用化 + 事件订阅者先行 +
  player 热点；
- `perf(benchmark)`：`equipment_broadcast` 双臂基准（第 32 bin）；
- `docs(note)`：本报告 + 索引；
- `chore(release)`：版本 0.3.43 dist。
