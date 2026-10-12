# 轮次 48：方块实体与投射物冗余 Arc 盒去化

**日期**：2026-10-13 · **主题**：内存占用（`Arc<Mutex<T>>` 单持有盒终局审计） · **结论**：正向优化，已收编

## 动机与侦察

轮次 47 收口实体构造路径后，把审计面转向「单持有者却包 `Arc` 的锁字段」：`Arc<Mutex<T>>` 只有当外部真会克隆 `Arc`（跨 `await` 持有、 handing 给任务/插件）时才有意义；若全仓没有任何 `.clone()` 克隆该 `Arc`，引用计数盒（强/弱计数 16B + malloc 头 + 取整）就是纯浪费——与轮次 45 `equipment_drop_chances` 去 `Arc` 盒同一模式的终局扫描。

全仓 `Arc<Mutex<..>>` 字段审计（排除 `Arc<dyn Trait>` 与真实共享点）后确认七个零克隆点：

| 字段 | 形态 | 每实例盒数 |
| --- | --- | --- |
| 告示牌 `SignText.messages` / `filtered_messages`（双面） | `Arc<Mutex<[Box<str>; 4]>>` ×4 | 4 |
| 告示牌 / 悬挂牌 `currently_editing_player` | `Arc<Mutex<Option<Uuid>>>` | 1 |
| 信标 `payment` | `Arc<Mutex<ItemStack>>` | 1 |
| 唱片机 `record_stack` | `Arc<Mutex<ItemStack>>` | 1 |
| 讲台 `book` | `Arc<Mutex<ItemStack>>` | 1 |
| 三叉戟 `item_stack` | `Arc<Mutex<ItemStack>>` | 1 |

保留不动的真实共享点：`player_inventory.entity_equipment`（轮 45 决策——玩家物品栏真共享）、`screen_handler.cursor_stack`、`crafting.result`（开屏会话持有）、`player.chat_session` / `advancements`（会话/统计侧共享）。

## 改动

七个字段统一降为裸 `Mutex<T>`：锁语义、中毒恢复、所有 `.lock()` 访问点完全不变；`ItemStack`（40B）与 `[Box<str>; 4]`（64B）内联进实体本体（方块实体本体本就在一个 `Arc<..>` 分配里）。涉及 `block/entities/{sign,hanging_sign,beacon,jukebox,lectern}.rs`、`entity/projectile/trident.rs` 与 `block/blocks/signs.rs` 两个辅助函数签名：

- `SignText::clone` 原共享 `Arc` 快照，改深拷贝双数组——全仓无调用点（读路径 `get_messages` 本就逐行克隆快照），语义不变；
- `currently_editing_player()` 访问器返回 `&Mutex<Option<Uuid>>`，`open_text_edit` / `other_player_is_editing_sign` 参数同步；
- 去盒后各文件冗余 `Arc` 导入清除。

## 基准（`benchmark/src/bin/redundant_arc_unwrap.rs`）

双臂同构语料（旧臂复刻 `Arc<Mutex<..>>` 字段形态、新臂裸 `Mutex`），共用电池脚本：20000 空白牌 + 8000 物品持有者构造 → 读电池 20 轮（正面第 2 行 / 背面过滤第 3 行折叠、编辑者空性、物品计数）→ 10% 牌单行写（双臂同码写路径）→ 写后全量回读。

| 闸门 | 结果 |
| --- | --- |
| 读写电池指纹全等 | √ `0x21f5526dd864b1ed` 双臂一致 |
| 驻留字节下降（主判据） | √ 10058802 → 7466802 B（净差 2592000 B ≈ 2.47 MB，每实例均摊 359.2 → 266.7 B） |
| 构造分配次数下降 | √ 108002 → 2 次（旧态 = 100000 牌盒 + 8000 持有者盒 + 2 缓冲；新态仅剩 Vec 缓冲） |
| 写路径不回归（等分配） | √ 6000 → 6000 次 |
| 读耗时不劣化（≥0.91×） | √ 8.25 → 6.73 ms（×1.225，去引用计数间接层反而略优） |

构造耗时 5.52 → 0.96 ms（×5.75）。原始数据见 `round48-redundant-arc-unwrap.json`。

## 语义与风险

- `Arc` 的唯一语义贡献是「克隆共享」；七个字段全仓零克隆点（编译器兜底验证：去 `Arc` 后任何残留 `.clone()` 克隆点都会因 `Mutex` 不实现 `Clone` 而编译失败），故去盒不改变任何可观察行为；
- 中毒路径（`unwrap_or_else(PoisonError::into_inner)`）逐点保留；
- 空白 `Box<str>`（`Box::from("")`）实测零堆分配，`empty_messages()` 不产生分配噪音。

## 后续

`Arc<Mutex<T>>` 单持有维度至此收口（保留项均有真实共享语义）。方块实体/实体构造侧的已知优化面已穷尽：轮 46 惰性化（负论证保护小容器族）、轮 47 空盒共享、本轮去冗余盒。下一轮候选转向挂账项：NBT `custom_data` 生命周期盘点（高风险，需先做存活期画像）或运行时驻留画像驱动的全新方向。
