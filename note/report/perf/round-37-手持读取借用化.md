# 轮次 37 · 手持读取借用化：`held_item`/`get_stack_in_hand` 按值克隆 → `with_*` 锁内借用（86 个调用点甄别·收编）

## 背景：在册候选的逐点甄别

轮次 36 归档的「held_item/get_stack_in_hand 的 69 个调用点」在本轮
完成全量甄别。扩 censuses 至 `held_item` / `off_hand_item` /
`get_stack_in_hand` 三张读脸（papokin 主 crate）：**86 个调用点**。
甄别结论：

| 类别 | 处置 |
|---|---|
| **纯读白付（本轮收编）** | **约 32 个生产读取点** |
| 所有权必须（set_active_hand 激活/丢弃/抛射实体携带/装备包同步/写路径消费） | 克隆是语义必需，豁免 |
| 读改写混合（brush×5/potions×4/glass_bottle/knowledge_book/use_item 等） | mut 消费或回退赋值，豁免 |
| 并发守卫快照（enchant×2/trident:46/crossbow×2/interact:104） | update_held 写锁内以前置克隆做同物比对，豁免 |
| 事件入参（player_action:170/use_item_on 事件簇） | 已受订阅者先行门保护，仅监听时支付，豁免 |
| 攻击摊销复用（player.rs:1125 attack） | 单次克隆六处下游复用（属性/附魔/耐久），摊销后无白付 |
| 攻击路径重入风险（spear:133/bow:240） | 借用无法逃逸回调且回调内调用触及物品栏，保持克隆 |
| 恒 Some 语义（fishing_bobber:153） | 战利品工具空栈也须 `Some(EMPTY)`，空栈克隆零分配无收益 |
| MobEntity 异面 API（piglin×4/piglin_ai×2） | piglin.rs:354 自有 `off_hand_item()`（装备 HashMap 面），非本轮范围 |
| WIT 插件边界（wasm_host×2） | 导出按值返回，豁免 |
| 旧按值 API 定义本体（living.rs:2709/2765） | 保留供所有权调用方 |

关键事实与轮 36 同源：`ItemStack::clone` 深克隆组件补丁（1 Vec +
每组件一次 dyn 克隆），**空栈克隆零分配**——白付集中在挖掘速度/
AI 探测/统计读取等高频路径的非空带组件手持槽。

## 改动

### 新增借用 API（6 方法）

| 类型 | 方法 | 形态 |
|---|---|---|
| PlayerInventory | `with_held_item` / `with_off_hand_item` / `with_stack_in_hand` | 主物品栏读锁内选中槽借用；副手走 equipment_slots，无槽位回调收 `ItemStack::EMPTY` |
| LivingEntity | 同名三方法 | 玩家分支分发到 PlayerInventory 变体；纯装备实体走 entity_equipment 锁内 HashMap 借用 |

回调约束与 `with_stack` 同源：回调体只调用 ItemStack 自身方法，
不重入同一物品栏。

### 收编点（约 32 处，按主题分组）

| 路径 | 文件 | 用途 |
|---|---|---|
| 掠夺等级 | living.rs:2138 | 战利品附魔读取（战利品工具仅非空才克隆） |
| 弩装填判定 | living.rs:2448 / crossbow.rs:26 | is_crossbow 探测（装填路径所有权另取） |
| 破甲等级 | living.rs:3074 | 附魔读取 |
| 损伤后空槽探测 | living.rs:3374 | damage_item 后 is_empty |
| **损伤事件先行门** | living.rs:3380 | 无监听零构造零克隆（cancelled=false 同源） |
| 工具耐久读取 | player.rs:1611 | ToolImpl damage |
| 收获判定 | player.rs:4819 | can_harvest（tool_required 短路前置） |
| 挖掘效率同步 | player.rs:4828 | EFFICIENCY 等级（每方块变化） |
| **挖掘速度** | player.rs:4857 | get_mining_speed（高频热路径） |
| 地图 id 探测 | player.rs:5097 | tick_maps 每 tick |
| 挖掘权限 | player_action.rs:62 | can_mine（item_registry 非 Inventory 方法，回调内安全） |
| 统计读取 ×2 | player_action.rs:119/258 | item_id 折叠 |
| 长矛戳刺 | player_action.rs:317 | on_spear_jab 入参借用 |
| 双手空手探测 | use_item_on.rs:70 | held/off 各一 is_empty |
| 可写书判定 | edit_book.rs | is_writable_book |
| 精准采集 | ice.rs | has_silk_touch |
| 剪刀判定 | tripwire.rs | has_shears（get_item 指针比对） |
| 武器形态判定 | combat.rs:34 | is_mace + is_sword（同一回调） |
| 自定义名称 | combat.rs:319 | 死亡消息（名称克隆仅命中时） |
| AI 有趣物品 | beg.rs | 每 tick 双手探测 |
| AI 诱惑物品 | tempt.rs | 每 tick 双手探测 |
| 诱饵/海之眷顾 | fishing_bobber.rs:44/126 | 附魔等级 |
| 猪灵喜爱物品 | piglin_ai.rs | 玩家双手探测 |
| 全员持图探测 | world/map.rs | holds_map 全玩家双手 |
| 方块实体组件 | block/registry.rs:819 | apply_item_components 入参直接回调 |
| 战利品工具 | world/mod.rs:4901 | 非空才克隆 |
| 手别选择探测 | spear.rs:50 / map.rs（物品）/ spawn_egg.rs | id 比对选手，栈本体所有权另取 |

### 逐点语义核对（红线：行为/UX 零变化）

- 全部回调体只调用 ItemStack 自身方法（is_empty/item_count/
  get_item/get_enchantment_level/get_data_component/get_speed/
  translated_name 等）；`can_transfer_to`、`item_registry` 之外的
  Inventory/世界方法一律保持在回调外；
- `can_mine`/`on_spear_jab` 回调内调用 `server.item_registry`——
  非物品栏方法，无重入；
- spawn_egg 变体选择保持「主手匹配即用，否则**无条件**回退副手」
  的原选择序（双回调 + applied 标志）；
- 地图（物品）手别探测后，扣减/替换仍由 update_held 写锁内自带
  匹配校验原子完成，探测只省克隆不改写序；
- living.rs 损伤事件先行门：无监听时 cancelled 保持 false（与
  fire_blocking 无处理器短路同源），damage 路径逐字不变；
- 三处回退记录（转换后发现语义/安全偏差，立即回退并注释）：
  attack 摊销复用、fishing_bobber 恒 Some、enchant 并发快照。

## 双臂基准

`benchmark/src/bin/held_read_borrow.rs`（第 37 个 bin）。
「玩家持续活动」混合轮模型：模型物品栏（RwLock 主栏 + 选中槽，
选中持镐带效率 3 + 耐久 2 双组件）——挖掘路径 ×3（收获判定/
挖掘速度/效率等级）+ AI 探测 ×2（is_empty + id 比对）+ 统计
读取 ×1，每轮 6 读；每 4 轮附损伤事件门（旧臂无条件构造含栈
克隆）。

### 预登记门与结果（全部 PASS，EXIT=0）

| 门 | 判据 | 结果 |
|---|---|---|
| G1 | 两臂语义指纹一致 | PASS（0x7b19d3fe70424925） |
| G2 | 旧臂 ≥ 轮数×18+事件、新臂 == 0 | PASS（1 875 000 = 1 875 000；0） |
| G3 | 新臂耗时 ≤ 旧臂 | PASS（9.64 ms ≤ 64.68 ms） |
| G4 | 旧臂毛字节 > 新臂 | PASS（45.00 MB > 0） |

### 数据

| 段 | 旧臂 | 新臂 | 收益 |
|---|---|---|---|
| 混合轮 ×100 000 | 1 875 000 allocs / 45.00 MB / 64.68 ms | **0** / **0** B / 9.64 ms | 分配 −100%、耗时 **6.7×** |

## 负面发现与后续清单

- `add_one_item` RMW 缺失（轮 36 归档）与装备广播簇（协议层按值
  入参）仍挂账，另立轮次；
- 攻击路径（attack/kinetic_attack/shoot）的借用化受回调重入约束
  阻塞：攻击内部触及耐久/扣减/装备同步，需先有「快照-验证-提交」
  分离的攻击重构，收益另评估；
- piglin 系 MobEntity 异面 `off_hand_item()`（装备 HashMap 克隆）
  可另立 with_ 变体，但调用点仅 6 处且低频，优先级低；
- `ItemStack::EMPTY` 常量引用（`pub const EMPTY: &'static Self`）
  使空槽借用回调天然零成本，无需特判。

## 提交

- `perf(inventory)`：PlayerInventory 借用 API 三方法；
- `perf(entity)`：LivingEntity 借用 API 三方法 + living.rs 五点
  （含损伤事件先行门）；
- `perf(player)`：挖掘簇借用化（五点）；
- `perf(net)`：player_action/use_item_on 处理器借用化；
- `perf(ai)`：beg/tempt/piglin_ai AI 探测借用化 + combat 判定；
- `perf(item+world)`：crossbow/spear/spawn_egg/map/ice/tripwire/
  edit_book/fishing_bobber/registry/world 借用化；
- `perf(benchmark)`：`held_read_borrow` 双臂基准（第 37 bin）；
- `docs(note)`：本报告 + 索引；
- `chore(release)`：版本 0.3.48 dist。
