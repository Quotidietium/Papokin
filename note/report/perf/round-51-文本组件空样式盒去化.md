# 轮次 51：文本组件空样式盒去化

- **日期**：2026-10-13
- **主题**：内存占用 · 文本组件每实例驻留（轮 50 挂账直系）
- **基准**：`benchmark/src/bin/text_style_option.rs`（第 51 bin）
- **数据**：`round51-text-style-option.json`（2000 组件/电池、1000 棵克隆树、200 轮读）
- **版本**：0.3.60

## 动机

轮 50 画像暴露：`TextComponent` 每实例 264-276 B，其中 **168 B（61%）是样式盒**
——`TextComponentBase.style: Box<Style>` 在构造时无条件物化，即使组件完全没有
样式（聊天消息、翻译键、物品名的绝对多数形态）。每次构造一次多余分配，
每次克隆逐节点深拷贝一个全空 168 B 堆块；`to_translated`（每次序列化都走）
的 Custom 分支还会为每个译文片段新建空样式盒。

## 改动

`crates/papokin-util/src/text/mod.rs`：

1. `style: Box<Style>` → `style: Option<Box<Style>>`（经 `StyleField` 包装承载），
   `None` 表示未设置任何样式；
2. 新增三访问器：`style()`（None 时返回**真 const 静态** `EMPTY_STYLE` 的引用，
   无 LazyLock 初始化检查）、`style_opt()`（规范化：None 与全空盒折叠等价）、
   `style_mut()`（首次写入按需物化盒）；
3. `StyleField` 解码边界包装：serde 反序列化把空样式盒折叠回 `None`——线格式
   map 形无样式键时不再物化 168 B；序列化行为与 `Option<Box<Style>>` 逐字节一致；
   `Deref/DerefMut` 透传使字段访问代码无需感知包装；
4. 手写 `PartialEq`/`Eq`/`Hash`：按 `style_opt()` 规范化比较，`None ≡ Some(空盒)`
   两态等价（哈希与相等同源规范化，相等的值哈希必相等）；
5. 生产读热路径提升：`to_nbt_compound_for_version`/`to_json_value_for_version`/
   `to_legacy` 三函数顶绑定 `let style = self.style();` 一次，字段读不再逐次过
   Option 分支；
6. 迁移全部触点：util 内 14 构造点 + builder 写点走 `style_mut()`、读点走
   `style()`；`legacy.rs`/`translation.rs`（2 构造）、`advancement.rs`（2 处）、
   `sign.rs`/`signs.rs`（4 读点）、wasm 宿主 `text.rs`（21 写点）同步。

## 双臂实测（旧臂 = 忠实复刻 `Box<Style>` 恒物化结构，`#[repr(C)]` 钉住 56 B 内容盒）

| 电池 | 旧臂 | 新臂 | 差 |
|---|---|---|---|
| 纯文本构造（每实例） | 279 B · 3 alloc | **111 B · 2 alloc** | −60.2% 驻留 |
| 带样式构造（每实例） | 279 B · 4 alloc | 279 B · **3 alloc** | 驻留持平、分配 −1 |
| 克隆广播（4 节点混合树 ×1000） | 1083000 B · 13001 alloc | **720000 B · 7001 alloc** | −33.5% / −46.2% |
| 译码边界（map 形 JSON） | —（旧态必物化空盒） | 111 B/实例 · 全折叠 `None` | 消除空盒 |
| 构造时间 | 基准 | **×2.014** | |
| 克隆时间 | 基准 | **×1.930** | |
| 读时间（生产编码形态） | 基准 | **×2.167** | |

## 闸门（七项全过，判定收编）

1. 指纹全等：逻辑等价树的 JSON 序列化值两臂全等 ✓
2. 驻留字节下降（纯文本）✓
3. 构造分配数下降（纯文本）✓
4. 带样式构造不劣化 ✓
5. 克隆驻留字节下降 ✓
6. 译码空样式折叠 ✓
7. 读路径耗时比 ≥ 0.91（实际 ×2.167）✓

## 语义保真（红线）

- 相等/哈希：手写实现按规范化两态等价，回归测试
  `empty_style_none_and_empty_box_are_equivalent` 锁死；
- 序列化：JSON/NBT 输出与改动前逐字节一致（空样式本就不产出任何键），
  `serde_roundtrip_preserves_style_option` 锁死往返形态；
- 译码：map 形无样式键 → `None`，与旧态的「空盒」在规范化视图下等价；
- 测试：papokin-util 90（+3 新语义回归），全仓 1281（+3）全绿；
  workspace clippy（all-targets）0 错 0 警。

## 过程要记（防重复踩坑）

1. **serde flatten + Option 会物化空盒**：反序列化 map 形即使无样式键也会给出
   `Some(空)`，必须解码边界折叠（`StyleField`），否则磁盘/线格式加载路径的驻留
   收益全失；
2. **`format!` 容量随内联上下文漂移**：同模板在两臂物化出 16/24 B 两种容量，
   污染驻留差分——语料必须窗口外预生成、两臂消费同一批字符串；
3. **`#[inline]` 跨 crate 实测未内联**：读路径 ×0.862 → `inline(always)` +
   `#[expect]` 理由后 ×0.9+；配套把空态判定从「视图 is_empty（11 次字段加载）」
   对齐到生产实际调用形 `style_opt().is_none()`（一次判空）后 ×2.1；
4. **LazyLock 空样式在 None 热路径带初始化检查**：Style 全字段 Option，可直接
   写真 `const` 静态，读成本与堆盒同阶。

## 后续挂账

- `extra: Vec<Self>` 与 `content: Box<TextContent>`（56 B）为内容本征，无进一步
  结构空间；
- 译码路径 11 alloc/实例是 serde_json Value 中转的既有成本（`from_nbt` 同走此
  路），属独立优化面（可立轮：NBT 直达反序列化），与本轮正交。
