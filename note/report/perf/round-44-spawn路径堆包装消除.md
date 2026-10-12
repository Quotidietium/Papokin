# 轮次 44：实体 spawn 路径堆包装消除（群系 Arc / 控制盒 / 导航盒）

> 日期：2026-10-12 ｜ 主题：内存（spawn 路径逐实体堆分配清零）+ CPU（构造与刻路径顺带） ｜ 闸门：4/4 PASS
> 基准：[benchmark/src/bin/spawn_path_heap_wrappers.rs](../../../benchmark/src/bin/spawn_path_heap_wrappers.rs)
> 数据：[round44-spawn-path-heap-wrappers.json](round44-spawn-path-heap-wrappers.json)

## 问题

实体构造（`Entity::from_uuid_with_id` → `Mob` → 导航装配）路径上有三个
**逐实体必然发生的堆分配**，全部只是"包装"而非实体自身数据：

1. **群系 `Arc` 包装**：`current_biome` 字段类型为 `ArcSwap<&'static Biome>`
   （ArcSwap 要求 `Arc` 载体，裸引用无 `RefCnt`，轮次 33 结论），
   构造与换群系时各做一次 `Arc::new(静态引用)`——堆上只放一个 8B
   指针，加上分配器头部与规格取整，**每次跨界都为同一个静态地址买单**；
2. **移动控制盒**：`move_control: Mutex<Box<dyn MoveControlTrait>>`——
   绝大多数生物装的是地面版 `MoveControl`（约百字节），却一律
   `Box::new` 进堆再挂虚表；
3. **导航盒**：`Navigator` 内部 `Box<dyn PathNavigationTrait>`——
   地面版 `GroundPathNavigation` 携路径状态与坐标缓存近 1KB，同样
   逐实体装箱。

三项合计每实体 3 次堆分配 + 分配器逐块开销与规格取整浪费；万级
生物服务器仅此即数万次纯包装分配。

## 机制

### 1. 群系 `Arc` 盒共享缓存（`entity/mod.rs`）

```rust
fn shared_biome_arc(biome: &'static Biome) -> Arc<&'static Biome> {
    static BIOME_ARCS: OnceLock<Mutex<FxHashMap<usize, Arc<&'static Biome>>>> = …;
    // 以静态引用的地址为键，or_insert_with 懒建后 clone
}
```

构造与 `refresh_current_biome` 的换群系分支改走缓存：同一群系恒同址
（`Biome::from_id` 的 const 静态表），**全服每种群系最多一个 `Arc` 盒**。
字段类型 `ArcSwap<&'static Biome>` 不变，load/store 路径零改动；
clippy 的 `redundant_allocation` 建议不适用（ArcSwap 硬性要求）。

### 2. `AnyMoveControl` 混合枚举（`ai/control/mod.rs`）

```rust
enum AnyMoveControl {
    Ground(MoveControl),                 // 热门变体内联
    Slime(SlimeMoveControl),             // 专属变体内联
    Other(Box<dyn MoveControlTrait>),    // 其余保持盒装
}
```

`Mutex<Box<dyn …>>` → `Mutex<AnyMoveControl>`；`ground_default()`
常量助手供构造，史莱姆族构造改写 `Slime` 变体，其余走 `Other`。
`impl Control` 与 `impl MoveControlTrait` 双 trait 全方法转发，
调用面零改动。内联变体令 clippy `large_enum_variant` 触发——刻意
为之（总字节数不变、少一次堆分配），就地 `allow` 并注明理由。

### 3. `NavigatorInner` 混合枚举 + Deref 单点分发（`ai/pathfinder/mod.rs`）

```rust
enum NavigatorInner {
    Ground(GroundPathNavigation),        // ~928B 直内联
    Custom(Box<dyn PathNavigationTrait>),
}
```

`Deref/DerefMut` 以 `dyn PathNavigationTrait` 为目标做**单点**
match 分发：`Ground` 臂静态分发、`Custom` 臂盒装转发。相比逐方法
委托（该 trait 面很宽），单点分发把 ~34 个方法的适配压成一个
match，新增 trait 方法也只改一处。

### 迁移面

- 三处 goal 文件（`blaze_attack`/`bow_attack`/`try_find_water`）补
  `use …MoveControlTrait`：`dyn` 主方法免导入即可调，**枚举分发要求
  trait 在作用域内**（本次唯一的调用方改动）；
- `mob/mod.rs` 字段与构造、`slime.rs` 装配、`living.rs` 无涉。

## 基准与闸门

双臂结构复刻（`GroundControl` ~100B / `GroundNav` 928B 含
`[u64; 96]` 体量的复刻体，保证臂间尺寸真实），20000 实体构造 +
20 轮使用电池（控制 wanted/tick + 导航 tick/move_to + 群系 id 折叠
指纹）+ 2 轮群系切换（10% 实体换目标群系）。预登记闸门：

| 闸门 | 结果 |
|---|---|
| 使用电池指纹全等 | ✅ `0x3c00547975915877` 全等 |
| 驻留字节下降（主判据） | ✅ 22 241 024 → 21 281 320 B（每实体 1112.1→1064.1 B，**−48 B/实体**） |
| 构造分配次数下降 | ✅ 60 001 → 9 次（每实体 3.000→0.0005） |
| 使用耗时不劣化（≥0.91×） | ✅ 7.84 → 6.18 ms（**×1.27**，内联免指针追逐） |

构造 7.60→3.21 ms（×2.37）；群系切换分配 4 000 → **0** 次
（旧臂每实体每次换群系都 `Arc::new`，新臂命中缓存）。

驻留净节省 48B/实机的构成：三项堆负载与分配器逐块开销
（头部 + 规格取整）合计略大于枚举内联增长（控制 16B 胖指针→
112B、导航 16B→928B、`Arc` 8B 不变），净差即分配器浪费部分；
**交换本质是"堆块+开销"换"紧凑内联"，分配次数与局部性是更大
的赢家**。门禁：workspace clippy 0 错误、papokin lib 439 测试全过。

## 红线核对

- **语义**：三臂行为逐字节一致（指纹全等）；`Other`/`Custom` 兜底
  变体保证非地面实现零回归——功能集不变，只是分发形态改变。
- **并发**：群系缓存 `OnceLock<Mutex<…>>` 初始化与访问线程安全，
  值为不可变 `Arc`；控制/导航字段仍由原 `Mutex` 保护，锁粒度不变。
- **UX**：移动/寻路/群系感知输出完全一致，无任何玩家可见差异。

## 同主题扫描完备性（本优化方面已做全）

spawn 路径逐实体堆分配盘点（接轮次 42/43 已覆盖属性表与装备槽）：

| 堆分配点 | 处置 |
|---|---|
| 群系 `Arc` 包装 | ✅ 本轮共享缓存 |
| 移动控制 `Box<dyn>` | ✅ 本轮混合枚举 |
| 导航 `Box<dyn>` | ✅ 本轮混合枚举 + Deref 分发 |
| 属性表 hashbrown | ✅ 轮次 42 覆盖层 |
| 装备槽映射重建 | ✅ 轮次 43 共享单实例 |
| `entity_equipment`/`Brain`/`CombatTracker` 空默认 | 零堆或惰性，无需动 |
| 跳跃控制（`JumpControl`） | 内联单元，无盒 |

## 后续方向（轮次 45 候选）

- `EntityEquipment` 的 `HashMap<EquipmentSlot, ItemStack>` 数组化
  （6 槽定域，仅利于已装备实体）；
- NBT 驻留生命周期面（轮 41 结论延续）：长期持有 `custom_data`
  的调用方盘点与落盘时机；
- 世界层区块/方块实体驻留扫描。
