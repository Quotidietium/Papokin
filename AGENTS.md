# Papokin 项目规范

## 汉化约定（2026-09-22 起生效）

全仓 Rust 源码已汉化（注释 + 运行时文本），新增/修改代码须延续同一约定：

- 用户可见文本（日志宏、panic/expect 消息、命令 DESCRIPTION、`TextComponent::text` 反馈）与注释/rustdoc 一律用简体中文；术语表与禁译清单见仓库根 `tmp_i18n_guide.md`，实施记录见 `note/14-全仓汉化实现记录.md`。
- 不译：占位符（`{}`、`{name}`、`{:.2}`）、注册表键（`minecraft:*`、`papokin:*`）、tracing `target:`、路径/URL、标识符；rustdoc 章节标题（`# Examples` 等）与 `SAFETY:`/`TODO:` 前缀词保留英文。
- 修改既有英文文本时同步检查测试断言；改动落库后重跑 `cargo fmt` 与 clippy（译文行长短会触发重排与 pedantic lint）。

## 编译缓存清理

每次编译完成后，清理相关缓存，避免对磁盘造成过大压力。

- `target/debug/incremental/` 是纯增量重建缓存，可整体安全删除（下次构建会重新生成）：
  ```sh
  rm -rf target/debug/incremental
  ```
- 背景：该目录在本机曾增长到 87GB（debug target 共 105GB），是磁盘压力的主要来源；`target/release` 与 `dist/`（已发布产物）不属缓存，勿删。
- 建议在每轮构建/测试收尾时执行一次，尤其是在 F: 盘可用空间紧张时。
