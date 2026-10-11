# 轮次 31 · 容器内容同步的比较-先行借用预检（未变化槽位零克隆）

## 背景：轮次 30 的残余读取克隆

轮次 30 把全屏扫描的每槽双份深克隆压到一份，但比较-先行仍缺一环：
`send_content_updates` 对每槽调 `get_cloned_stack()`（`Inventory::
get_stack` 按值返回，组件补丁深克隆）后才做 `are_equal`/`is_in_sync`
比较——未变化槽位（全屏扫描的绝大多数）依然为「比较」付整条
读取克隆链。消除它需要**借用读取**通道穿透到物品栏存储层。

## 可行性侦察（19 个实现方全量摸底）

`Inventory` trait 有 19 个实现方，锁结构高度模板化：

- 13 个 `RwLock<Vec<ItemStack>>` 直取（simple/crafting/ender_chest/
  gui + barrel 系 8 个方块实体 + 矿车容器）；
- 3 个单槽 `Mutex<ItemStack>`（beacon.payment/lectern.book/
  jukebox.record_stack）；
- 1 个递归组合（DoubleInventory 双箱）；
- PlayerInventory：main 区 `RwLock<[ItemStack; 36]>` 可借用，
  装备区分支 `EntityEquipment::get` 按值（回退克隆）；
- EquipmentSlotInventory：低频，走默认回退。

结论：可行，全部覆写为「锁内回调」模板。

## 改动

- `Inventory::with_stack(&self, slot, f: &mut dyn FnMut(&ItemStack))`：
  **dyn 兼容**的借用访问方法（首版 `impl FnOnce` 泛型使 trait
  失去 dyn 兼容性——`Arc<dyn Inventory>` 全仓在用，编译期即
  暴露；去泛型化为 `&mut dyn FnMut` + 闭包聚合后通过）。默认
  实现回退 `get_stack` 克隆后借出（功能等价，仅失去零克隆收益）；
  语义约束钉死在 rustdoc：回调内不得调用同一物品栏的任何方法
  （覆写方在持锁状态下回调，重入可能死锁）；
- 17 处覆写：13 个 Vec 型（`items.get(slot)`/`items[slot]` 引用
  回调，越界回退 EMPTY 与各自 `get_stack` 语义一致）、3 个单槽
  Mutex（锁内 `f(&stack)`）、DoubleInventory 递归透传；
  PlayerInventory 分支式（main 借用 + 装备回退）；
- `Slot::with_stack` 默认透传（`get_inventory().with_stack(
  get_index(), f)`）——cartography 的 3 处 `get_cloned_stack`
  覆写均为直通形态，默认透传语义等价，无需覆写；
- `send_content_updates`/`update_to_client` 比较-先行预检：
  `with_stack` 闭包内以引用与两份跟踪态比较，两者都一致则跳过
  该槽（零克隆、零分配）；否则回落完整路径（读取克隆 + 条件内
  克隆，与轮次 30 形态一致）。预检只覆盖 `received_stack` 存在
  且相等的常见路径；`received_hash` 命中或初态等罕见路径预检
  判否、由 `is_in_sync` 处理，语义与逐槽克隆版严格一致。

### 红线核对（行为/UX 零变化）

- 比较语义逐字未动（同一 `are_equal` 调用、同一操作数）；
- 变化槽位的完整路径与轮次 30 形态完全一致（读取克隆 + 条件
  内克隆，G1/G2 指纹全等实证）；
- 未变化槽位省掉的是「比较后必然丢弃」的读取克隆；
- 死锁边界：预检闭包只读 screen handler 自己的跟踪 Vec，不触
  任何物品栏方法；`RwLock` 读锁回调返回即释放；默认回退不持锁；
- 新增 2 项单测：`with_stack_borrows_current_content`（借用读取
  与 `get_stack` 内容一致、越界回退 EMPTY）；轮次 30 的行为
  不变式单测在预检路径下原样通过（稳态零重发/单槽变化恰重发）。

## 双臂基准（46 槽 × 每段 10 000 次扫描）

`benchmark/src/bin/screen_sync_borrow.rs`（第 31 个 bin）。旧臂
= 轮次 30 收编后形态（每槽读取克隆 + 引用比较），新臂 = 借用
预检（未变化零克隆、变化回落完整路径）。负载与轮次 30 相同
（46 槽：12 带补丁、26 裸物品、8 空）。

### 预登记门与结果（全部 PASS，EXIT=0）

| 门 | 判据 | 结果 |
|---|---|---|
| G1 | 稳态段触发序列指纹一致 | PASS（空序列固定折叠） |
| G3 | 稳态段新臂零分配 | PASS（0） |
| G4 | 稳态段旧臂分配 ≥ 带补丁槽数×扫描数 | PASS（600 000 ≥ 120 000） |
| G2 | 扰动段触发序列指纹一致 | PASS（0xd10f9ce4a8b76154，与轮次 30 记录同值） |
| G5 | 扰动段触发非空且旧臂分配 > 新臂 | PASS（792 429 > 214 972） |

### 数据

| 段 | 旧臂（轮 30 后） | 新臂 | 收益 |
|---|---|---|---|
| 稳态（零变化） | 600 000 allocs / 16.12 MB / 24.11 ms | **0** / **0** B / 3.71 ms | 分配 −100%、耗时 **6.49×** |
| 扰动（每拍一变） | 792 429 allocs / 22.01 MB / 30.77 ms | 214 972 / 6.24 MB / 10.16 ms | 分配 **3.69×**、字节 3.53×、耗时 3.03× |

扰动段新臂残余分配 = 两臂同付的变更成本（每拍一个新值构造）
+ 变化槽的完整路径克隆——结构性预期内。

## 负面发现与后续清单

- `EntityEquipment::get` 按值返回使 PlayerInventory 装备槽
  （4/玩家）走默认克隆回退——量级小，不为它再造一层 borrow；
- `EquipmentSlotInventory` 走默认回退（实体装备槽屏幕，低频）；
- 泛型 trait 方法与 dyn 兼容性的冲突（`impl FnOnce` 参数即破
  dyn）——后续 trait 扩展设计期就要用 `&mut dyn FnMut` 形态。

## 提交

- `perf(inventory)`：`Inventory::with_stack` 借用访问 + 17 覆写
  + Slot 透传 + 比较-先行预检 + 2 单测；
- `perf(benchmark)`：`screen_sync_borrow` 双臂基准（第 31 bin）；
- `docs(note)`：本报告 + 索引；
- `chore(release)`：版本 0.3.42 dist。
