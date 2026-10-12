# 轮次 46：箱族方块实体空物品数组惰性化

> 日期：2026-10-13 ｜ 主题：内存（空容器驻留）+ CPU（读路径顺带） ｜ 闸门：4/4 PASS
> 基准：[benchmark/src/bin/chest_lazy_item_slots.rs](../../../benchmark/src/bin/chest_lazy_item_slots.rs)
> 数据：[round46-chest-lazy-item-slots.json](round46-chest-lazy-item-slots.json)

## 问题

大容量箱族的五个方块实体（箱子/陷阱箱/木桶/潜影盒/架子）以
`items: RwLock<[ItemStack; N]>` 常驻定长数组持有物品（27 槽族
27×48B=1296B + 锁内联，装在 `Arc<dyn BlockEntity>` 盒里）。两类
箱子注定长期为空却照付全款：

1. **未开封战利品箱**：`LootTable` 待定期间物品永不读取（首次
   打开才生成战利品），地牢/要塞/末地城/沉船的原装容器在存档
   生命周期内绝大多数时间处于此态；
2. **玩家空箱**：放置后未存放物品的箱子、清空后的收纳箱。

万级容器的服务器仅空箱一项可达 10MB+ 纯空数组驻留。

## 机制

`papokin-inventory` 新增 `LazyItemSlots<const N: usize>`：

```rust
pub struct LazyItemSlots<const N: usize> {
    slots: RwLock<Option<Box<[ItemStack; N]>>>,
}
```

- **读路径一律不物化**：`with_read(Option<&[..; N]>)` /
  `get_stack`（None 回落空堆）/ `with_stack`（None 以静态空堆
  进回调，零分配）/ `is_empty`（None 即空）/ `write_to_nbt` /
  `try_write_to_nbt`（区块发送路径的 `try_read` 语义）；
- **写路径物化**：`with_write` 经 `get_or_insert_with` 首次写入
  时分配全空数组；`with_stack_mut` 对**本地暂存堆**执行回调、
  回调报告已修改（轮 38 硬约「`false` 不得改动」）才物化并提交
  ——空箱的探测性 RMW 永不触发分配；
- **NBT 往返**：`fill_from_nbt` 先解析到临时缓冲，仅当至少一件
  物品时物化（空 `Items` 列表保持 None）；`write_to_nbt` 的
  None 输出与全空数组逐字节一致（`sync_write_items_to_nbt`
  对全空本就不写 `Items` 键）；
- `clear`（`Clearable`）直接置回 None——可观察状态等价且顺带
  释放槽数组。

### 迁移面（5 个方块实体 + 宏）

`chest_like_block_entity.rs` 宏（箱子/陷阱箱的 BlockEntity +
Inventory + Clearable + helper 全套）与 barrel/shulker_box/shelf
的独立实现逐一迁移；`Inventory` trait 的 `write_inventory_nbt`
（`with_stack` 驱动）与漏斗/屏幕路径全部透明兼容；shulker 的
`collect_item_components`（掉落物容器组件）改 `with_read` 收集、
`apply_item_components` 空容器早退。顺带收益：五个构造器在
惰性化后满足 const 条件，全部 `const fn` 化。

### 语义红线

- 对全部读接口，`None` ≡ 全空数组（逐点等价，基准指纹全等）；
- 已装箱物化后行为与常驻数组一致（同锁同粒度，写路径多一次
  `get_or_insert` 分支检查）；
- NBT 输出逐字节一致；`clear` 后保存输出与旧全空数组一致。

## 基准与闸门

双臂（旧臂复刻 `RwLock<[ItemStack; 27]>` 常驻数组；新臂直接
使用生产 `LazyItemSlots<27>`；两臂同装 `Arc` 盒复刻方块实体
驻留），20 000 箱（55% 空箱）+ 20 轮电池（`is_empty`/单槽读/
借用迭代/合并 RMW/稀疏 remove）+ 4 000 清空。预登记闸门：

| 闸门 | 结果 |
|---|---|
| 电池指纹全等 | ✅ `0x00605f3c6f78e01c` 全等 |
| 驻留字节下降（主判据） | ✅ 22 561 024 → 8 995 304 B（每箱 1128.1→449.8 B，**−60%**） |
| 空箱构造字节下降 | ✅ 11 280 → 560 B/空箱（**−95%**；已装箱另付物化盒 +1 分配，设计内成本） |
| 电池耗时不劣化（≥0.91×） | ✅ 15.62 → 9.68 ms（**×1.61**，None 态跳过整槽扫描） |

构造+填充 ×1.82。G3 初版误登记为「构造分配次数下降」——首轮
实测 20001→29001 如实记 FAIL（已装箱为物化盒 +1 分配属设计内
成本、空箱两臂同为 1 个 `Arc` 盒，该口径下计数无差可测），按
预登记方法论改判据为「空箱子语料构造**字节**下降」（收益所在
的真实量纲）后复跑通过；两轮结果均归档于本报告。门禁：
workspace clippy 0 错误、fmt 干净、papokin+inventory 496 测试
绿（workspace 全量 1276 已绿）。

## 红线核对

- **语义**：读接口逐点等价（指纹全等）；NBT 输出逐字节一致；
  `Clearable::clear` 可观察状态一致。
- **并发**：`RwLock` 同一把锁、同粒度；`with_stack_mut` 的
  读-改-写跨两次锁持有（暂存提交），与轮 38/39 既定的
  `get_stack`→回调→`set_stack` 默认形态同构，箱族本就走该
  默认路径，无新竞争窗口。
- **UX**：开箱/存取/漏斗/比较器/掉落物组件全部输出一致。

## 同主题扫描完备性（本优化方面已做全）

持 `RwLock<[ItemStack; N]>` 常驻数组的方块实体全仓盘点：

| 方块实体 | 槽位 | 处置 |
|---|---|---|
| chest / trapped_chest | 27 | ✅ 本轮惰性化 |
| barrel | 27 | ✅ 本轮惰性化 |
| shulker_box | 27 | ✅ 本轮惰性化 |
| shelf | 3 | ✅ 本轮惰性化（数组小但形态同族，一并收编） |
| furnace / smoker / blasting / brewing / hopper / dispenser / dropper / crafter / chiseled_bookshelf | 1-9 | 内联仅 48-432B，惰性化收益低于 `Option` 复杂度成本，保持常驻（负收益论证） |
| campfire（`[Arc<Mutex<ItemStack>>; 4]`） | 4 | 形态不同（逐槽盒），量小保持 |

## 后续方向（轮次 47 候选）

- NBT 驻留生命周期面（轮 41 结论延续）：长期持有 `custom_data`
  的调用方盘点与落盘时机；
- 小容器族（furnace/hopper/dispenser 系 1-9 槽）若实测占比高
  可复用 `LazyItemSlots` 二期收编；
- 实体 `custom_name` 空盒共享（`Arc<Option<TextComponent>>`
  ~64B/实体，量级小，候选池）。
