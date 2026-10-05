# 轮次 25：插件宿主表治理（事件订阅表 + 两处泄漏 + 护栏衰减）

- **日期**：2026-10-05
- **主题**：内存（插件宿主面）
- **红线**：零数据风险、语义等价硬证明（逐键派发序指纹全等）
- **提交**：e7c33bec4（HandlerMap 单事件 `ArcSwap` 化）、00e818aca（待重试集认领移除）、f8909e4f6（权限节点命名空间回收）、e4ff3cf0d（护栏向量衰减）、a6f5ec004（测试展平）

## 背景

轮次 20-24 收口了世界/网络面的「只清不缩」与解析态驻留。本轮转向插件宿主（侦察结论见正文）：`PluginManager::handlers` 整表 COW 的平方级分配、`unloaded_files` 与插件权限节点两处只增不减的累积、`dispatch_guard` 一次大派发永久撑大的每插件驻留。

## 机制

### A. 事件订阅表：整表 COW → 单事件 `ArcSwap` + 注册时排序（e7c33bec4）

旧形态 `ArcSwap<HashMap<&'static str, Vec<Arc<dyn DynEventHandler>>>>`：

- 每次订阅/退订 `rcu` 克隆**全表**（367+ 事件键 × 各键向量）——40 插件 × 25 订阅的负载下加载分配 20.5 MB / 24.1 万次；
- `fire` 持**整表**代际跨 `await`，派发期间并发订阅各钉一份全表；
- 每次 `fire` 现排 `order_handlers`（每次一排序 Vec）。

新形态 `HandlerMap`：外层 `RwLock<HashMap<键, Arc<ArcSwap<Vec>>>>` 仅在事件键**首次出现**时写；订阅只 rcu 单事件向量并在注册时排好序（`sort_handlers`，Bukkit 序：`Reverse(priority)` 稳定排序 = 同优先级注册序）；`fire` 只钉单事件代际、零排序零分配。语义保持：

- 同优先级注册序：稳定排序对「已排序向量 + 尾插一个」重排，等价旧的每次全量稳定排序（单测 `handler_map_same_priority_keeps_registration_order`）；
- 去重：`Context::register_event` 按 `handler_identity` 去重语义原样保留（`dedup_identity` 参数），重复注册不产新代际；
- 键不再移除：事件键有界于事件类型全集，空向量对读取方（`has_handlers`/`handlers_for`）与缺键语义一致；
- `has_handlers`（`fire_blocking` 前置门）从「整表 load 后查键」变为「单键查询」。

### B1. 待重试集认领移除（00e818aca）

`unloaded_files`（无加载器认领的文件，等自定义加载器注册后重试）此前只增不减：目录里每加一个暂不可加载文件（或早期阶段无加载器的 wasm），条目永久累积。两处加载器认领点（批量扫描 `start_loading_all` 与单文件 `start_loading_plugin`）成功后从集合移除。

### B2. 插件权限节点命名空间回收（f8909e4f6）

`PermissionRegistry` 的插件注册节点（`{plugin}:*`）卸载时从不注销：热重载 N 次累积 N 份数据结构，且重新注册撞 `already registered` 报错。新增 `PermissionRegistry::unregister_prefix`（`DashMap::retain`，跳过 `permissions.toml` 预声明的 `from_config` 节点——服务器所有者显式声明不随插件卸载消失），挂入 `unload_plugin` 卸载序列。

### C. 派发护栏向量衰减（e4ff3cf0d）

`PluginHostState::dispatch_guard`（派发失败路径的资源回收闭包记录）成功路径 `clear()` 只清不缩：聊天广播类大派发（数百收件人 × 资源句柄）永久撑大每插件驻留。改 `decay_clear_vec`（轮次 20 原语）：尖峰后在下一轮小派发即缩回（4× 滞回，稳态零收缩）。

## 双臂基准（`benchmark/src/bin/handler_sub_table.rs`）

负载模型：367 事件键、40 插件 × 25 订阅 = 1000 次注册（确定性 LCG）、注册期每 50 次夹一次「热键 fire 钉住跨 25 次注册」（模拟 fire 跨 `await` 期间并发订阅）、稳态 1 万次 fire、半数插件按 source 卸载。分配计数器（次数 + 毛字节 + 释放字节，净驻留 = 毛 − 释放）。

| 指标 | 旧臂（整表 COW） | 新臂（单事件 ArcSwap） | 倍率 |
|---|---|---|---|
| 加载分配字节 | 20,520,552 B | 114,388 B | **179.4×** |
| 加载分配次数 | 241,278 | 2,733 | 88.3× |
| 钉住驻留增量（峰值） | 10,712 B | 7,104 B | 1.51× |
| 稳态 fire 分配（1 万次） | 10,000 | **0** | ∞ |
| 卸载分配 | 6,446 | 610 | 10.6× |
| 逐键派发序指纹 | `0xe36251457c09f3c2` | `0xe36251457c09f3c2` | 全等 |

### 闸门（预登记）

| # | 闸门 | 结果 |
|---|---|---|
| G1 | 逐键派发序指纹全等 | PASS |
| G2 | 加载分配字节 ≤ 旧臂 1/4 | PASS（179× 裕度） |
| G3 | 钉住驻留增量 ≤ 旧臂 1/4 | **FAIL**（1.51×） |
| G4 | 稳态 fire 零分配（新臂） | PASS |
| G5 | 卸载分配 ≤ 旧臂 1/4 | PASS |

### G3 未达的论证（数据优先于预登记估计）

预登记时假设 rcu 钉住期间**多代整表并存**（每钉住期 25 次注册各留一份全表）。实测两臂钉住增量同量级（10.7 KB vs 7.1 KB）：`ArcSwap::rcu` 的中间代际在下一次换代后即释放（每代只被「上一代指针 + 持守卫者」引用），钉住稳态净驻留 ≈ 2 份当前形态（钉住的旧代 + 最新代），差异只剩单份形态大小——而钉住实验发生在注册早期（表尚小），差距被压缩。即：旧形态的真实代价在**分配率**（G2 的 179×）与**派发路径**（G4），不在单次钉住的稳态驻留。G1/G2/G4/G5 全过 + G3 如实记录 FAIL，收编决策不受影响（与轮次 24 同范式）。

## 测试

- `handler_map_register_maintains_bukkit_order` / `handler_map_same_priority_keeps_registration_order`：注册时排序 = 旧每次派发排序的序（优先级序 + 同级注册序）；
- `handler_map_dedup_skips_same_identity`：Context 去重语义保留；
- `handler_map_unregister_source_removes_only_matching` / `handler_map_has_handlers_reflects_emptiness`：退订与空键语义；
- `unregister_prefix_removes_only_namespaced_non_config_nodes`：命名空间回收跳过 toml 预声明；
- `dispatch_guard_decay_reclaims_spike_capacity_next_round`：尖峰滞回带内不缩、下轮小派发缩回；
- 门禁：clippy 全绿；测试基线 util 86→**87**、world 271、protocol 142、papokin 411→**417**（+5 HandlerMap、+1 护栏衰减）。

## 负面发现与后续清单

- 共享 wasmtime `Engine` + 内存保留参数调优（每插件一 Engine/一 100ms ticker、~4GiB VA 预留/实例未调）：本轮侦察确认现状，属架构级改造，留待后续轮次；
- 事件派发路径的每调用 `format!("minecraft:...")` String 与逐资源 boxed 闭包：分配率优化点，未动；
- WASM `Store`/`Instance` 生命周期与 IPC 缓冲经侦察确认全部有界（`StoreLimits` 512MB 上限、`RESOURCE_TABLE` 8192 硬顶、无 thread_local 残留），如实记录防重复探查。
