# 轮次 36 · 容器读取借用化：`get_stack` 深克隆 → `with_stack`（99 个 clone 上下文点甄别·收编）

## 背景：在册候选的逐点甄别

轮次 34 归档的「99 个 clone 上下文点」在本轮完成全量甄别。以接收者
语境扫描全仓 `.clone()`（papokin 主 crate）：**145 个物品堆语境 +
629 个 Arc 计数语境 + 1203 个其他**。甄别结论：

| 类别 | 处置 |
|---|---|
| Arc 计数克隆（629） | 引用计数非堆分配，豁免 |
| 事件构造内克隆（轮 35 门内，约 15） | 已受订阅者先行门保护，仅监听时支付，豁免 |
| 所有权必须（写入/掉落/传递/NBT 落盘入参，约 50） | 克隆是语义必需，豁免 |
| 测试断言（recipe_helper 等，约 12） | 非生产路径，豁免 |
| **纯读白付（本轮收编）** | **13 个生产读取点** |
| `held_item`/`get_stack_in_hand` 调用面（69 点） | 低频交互路径为主，转下轮候选 |

关键事实：`ItemStack::clone` 深克隆 `patch: Vec<(DataComponent,
Option<Box<dyn DataComponentImpl>>)>`（1 次 Vec 分配 + 每组件一次
dyn 克隆分配）；**空 Vec 克隆零分配**——白付集中在非空带组件槽的
纯读扫描。`with_stack(slot, f)` 锁内回调借用变体已存在（前轮落位，
20 个容器实现覆写），本轮是把它推广到剩余按值读取点并改造 trait
默认方法自身。

## 改动（13 个生产读取点）

| 路径 | 文件 | 用途 | 白付 |
|---|---|---|---|
| trait 默认 `count()` | papokin-inventory/inventory.rs | 同物品计数（全背包） | 每非空槽 1+N |
| trait 默认 `contains_any_predicate()` | 同上 | 谓词扫描 | 同上 |
| trait 默认 `write_inventory_nbt()` | 同上 | NBT 落盘序列化 | 同上 |
| 漏斗吸入扫描 | hopper.rs | 空槽探测先行 | 空槽零分配，非空省克隆 |
| 漏斗吐出扫描 | hopper.rs | 空槽探测 + 事件 registry_key 仅监听时取 | 同上 |
| 漏斗目标满载判定 | hopper.rs | 纯读 item_count/max | 每非空槽 1+N |
| 矿车漏斗吸取 | minecart/hopper.rs | **双重克隆收敛** | 读取快照兼作回滚值，成功路径免备份克隆 |
| 比较器输出计算 | block/mod.rs | 红石热路径纯读 | 4×3 分配/次 |
| 战利品空槽过滤 | world/loot.rs | is_empty 探测 | 非空槽白付 |
| 书架比较器三槽 | blocks/shelf.rs | is_empty 探测 | 非空槽白付 |
| 合成器残留物探测 | redstone/crafter.rs | 回调内只读 item.id | 每非空槽 1+N |
| 配方匹配判定 ×2 | play/place_recipe.rs | 匹配/最小数量纯读 | 每占用格 1+N |

### 逐点语义核对（红线：行为/UX 零变化）

- `with_stack` 文档约束「回调内不得调用同一物品栏的任何方法」：
  全部回调体只调用 `ItemStack` 自身方法（is_empty/item_count/
  get_item/write_item_stack/match_item）；漏斗吸入与矿车吸取的
  `can_transfer_to`（Inventory 方法）保持在回调外；
- `contains_any_predicate` 保持逐槽短路（命中即返回 true）；
- `write_inventory_nbt` 的 `write_item_stack` 是 ItemStack 方法，
  不触物品栏；`Item`/`Slot` 键与空判断逐字不变；
- 漏斗吸入改为「空槽探测 → 非空槽再取一次」：探测与取出之间本就
  存在并发窗口（取出-回滚注释明示），探测只是把空槽的整堆克隆
  换成借用读，非空路径的判定/取出/回滚序不变；
- 矿车漏斗收敛双重克隆：原 `get_stack`（克隆一）仅供判定 +
  `backup = stack.clone()`（克隆二）eager 备份；新形以读取快照
  兼作失败回滚值（`stack` 在 remove 后未被触碰，内容与原 backup
  逐位一致），成功路径（常态）零备份克隆，失败路径回滚等价；
- trait 默认方法对未覆写 `with_stack` 的容器回退为 `get_stack`
  克隆（功能等价仅失去收益），20 个主要实现均已覆写。

## 双臂基准

`benchmark/src/bin/container_read_borrow.rs`（第 36 个 bin）。
「容器运转」混合轮模型：比较器 9 槽（4 非空各带 2 组件 + 5 空）+
漏斗 5 槽（2 非空）+ 上方箱子 27 槽（3 非空）+ `count()` 36 槽
（8 非空），每轮 51 个白付分配。

### 预登记门与结果（全部 PASS，EXIT=0）

| 门 | 判据 | 结果 |
|---|---|---|
| G1 | 两臂语义指纹一致 | PASS（0x4f8324754de7af25） |
| G2 | 旧臂 ≥ 轮数×51、新臂 == 0 | PASS（5 100 000 = 5 100 000；0） |
| G3 | 新臂耗时 ≤ 旧臂 | PASS（2.64 ms ≤ 138.69 ms） |
| G4 | 旧臂毛字节 > 新臂 | PASS（107.10 MB > 0） |

### 数据

| 段 | 旧臂 | 新臂 | 收益 |
|---|---|---|---|
| 混合轮 ×100 000 | 5 100 000 allocs / 107.10 MB / 138.69 ms | **0** / **0** B / 2.64 ms | 分配 −100%、耗时 **52.5×** |

## 负面发现与后续清单

- `held_item`/`get_stack_in_hand` 的 69 个调用点未逐点甄别（低频
  交互路径为主，读改写混合）——转下轮候选；
- `add_one_item` 的克隆-改写-`set_stack` 是锁稳定的读改写 API
  缺失所致（与轮 34 注释的并发窗口同源），改造需新增 RMW trait
  方法，另立轮次评估；
- 装备广播簇（living.rs 2644-2687，`send_equipment_changes` 取
  所有权入参）需协议层 API 借用化配合，另立轮次；
- `Arc` 计数克隆（629 点）非堆分配，永无收益，关闭该子项。

## 提交

- `perf(inventory)`：trait 默认方法借用化（3 处）；
- `perf(hopper)`：漏斗/矿车漏斗扫描与双重克隆收敛（4 处）；
- `perf(block)`：比较器/书架/合成器读取借用化（3 处）；
- `perf(net+world)`：配方放置/战利品过滤借用化（3 处）；
- `perf(benchmark)`：`container_read_borrow` 双臂基准（第 36 bin）；
- `docs(note)`：本报告 + 索引；
- `chore(release)`：版本 0.3.47 dist。
