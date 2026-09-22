<div align="center">

# Papokin

![CI](https://github.com/Quotidietium/Papokin/actions/workflows/rust.yml/badge.svg)
[![License: GPL](https://img.shields.io/badge/License-GPLv3-yellow.svg)](https://opensource.org/licenses/gpl-3-0)

</div>

**Papokin** 是一款完全用 Rust 编写的 Minecraft：Java 版服务器，提供快速、高效、
可定制的体验。它是 [Pumpkin](https://github.com/Pumpkin-MC/Pumpkin) 的分支（fork），
专注于 Java 版协议，插件体系采用 WASM 组件模型。
服务端自身文本（日志、命令反馈、panic/expect 消息）与全部代码文档注释已于
2026-09-22 完成简体中文全量汉化，实施记录见
[note/14-全仓汉化实现记录.md](note/14-全仓汉化实现记录.md)。

<div align="center">

![Papokin 区块加载](./assets/papokin-chunk-loading.webp)

</div>

## 目标

- **性能**：充分利用多线程，追求极致的速度与效率。
- **兼容性**：支持最新的 Java 版 Minecraft，同时严格遵循原版游戏机制。
- **安全**：以安全为先，防范已知的安全漏洞利用。
- **灵活**：高度可配置，可关闭不需要的功能。
- **可扩展**：为插件开发提供坚实基础。

> [!IMPORTANT]
> Papokin 目前处于重度开发阶段。

## 功能

- [x] 配置（toml）
- 协议
  - [x] 服务器状态/Ping
  - [x] 加密
  - [x] 数据包压缩
  - [x] Java 版
  - ...
- 世界
  - [x] 玩家 Tab 列表
  - [x] 记分板
  - [x] 世界加载
  - [x] 世界时间
  - [x] 世界边界
  - [x] 世界保存
  - [x] 光照
  - [x] 实体生成
  - [x] Boss 血条
  - [x] 区块加载（原版、Linear、Pump）
  - 区块生成
  - [x] 区块保存（原版、Linear、Pump）
  - 红石
  - [x] 流体物理
  - ...
- 玩家
  - [x] 皮肤
  - [x] 传送
  - [x] 移动
  - [x] 动作动画
  - [x] 物品栏
  - 战斗
  - [x] 经验
  - [x] 饥饿
  - [X] 副手
  - [X] 进度（进行中）
  - [x] 进食
  - ...
- 实体
  - [x] 非生物（矿车、鸡蛋……）（进行中）
  - [x] 实体效果
  - [x] 玩家
  - [x] 怪物（进行中）
  - [x] 动物（进行中）
  - 实体 AI
  - [x] Boss（进行中）
  - [x] 村民（进行中）
  - [X] 实体保存
- 服务器
  - 插件（WASM 组件模型）
  - [x] Query 查询
  - [x] RCON
  - [x] 容器界面
  - [x] 粒子
  - [x] 聊天
  - 命令
  - [x] 权限
  - [x] 多语言翻译
- 代理
  - [x] [BungeeCord](https://github.com/SpigotMC/BungeeCord)
  - [x] [BungeeGuard](https://github.com/lucko/BungeeGuard)
  - [x] [Velocity](https://github.com/PaperMC/Velocity)

## 如何运行

使用较新的稳定版 Rust 工具链从源码构建：

```sh
cargo build --release
./target/release/papokin
```

服务器首次启动时会生成 `papokin.toml`（若已存在旧的 `pumpkin.toml` 会自动读取）。
WASM 插件放入 `plugins/` 目录；插件 API 参考文档见
[note/12-插件API文档.md](note/12-插件API文档.md)。

## 贡献

欢迎贡献！详见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 赞助

如果你想支持原项目，请访问上游的
[Pumpkin 捐赠页面](https://pumpkinmc.org/donate/)。

## 许可证与署名

* **Papokin 服务器**：基于 [GNU 通用公共许可证 v3.0（GPLv3）](LICENSE) 发布，
  是 [Pumpkin](https://github.com/Pumpkin-MC/Pumpkin)（GPLv3，© Pumpkin-MC
  贡献者）的衍生作品。
* **插件 API（`papokin-plugin-api` 与 `papokin-plugin-wit`）**：采用 [MIT](crates/papokin-plugin-api/LICENSE-MIT) 或 [Apache-2.0](crates/papokin-plugin-api/LICENSE-APACHE) 双许可证，为插件开发提供最大灵活性。
* **第三方资产与数据**：Minecraft 资产遵循其各自的许可证与署名条款。完整说明见 [assets/NOTICE.md](assets/NOTICE.md)。
