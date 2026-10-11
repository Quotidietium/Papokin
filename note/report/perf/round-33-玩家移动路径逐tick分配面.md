# 轮次 33 · 玩家移动路径逐 tick 分配面（统计递增订阅者先行 + 跨方块群系指针去重）

## 背景：移动包处理路径的两处逐 tick 分配

移动包（`SPlayerPosition` / `SPlayerPositionRotation`）是服务器
最高频的入站流量面：每个移动玩家每秒约 20 包，全部经
`handle_position` / `handle_position_rotation` 处理。本轮对该
路径逐环节盘点（事件宏展开、统计递增、跨方块簿记、chunker、
附魔检查、位置广播）后收敛出两处纯浪费：

1. **统计递增的 `format!` 白付**：`Player::increment_stat` 在
   检查订阅者之前就查表取玩家（`get_player_by_uuid` 的 HashMap
   查找 + `Arc` 计数克隆）并 `format!("{category:?}:{stat}")`
   构造 `PlayerStatisticIncrementEvent.statistic_id`（`String`
   堆分配）——移动路径每包 `cm > 0` 都会递增行走统计（移动
   统计、跳跃、挖掘、合成等全部统计面共用此函数），无插件
   监听时 String 分配与查表全部白付。`fire_blocking` 内部虽有
   无处理器短路（`block_on` 之前），但救不回调用方已付的
   构造分配——与轮次 32 `EntityEquipmentChangedEvent` 同根
   通病；
2. **跨方块群系刷新的 Arc 包装浪费**：`Entity::set_pos` 每次
   跨方块边界都 `current_biome.store(Arc::new(biome))`——
   `get_rough_biome` 返回的是注册表 `&'static Biome`（出自
   `Biome::from_id` 的 const 静态表），把一个静态引用再包一层
   `Arc` 纯花一次堆分配（16 B）。`current_biome` 字段类型
   `ArcSwap<&'static Biome>` 本质是 `ArcSwapAny<Arc<&'static
   Biome>>`（arc-swap 1.9 无裸引用 `RefCnt` 实现，见「负面
   发现」）。实体在同群系内的连续跨界占绝对多数（平原散步
   每秒 ~4.3 次跨界、群系边界几十格一个），每次跨界都白包
   一个内容不变的 Arc。

### 侦察中确认干净的环节（本轮不动）

- `PlayerMoveEvent` 构造：字段全部 Copy + `Arc` 计数克隆，
  栈上构造零堆分配；`send_cancellable_blocking!` 展开的
  `fire` 无订阅者路径 `handlers_for` 早退；
- 位置广播：`broadcast_java_clients` 已是版本槽缓存（每版本
  序列化一次 + `Bytes::clone` 引用计数分发，先前轮次治理）；
- `chunker::update_position`：同区块驻留时
  `old_cylindrical == new_cylindrical` 早退，零分配；
- `check_location_enchantments`：`get_slot` 克隆为
  `ItemStack` 结构体拷贝 + `Option<Arc>` 计数（无堆分配）；
- `progress_motion` / `add_exhaustion`：纯计算。

## 改动

- `crates/papokin/src/entity/player.rs`
  `increment_stat`：订阅者先行——`server.plugin_manager.
  has_handlers::<PlayerStatisticIncrementEvent>()` 通过后才
  查表取玩家并构造事件（`format!` 只在有监听时付费）。判据
  与 `fire_blocking` 内部短路完全同源（同为
  `handlers.has_handlers(E::get_name_static())`），语义分支
  全覆盖核对：
  - 无 server / 有 server 无监听 / 有监听但玩家未注册 →
    旧路径事件即使构造也被短路丢弃、`final_amount ==
    amount`，新路径直接 `amount`——统计结果逐位一致；
  - 有监听且玩家在册 → 构造与派发逐字不变（含
    `cancelled` 短路返回与 `event.amount` 回读）。
- `crates/papokin/src/entity/mod.rs`：新增私有
  `refresh_current_biome(&self, world, block_pos)`——查询
  `get_rough_biome` 后**指针去重门**：`Biome::from_id` 为
  const 静态表匹配（同 id 恒同址，见下），群系未变时跳过
  `Arc::new` 只更新 `last_biome_update_pos`。`set_pos`
  跨方块分支与 `set_world`（换维度）统一改走该助手。

### 指针身份的可靠性论证

`Biome::from_id` 是 generated `const fn`，`match id` 逐 id
返回 `&Self::XXX` 关联常量——单一函数体内的单一匹配点，
同 id 两次调用必然命中同一提升静态地址；`level.rs` 的
`unwrap_or(&Biome::THE_VOID)` 兜底是另一调用点，最坏情形
（THE_VOID 两址不同）只多付一次 store，去重门是**优化过滤
器**而非语义开关：指针相等 ⟹ 同一对象 ⟹ 跳过安全，指针
不等 ⟹ 照旧 store，值语义（`current_biome` 恒反映最新
`block_pos` 的群系）无任何变化。读侧（`natural_spawner`
三处 `current_biome.load()`）经 Guard 自动解引用，零改动。

### 红线核对（行为/UX 零变化）

- 统计数值：无监听路径 `final_amount` 与旧路径逐位一致
  （基准 G1 指纹全等实证）；有监听路径构造与派发逐字不变；
- 事件可观测性：无监听时旧路径事件构造后即被丢弃，插件
  不可观测；门谓词与 `fire_blocking` 内部短路完全同源，
  不存在「门放行但 fire 短路」或反之外观；
- 群系语义：`current_biome` 恒等于最新方块位置的群系
  （基准 G4 逐步指纹全等实证）；`last_biome_update_pos`
  更新时序不变；
- 同步语义：`ArcSwap` 原语未动（仍是原子 swap），仅减少
  冗余 store 次数；
- 兼容性：全部为 crate 内部实现细节，无 API/协议/存档面。

## 双臂基准

`benchmark/src/bin/movement_path_alloc.rs`（第 33 个 bin）。
段 1 统计递增为无订阅者模型（`ModelStatEvent` 镜像事件的
`format!` String 分配面）；段 2 用**真** `ArcSwap<&'static
Biome>` + 真 `Biome::from_id` 注册表静态表（60 万移动包 +
30 万次跨方块，每 64 次跨界变更一次群系）。

### 预登记门与结果（全部 PASS，EXIT=0）

| 门 | 判据 | 结果 |
|---|---|---|
| G1 | 统计段两臂指纹一致（final_amount 语义等价） | PASS（0x9fd91ff85a33b725） |
| G2 | 统计段旧臂 ≥ 包数、新臂 == 0 | PASS（960 000 ≥ 600 000；0） |
| G3 | 统计段新臂耗时 ≤ 旧臂 | PASS（0.49 ms ≤ 47.78 ms） |
| G4 | 群系段两臂逐步指纹一致（去重不留陈旧群系） | PASS（0xa29fdeeece04925） |
| G5 | 群系段旧臂 ≥ 跨界数、新臂 ≤ 变更数+1 | PASS（300 001；4 687 ≤ 4 689） |
| G6 | 群系段新臂耗时 ≤ 旧臂 | PASS（6.01 ms ≤ 19.00 ms） |

### 数据

| 段 | 旧臂 | 新臂 | 收益 |
|---|---|---|---|
| 统计递增（无订阅者） | 960 000 allocs / 10.56 MB / 47.78 ms | **0** / **0** B / 0.49 ms | 分配 −100%、耗时 **97.6×** |
| 跨方块群系刷新 | 300 001 allocs / 7.20 MB / 19.00 ms | 4 687 / 0.11 MB / 6.01 ms | 分配 **64.0×**、字节 64.0×、耗时 **3.16×** |

- 统计段旧臂 960 000 = 600 000 包 ×1.6：`format!` 产出
  `String` 超出初始容量（如 `Crafted:72` 10 字节）时多一次
  容量增长分配；
- 群系段旧臂 300 001 的 +1 是 arc-swap 线程首次 `load()` 的
  hazard 指针槽初始化；新臂 4 687 = 30 万跨界中 4 687 次真实
  群系变更（每 64 次一个），残余为去重门放行的必要 store。

## 负面发现与后续清单

- **`ArcSwapAny<&'static T>` 不可行**：arc-swap 1.9.2 的
  `RefCnt` 仅实现 `Arc`/`Rc`/`Option<T>`/`Pin`/`Weak`，无
  裸 `&'static T`（无相关 feature）——字段级零分配存储需
  `AtomicPtr<Biome>`（自证 unsafe）或 `RwLock<&'static
  Biome>`（弃无锁读），本轮以指针去重门达成同量级收益
  （残余仅真实变更时 1 次 16 B 分配）；
- 同款「构造先于订阅者检查」通病在 `jump()`（PlayerJumpEvent
  前置 `get_player_by_uuid`）等低频调用点仍存在——跳跃起始
  才触发、量级小，未纳入本轮；后续可按热点审计逐点收编；
- `WasmPlugin.epoch_ticker` 纯 Drop 守卫字段触发 rustc
  `dead_code` 警告（干净 HEAD 亦然，先于本轮存在）——纯
  lint 噪声，Drop 语义正确，未动。

## 提交

- `perf(entity)`：跨方块群系刷新指针去重门；
- `perf(player)`：统计递增订阅者先行门；
- `perf(benchmark)`：`movement_path_alloc` 双臂基准（第 33 bin）；
- `docs(note)`：本报告 + 索引；
- `chore(release)`：版本 0.3.44 dist。
