# 轮次 47：实体 custom_name 空盒共享单例

**日期**：2026-10-13 · **主题**：内存占用（实体构造残余堆分配收口） · **结论**：正向优化，已收编

## 动机与侦察

轮次 44 消除了 spawn 路径三大堆包装（群系盒 / 移动控制盒 / 导航盒）后，对 `Entity` 构造器剩余的逐实体分配点做终局盘点，确认 `custom_name: ArcSwap<Option<TextComponent>>`（`crates/papokin/src/entity/mod.rs:1019`）是最后一个高频空态付费点：

- 构造器原为 `ArcSwap::new(Arc::new(None))`——每个实体在构造时分配一个仅装 `None` 的堆盒。`Option<TextComponent>` 实测 40 字节，加上 malloc 头与规格取整后每盒净驻留约 50 字节；
- 绝大多数实体（野生生物、掉落物、弹射物……）终生未命名，这笔分配是纯粹的空态常税；
- `ArcSwap` 字段本身 8 字节内联存 `Arc` 指针，不占堆；堆成本全部来自这个空盒；
- 读点三处（`get_name` / `get_display_name` / 元数据序列化）与写点一处（`set_custom_name`）都不区分盒的来源，共享单例在语义上不可区分。

这与轮次 44 的 `shared_biome_arc`（静态地址键缓存）是同一模式：**空态 / 常态的 `Arc` 载体进程级共享，真实分化时才独立分配**。

## 改动

`crates/papokin/src/entity/mod.rs`：

- 新增模块级 `shared_none_name_arc() -> Arc<Option<TextComponent>>`：`LazyLock` 持有全进程唯一空盒，构造器每次 `clone()`（仅原子引用计数 +1，零分配）；
- 构造器 `custom_name: ArcSwap::new(shared_none_name_arc())`；
- `set_custom_name` 写路径不变（仍 `Arc::new(Some(name))` 独立分配），命名实体与旧实现逐位一致。

```rust
fn shared_none_name_arc() -> Arc<Option<TextComponent>> {
    static NONE_NAME: std::sync::LazyLock<Arc<Option<TextComponent>>> =
        std::sync::LazyLock::new(|| Arc::new(None));
    NONE_NAME.clone()
}
```

## 基准（`benchmark/src/bin/custom_name_shared_none.rs`）

双臂同构 `Entity { custom_name: ArcSwap<Option<TextComponent>> }`，仅构造函数不同（旧臂逐实体 `Arc::new(None)`，新臂共享单例），共用同一电池脚本消除双臂代码漂移。语料 20000 实体：未命名读电池 20 轮（`load` + `is_some`，与元数据序列化同路径）→ 10% 实体命名写 → 混合读电池 5 轮（命名实体 `load` + `as_ref().clone()` + `get_text` 折叠文本字节）。

| 闸门 | 结果 |
| --- | --- |
| 读写电池指纹全等 | √ `0x27bf6a32d37d6da9` 双臂一致 |
| 驻留字节下降（主判据） | √ 1781152 → 773208 B（净差 1007944 B ≈ 每实体 50.4 B） |
| 构造分配次数下降 | √ 20001 → 2 次（每实体 1.000 → 0.000，新臂仅剩 Vec 缓冲 + 首访单例） |
| 命名写路径不回归（等分配） | √ 8000 → 8000 次（写路径双臂同码） |
| 读耗时不劣化（≥0.91×） | √ 4.45 → 3.87 ms（×1.149，共享单例的缓存局部性反而略优） |

构造耗时 0.80 → 0.04 ms（×18.74）。原始数据见 `round47-custom-name-shared-none.json`。

## 语义与风险

- `Arc` 共享对读方完全透明：`load()` 拿到的 `Guard` 只解引用与克隆内容，从不观察引用计数或指针身份；
- 命名写整体替换 `Arc`（`ArcSwap::store`），从不原地改共享盒，不存在单例被污染的路径；解除命名若未来引入也只会 `store` 回共享单例；
- 单例生命周期为进程级静态，与实体集合的装卸无关，无泄漏面。

## 附带清理（style 提交）

轮 45/46 收尾在库内留下了一批已提交的未用导入与一个死绑定，本轮门禁时全部清除：

- 箱族六文件（barrel / chest / trapped_chest / chest_like_block_entity / shulker_box / shelf）的 `RwLock`、`array::from_fn`、`sync_write_items_to_nbt` 残留导入；
- `entity/living.rs` 的 `DEFAULT_EQUIPMENT_DROP_CHANCE` 与 `entity/mob/mod.rs` 的 `MoveControl`（轮 44/45 迁移后失效）；
- `chest_like_block_entity.rs` 的 `chunk_data_nbt`：`has_loot_table` 绑定原本算完即弃、条件处二次加锁，改为直接使用绑定（单次加锁，语义等价）。

## 后续

实体构造路径至此抵达结构性下限：剩余逐实体分配仅为 `MobEntity` 装箱（多态必需）与 equipment 的 `Arc`（玩家物品栏真实共享，轮 45 决策保留）。下一候选见轮 46 报告挂账（NBT custom_data 生命周期 / 小容器族二期）。
