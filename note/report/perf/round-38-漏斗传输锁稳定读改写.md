# 轮次 38 · 漏斗传输锁稳定读改写：`add_one_item` 两段锁克隆 → `with_stack_mut` 原位 RMW

## 背景：在册候选的落地

轮次 34 在漏斗取出路径注释的并发窗口（「克隆后盲写回」在取出与
写回之间可被并发改动覆盖）、轮次 36 归档的「`add_one_item` 的
克隆-改写-`set_stack` 是锁稳定读改写 API 缺失所致」在本轮收口。

原形态：`add_one_item` 对每个候选槽 `get_stack`（整栈克隆：1
Vec + 每组件一次 dyn 克隆）→ 改写 → `set_stack` 回写（再次
取写锁）。三重代价：**满载扫描对每个非空槽白付克隆后丢弃**、
合并路径付整栈克隆、读与写两段锁之间存在并发替换窗口（轮 34
同源）。调用面 9 处：漏斗吸入/吐出/自喂 ×4、合成器弹出、
投掷器 ×2、矿车漏斗 ×2。

## 改动

### 新 API：`Inventory::with_stack_mut`

```rust
fn with_stack_mut(&self, slot: usize, f: &mut dyn FnMut(&mut ItemStack) -> bool)
```

- 与 `set_stack` 等同的写锁内原位改写；回调返回是否已修改，
  实现按各自 `set_stack` 的副作用提交（含 `mark_dirty`）；
- 回调约定：返回 `false` 时**不得改动物品堆**（覆写实现为
  原位修改无法回滚）；回调内不得调用同一物品栏的任何方法
  （与 `with_stack` 重入约束同源）；
- 默认回退 `get_stack` + 条件 `set_stack`（语义等价，仅失去
  锁稳定与省克隆收益）——与轮 36 `with_stack` 推广策略同构。

### 19 个实现全量覆写

| 实现族 | 形态 | 复刻的 set_stack 副作用 |
|---|---|---|
| 漏斗/木桶/潜影盒/投掷器/发射器/书架(凿纹)/置物架/合成器 | 写锁 + `items[slot]` 原位 | `mark_dirty`（合成器另含禁用槽前置检查） |
| 矿车容器/末影箱 | 写锁原位 | 无 |
| 信标/唱片机/讲台 | `Mutex` 单槽 | dirty（讲台另含回第一页） |
| SimpleInventory/CraftingInventory/插件 GUI | 写锁 + 越界容忍 | 无 |
| DoubleInventory | 按槽分派两侧（各自覆写保持生效） | — |
| PlayerInventory | 主栏原位；装备槽守卫内 get→f→条件 put | 无 |
| EquipmentSlotInventory | 守卫内 get→f→条件 put | put 后 `notify`（装备同步回调） |

### 调用方迁移（3 处）

| 路径 | 文件 | 收益 |
|---|---|---|
| `add_one_item` 逐槽传输 | hopper.rs | 合并零克隆、满载扫描零克隆、空槽填入仅付一次 item 克隆（两臂同付）；关闭两段锁窗口 |
| 合成器弹出后逐槽扣减 | crafter.rs（方块） | 非空槽原位 -1 免克隆，空槽零分配 |
| `/loot` 容器分发循环 | loot.rs | 空槽 `mem::replace` 整栈移入零克隆、合并原位加 |

### 逐点语义核对（红线：行为/UX 零变化）

- `add_one_item`：槽迭代序、`is_valid_slot_for` 前置（回调外）、
  满栈上限 `get_max_stack_size`、组件相等判定
  `are_items_and_components_equal`、漏斗冷却原子序（7/8 写入
  条件）、`mark_dirty` 触发次数（覆写一次 + 显式一次，与原
  set_stack+显式双触发一致）、返回值逐字不变；
- 满载失败路径：原「克隆后判不可合并、无写入」→ 新「借用判读、
  回调返回 false 无提交」——无 `mark_dirty` 误触发（覆写以
  回调返回值为门，与原 set_stack 不被调用等价）；
- loot 分发：空槽整栈移入以 `mem::replace(&mut stack, EMPTY)`
  表达（`ItemStack` 无 `Default`）；合并仅比对物品 id（原样，
  不收紧为组件相等）；首空槽填入即 break 的顺序保持；
- 副作用复刻核对：合成器禁用槽前置检查在取写锁前（与原
  `set_stack` 同序）；讲台改写后回第一页；装备槽视图 put 后
  `notify`；
- 默认回退保证未覆写的未来实现语义不变（changed 才写回）。

## 双臂基准

`benchmark/src/bin/hopper_transfer_rmw.rs`（第 38 个 bin）。
「自动物品传输」混合轮模型：每轮 1 次成功合并（目标槽非空同物
未满）+ 每 8 轮满载失败扫描（5 槽逐槽克隆后丢弃）+ 合成器
扣减（3 非空槽克隆后 -1）+ 每 16 轮填入槽单槽传输（add_one_item
单槽语义：空则移入、非空同物未满则 +1）；传输物品预构建共享
（构造分配不进差异面）。

### 预登记门与结果（全部 PASS，EXIT=0）

| 门 | 判据 | 结果 |
|---|---|---|
| G1 | 两臂语义指纹一致 | PASS（0xb61398bdc8eb1c25） |
| G2 | 旧臂 ≥ 618 750、新臂 == 3（仅空槽首填） | PASS（618 750 = 618 750；3） |
| G3 | 新臂耗时 ≤ 旧臂 | PASS（0.85 ms ≤ 20.57 ms） |
| G4 | 旧臂毛字节 > 新臂 | PASS（14.55 MB > 72 B） |

### 数据

| 段 | 旧臂 | 新臂 | 收益 |
|---|---|---|---|
| 混合轮 ×100 000 | 618 750 allocs / 14.55 MB / 20.57 ms | **3** / **72** B / 0.85 ms | 分配 −99.9995%、耗时 **23.5×** |

## 负面发现与后续清单

- 屏幕处理器事务引擎（screen_handler.rs 移动/合并/shift-click
  核心路径，:796/:1175/:1192 等）与商人/铁砧/织布机/锻造台等
  输入槽逻辑仍是「克隆-改写-set_stack」形态——玩家点击驱动
  （低频、跨槽事务带回滚语义），整仓迁移需单独立轮评估回滚
  语义在原位改写下如何表达；
- 漏斗吸入路径的 `can_transfer_to` 判定克隆（hopper.rs:244）受
  回调重入约束（`can_transfer_to` 是 Inventory 方法，必须在
  回调外调用）保持现状——要消除需 RMW API 支持锁内有效性
  判定（谓词入参化），收益与复杂度另评估；
- 漏斗吐出路径的事件构造（hopper.rs:464）已受订阅者先行门
  保护（轮 35/36），常态零分配，无需再动。

## 提交

- `perf(inventory)`：`with_stack_mut` trait 方法 + 默认回退 +
  papokin-inventory 七实现覆写；
- `perf(block)`：方块实体容器八实现覆写（漏斗/木桶/潜影盒/
  投掷器/发射器/书架/置物架/合成器）+ 矿车容器 + 信标/唱片机/
  讲台 + 插件 GUI；
- `perf(hopper)`：`add_one_item` 迁移锁稳定 RMW；
- `perf(crafter+loot)`：弹出扣减与容器分发循环迁移；
- `perf(benchmark)`：`hopper_transfer_rmw` 双臂基准（第 38 bin）；
- `docs(note)`：本报告 + 索引；
- `chore(release)`：版本 0.3.49 dist。
