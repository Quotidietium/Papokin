# Papokin 汉化翻译规范（阶段执行手册）

> **状态（2026-09-22）：全仓汉化已完成并通过全部门禁**（20203 单元译 19006 / 保留 1197；
> 968 测试全绿；e2e 42 行标记 0 失败类），实施记录见
> [note/14-全仓汉化实现记录.md](note/14-全仓汉化实现记录.md)。
> 本文件保留为新增/修改代码时文本翻译的现行规范。

## 任务定义
把指定的 Rust 源文件中**面向人的英文散文**翻译成简体中文。只改文本，不改逻辑。

## 翻译对象（仅此几类）
1. 日志宏消息：`info!/warn!/error!/debug!/trace!` 中的字符串字面量
2. `println!/eprintln!/panic!/unreachable!/todo!/unimplemented!/expect(...)` 中的消息
3. 命令 `DESCRIPTION` 常量、发给玩家/控制台的反馈文本（`TextComponent::text("...")` 等）
4. 配置校验/默认值中的**用户可读消息**（如 kick_message 默认值）

## 严禁触碰
- 标识符、变量名、函数名、类型名、crate 名
- tracing 的 `target:` 字符串、结构化字段**键名**（`foo=bar` 里的 `foo` 不译，值视情况）
- 日志 target 如 `"papokin::gametest"`
- 路径、URL、文件名、扩展名、配置键名、权限节点、注册表键（`minecraft:*`、`papokin:*`）
- 格式占位符：`{}`、`{:?}`、`{0}`、`{name}` —— **逐字保留**，位置可移动
- 格式说明符：`{:.2}`、`{:x}` 等
- E2E 标记：`examples/e2e-plugin/` **整个目录不动**；任何 `"E2E ..."` 字符串不动
- 生成代码：`crates/papokin-data/src/generated/`、`crates/papokin-plugin-api/src/generated/`
- 注释（那是另一阶段的事，本阶段**不改注释**）
- 测试断言：若断言的字符串被你翻译了，**同步更新断言**；无法确定就保留英文并在汇报中列出

## 术语表（强制一致）
服务器=server、客户端=client、玩家=player、世界=world、区块=chunk、实体=entity、
插件=plugin、数据包=datapack、资源包=resource pack、注册表=registry、刻=tick、
TPS 不译、数据包（网络）=packet→数据包、登录=login、踢出=kick、封禁=ban、
白名单=whitelist、管理员=op/operator、游戏模式=gamemode、创造/生存/冒险/旁观模式、
重生=respawn、生成=spawn、Boss 血条=boss bar、记分板=scoreboard、进度=advancement、
配方=recipe、附魔=enchantment、效果=effect、属性=attribute、维度=dimension、
生物群系=biome、结构=structure、战利品表=loot table、交易=trade、村民=villager、
末影龙=dragon、信标=beacon、铁砧=anvil、红石=redstone、传送门=portal、
下界=nether、末地=the end、主世界=overworld、种子=seed、MOTD 不译、RCON 不译、
Query 不译、代理=proxy、离线模式=offline mode、压缩=compression、加密=encryption、
心跳=heartbeat、遥测=telemetry、崩溃报告=crash report、权限=permission、
命名空间=namespace、标识符=identifier、调度器=scheduler、任务=task、事件=event、
通道=channel、WASM/WIT/IPC 不译、组件模型=component model、沙箱=sandbox、
签名=signature、元数据=metadata、缓存=cache、快照=snapshot、保存=save、加载=load、
卸载=unload、重载=reload、关停=shutdown、启动=startup、初始化=initialize、
配置=config、校验=validate、解析=parse、序列化=serialize、线程=thread、
异步=async、运行时=runtime、锁=lock、缓冲区=buffer、迭代器=iterator、闭包=closure、
trait/struct/enum/crate 不译

## 不译的专名
Linear、Anvil、Region、MCA、wasmsign2、tokio、rayon、wasmtime、BungeeCord、Velocity、
SignPath、GitHub、Docker、Papokin、Pumpkin、Minecraft、Java、Rust、TOML、JSON、NBT、
SNBT、UUID、HTTP(S)、TCP、UDP、WebSocket、TLS、DNS、IP、API、CLI、TUI、TTY

## 风格
- 简洁的运维中文，直陈事实；错误消息保持"事实+原因"结构
- 示例：
  - `"Starting server"` → `"正在启动服务器"`
  - `"Failed to load config at {}: {err}"` → `"加载配置 {} 失败：{err}"`
  - `"Player {name} joined the game"` → `"玩家 {name} 加入了游戏"`
  - `"Chunk ({x}, {z}) is already loaded"` → `"区块 ({x}, {z}) 已加载"`
  - `"Kicked for spamming packets"` → `"因发送数据包过于频繁被踢出"`
- 中文与英文/数字/标识符之间不加多余空格（日志紧凑优先），但可读性差时可加
- 标点用中文全角（，。：；），但占位符/标识符紧邻处可用半角

## 验证
- 完成本批后运行 `cargo check -p <crate>`（需要 `export PATH="$HOME/.cargo/bin:$PATH"`），必须 0 错误
- 用 `git diff --stat` 自查只改了该改的文件
- 汇报：改动文件数、翻译字符串数、保留英文的字符串及原因、check 结果

---

# 阶段 C 增补：Rust 注释/rustdoc 翻译规则

## 对象
`//` 行注释、`///` rustdoc 注释、`//!` 模块文档、`/* */` 块注释。

## 严禁触碰（在阶段 B 禁碰清单基础上追加）
- **字符串字面量一个字都不动**（阶段 B 已处理，重复改会引发冲突）
- rustdoc 代码块（``` 围栏）内的**代码**不动；代码内的英文注释可译
- rustdoc 链接结构保持：`[文本](url)`、`[`Ident`]`、`[`Ident`](path)` —— 链接目标与标识符不译，显示文本可译
- `# Examples`、`# Panics`、`# Errors`、`# Safety` 等 rustdoc 章节标题**保留英文**（rustdoc 惯例可检索）
- `SAFETY:`、`TODO`、`FIXME`、`XXX`、`HACK`、`NOTE` 前缀词保留英文，后续内容译（如 `SAFETY: 调用者保证……`、`TODO: 接入战利品表`）
- 文档测试里的断言字符串不动
- `#[doc = "..."]` 属性按 rustdoc 处理
- 注释中的代码标识符、类型名、函数名用反引号包裹的保持原样
- 行号引用、文件路径引用（如 `world.rs:952`）保持原样

## 风格
- 技术中文，简洁准确；长段设计说明逐句意译，保留原意与技术细节
- 不逐词硬译；英文长句可拆为多个中文短句
- 保持注释的缩进与 `//`/`///` 前缀不变

---

# 阶段 C 增补：Rust 注释/rustdoc 翻译规则

## 对象
`//` 行注释、`///` rustdoc 注释、`//!` 模块文档、`/* */` 块注释。

## 严禁触碰（在阶段 B 禁碰清单基础上追加）
- **字符串字面量一个字都不动**（阶段 B 已处理，重复改会引发冲突）
- rustdoc 代码块（``` 围栏）内的**代码**不动；代码内的英文注释可译
- rustdoc 链接结构保持：`[文本](url)`、`[`Ident`]`、链接目标与标识符不译，显示文本可译
- `# Examples`、`# Panics`、`# Errors`、`# Safety` 等 rustdoc 章节标题**保留英文**（rustdoc 惯例可检索）
- `SAFETY:`、`TODO`、`FIXME`、`XXX`、`HACK`、`NOTE` 前缀词保留英文，后续内容译（如 `SAFETY: 调用者保证……`、`TODO: 接入战利品表`）
- 文档测试里的断言字符串不动
- `#[doc = "..."]` 属性按 rustdoc 处理
- 注释中反引号包裹的代码标识符、类型名、函数名保持原样
- 行号引用、文件路径引用（如 `world.rs:952`）保持原样

## 风格
- 技术中文，简洁准确；长段设计说明逐句意译，保留原意与技术细节
- 不逐词硬译；英文长句可拆为多个中文短句
- 保持注释的缩进与 `//`/`///` 前缀不变
