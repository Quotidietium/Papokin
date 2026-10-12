# 轮次 42：实体属性表写时物化覆盖层（HashMap 全量抄写 → AttributeMap）

> 日期：2026-10-12 ｜ 主题：内存（每生物实体常驻属性表）+ CPU（顺带大幅） ｜ 闸门：4/4 PASS
> 基准：[benchmark/src/bin/attribute_overlay.rs](../../../benchmark/src/bin/attribute_overlay.rs)
> 数据：[round42-attribute-overlay.json](round42-attribute-overlay.json)

## 问题

`LivingEntity.attributes: RwLock<FxHashMap<u8, AttributeInstance>>` 在构造时把
实体类型声明的**全部**属性逐条插入哈希表。生成数据显示 93 个可生物实体
类型全部声明 26-36 条属性（僵尸 29、玩家 36，均值约 28）——每个实体
一落地就抄写一份：

- `AttributeInstance`（f64 + Vec + AtomicU64 + AtomicBool）48B，哈希桶
  条目 56B；28 条属性终容量 56 桶 ≈ **3.2KB/实体**驻留；
- 构造路径 5 次增长分配（容量梯 3→7→14→28→56）；
- 而其中绝大多数条目终生保持「基础值 = 类型表值、修饰符为空」的
  初生态——这份表本质是 `'static` 类型表的逐实体抄写。

1000 个生物约 3.2MB、万级实体农场约 32MB，全是重复数据。

## 机制

新承载 `AttributeMap`（`crates/papokin/src/entity/attribute_map.rs`）：
**写时物化覆盖层**，典型生物零堆分配。

1. **三态读取视图 `AttributeView`**：`Live(&AttributeInstance)`（已分叉）/
   `Declared { base }`（类型表声明且未分叉，值=基础值、修饰符空——与
   初生实例逐点等价）/ `Absent`（未声明、动态移除或从未添加）。各调用
   方的回落链保持原位：值读取缺失→全局默认、基础值缺失→类型表→
   默认、命令门禁→`contains_key` 语义由 `is_present` 承接。
2. **每类型直索引基础值表 `TypeBases`**：`Box<[f64]>` 以属性 id 为下标、
   NaN 为「未声明」哨兵（生成表基础值全为有限字面量，`debug_assert`
   兜底），懒构建、跨实体共享 `Arc`（93 类型合计约 30KB）。`Declared`
   回退由「类型表线性扫描 28 步」降为一次下标访问——这是热读反超的
   关键（初版线性扫描实测热读 ×0.82 未过闸门，直索引后 ×4.37）。
3. **写路径语义对齐**（红线清单逐点复刻）：
   - `get_mut`：覆盖层命中→直返；声明未分叉→物化基础值取类型表的
     实例（旧 map 构造即建实例，`get_mut` 必命中）；墓碑/未声明→
     `None`；
   - `get_or_insert_with`（旧 `entry().or_insert_with`）：复活墓碑；
   - `remove`（wasm reset_attribute）：删条目 + 对声明属性压墓碑——
     之后值读取回落**全局默认**（旧 map 删除实例后的既有路径，
     与基础值回落的类型表值不对称，按原样保留）；
   - `insert`（旧 `HashMap::insert` 后者胜）：原位替换或按 id 序插入。
4. **构造零抄写**：`AttributeMap::new` 只做直索引表共享克隆；初始
   生命值从类型表线性取一次。87001→15 次构造分配（余量为 6 类型
   的懒表构建）。
5. **合并视图 `for_each_present`**：full_sync 迭代「类型表声明条目
   （覆盖层命中给 Live、墓碑跳过）+ 覆盖层动态添加条目」，输出仍按
   属性 id 排序（该函数本就 sort，迭代序无观察面）。
6. **调用面迁移**（约 20 处）：`LivingEntity` 六个属性方法（update×3/
   get_value/get_base/set_base）+ 武器属性更新 + 属性同步双函数 +
   `/attribute` 命令六处 + wasm 宿主两处 + 生物初始化六文件
   （hoglin/creaking/giant/magma_cube/slime/zoglin）+ 三叉戟已施加
   探测。

### 有意修正的一处旧怪癖

旧 `update_weapon_attributes` 的 `or_insert_with(|| new(default_value))`
闭包在旧承载下从不运行（构造已建实例）属死代码；被 wasm 移除后再次
装备时旧代码会以**通用默认值**复活实例（与移除前读取的类型表值不一
致——重置显示 35、装武器变 32 的自相矛盾）。新物化一律取类型表声明
基础值，消除该不一致（属红线 2 要求的体验一致性方向）。

## 基准与闸门

双臂（旧臂在基准内复刻旧 FxHashMap 承载与六方法语义，新臂复刻
覆盖层），语料 20000 实体（僵尸 5000/骷髅 4000/牛 4000/羊 3000/
苦力怕 2000/玩家 2000，papokin-data 真实类型表），操作脚本含
20 轮热读（每实体移速值+生命基础值）、装备修饰符增删（30%/15%）、
基础值改写（10%）、移除回落探针（5%，验证墓碑语义）、5 轮全量
同步构建。预登记闸门：

| 闸门 | 结果 |
|---|---|
| 操作脚本全量指纹全等（读取/回落/同步） | ✅ `0xcbff7415358508cc` 全等 |
| 驻留字节下降（主判据） | ✅ 52 409 731 → 5 032 163 B（**×10.42**，每实体 2620.5→251.6 B） |
| 构造分配次数下降 | ✅ 87 001 → 15 次（每实体 4.350→0.0005） |
| 热读耗时不劣化（≥0.91×） | ✅ 5.19 → 1.19 ms（**×4.37**，直索引快于哈希探测） |

分段：构造 31.95→0.40 ms（×79.2）、热读 ×4.37（零分配）、增删改写
×1.15（分配 35 430→43 994——物化条目随首次写入发生，换走构造期的
87k 分配，净省约 78k 次/2 万实体）、全量同步 46.89→14.80 ms（×3.17）。

门禁：workspace clippy 0 错误、全测试 0 失败（papokin lib 449 通过，
含覆盖层 10 项新单测：fresh 声明取类型表值/未声明不在场/物化/墓碑/
复活/合并视图计数/Live 修饰符计算等）。

## 红线核对

- **语义**：三态视图 + 原位回落链，操作脚本全量指纹（含三条回落
  分支与同步清单）双臂全等；移除-复活-再移除链路单测钉死。
- **并发**：承载仍在 `RwLock` 内；`AttributeInstance` 原子缓存字段
  原样保留；`TypeBases` 懒构建走 `OnceLock<Mutex<..>>` 只读共享。
- **UX**：full_sync 输出仍按 id 排序、基础值/修饰符逐点一致；唯一
  行为变化是上述旧怪癖修正（更一致方向）。

## 后续方向（轮次 43 候选）

- NBT 驻留生命周期面（轮 41 结论的延续）：长期持有 `custom_data`
  的调用方盘点；
- `equipment_attribute_modifier_ids: Mutex<FxHashMap<EquipmentSlot, …>>`
  等固定小键映射的稀疏化（量级小，低优先）；
- 覆盖层条目 `AttributeInstance` 的进一步收缩（48B：cached+dirty
  可否合并进一位原子）——收益每分叉条目 8B，量级小。
