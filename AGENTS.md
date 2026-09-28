# Papokin 项目规范

## 汉化约定（2026-09-22 起生效）

全仓 Rust 源码已汉化（注释 + 运行时文本），新增/修改代码须延续同一约定：

- 用户可见文本（日志宏、panic/expect 消息、命令 DESCRIPTION、`TextComponent::text` 反馈）与注释/rustdoc 一律用简体中文；术语表与禁译清单见仓库根 `tmp_i18n_guide.md`，实施记录见 `note/14-全仓汉化实现记录.md`。
- 不译：占位符（`{}`、`{name}`、`{:.2}`）、注册表键（`minecraft:*`、`papokin:*`）、tracing `target:`、路径/URL、标识符；rustdoc 章节标题（`# Examples` 等）与 `SAFETY:`/`TODO:` 前缀词保留英文。
- 修改既有英文文本时同步检查测试断言；改动落库后重跑 `cargo fmt` 与 clippy（译文行长短会触发重排与 pedantic lint）。

## 编译缓存清理

**每一次构建或测试结束后都必须清理不会复用的缓存**（2026-09-26 起为强制项，不再是建议），避免对磁盘造成过大压力。**但要谨慎区分**：只删确认不会复用的纯缓存，可复用的依赖/产物一律保留（2026-09-27 补充）。

- `target/debug/incremental/` 与 `target/release/incremental/` 是纯增量重建缓存：其中对应已改动单元的缓存天然失效、未改动单元的缓存重生成成本很低，删除不会拖慢后续构建的正确性，只影响单次重建速度。每轮 `cargo build` / `cargo check` / `cargo clippy` / `cargo test` 收尾时**立即执行**：
  ```sh
  rm -rf target/debug/incremental target/release/incremental
  ```
- 背景：该目录在本机曾增长到 87GB（debug target 共 105GB），是磁盘压力的主要来源。
- **不属缓存、勿删**（编译后仍会复用，删了只会白白拖慢后续构建）：
  - `target/debug/deps`、`target/release/deps`（编译产物与测试二进制，增量构建直接复用）；
  - `target/release` 下的最终产物与中间对象（`.rlib`/`.rmeta` 等）；
  - `dist/`（已发布产物）；
  - `~/.cargo/registry` 与 `target` 之外的任何依赖缓存。
- 判断原则：拿不准某个目录是否可复用时，**先保留并在 note 里记录**，不要随手删除。

## 细粒度提交（2026-09-28 起为强制项）

每轮改动完成门禁后，必须**按主题拆分提交**，禁止把整轮工作压成一个巨型 commit：

- 一个逻辑修复/特性 = 一个 commit，消息沿用 conventional commits + 简体中文祈使句：`fix(范围): 描述` / `feat` / `perf` / `refactor` / `chore` / `docs` / `style`，单行 ≤ 72 字符。
- 依赖序提交：先基础设施（新增 API、trait 变更），后调用方；跨 crate 的同一主题（如 world 层新函数 + papokin 层调用点）合入同一 commit，保证逐 commit 可编译。
- 同一文件混多个主题时，用 `git apply --cached` 按 hunk 拆分暂存（交织 hunk 可用「回退工作树 → 部分应用 → 提交 → 恢复最终态」流程），不要为省事混提。
- 测试与被测行为同 commit 落地；`Cargo.lock` 中对应依赖变更随引用它的 commit 走。

## 每次更新后构建 dist（2026-09-27 起为强制项）

每轮代码改动通过门禁后，必须做一次**细粒度**的编译打包并刷新本地 `dist/`（只重编有改动的 crate，不 `cargo clean`）：

```sh
cargo build --release
cp target/release/papokin.exe dist/papokin-X64-Windows.exe
# 重新打包带版本号的 zip 并重算校验和（版本取 Cargo.toml 的 MC 版本段）
powershell Compress-Archive -Force dist/papokin-X64-Windows.exe dist/papokin-<版本>-X64-Windows.zip
cd dist && sha256sum papokin-* > checksums.sha256
```

- `plugin-devkit` 仅在其内容来源（WIT/SDK/文档）变化时重建，普通服务端改动不必动它。
- GitHub 推送/发布非本规范强制项：推送失败（代理不稳定等）可暂时放弃，本地 dist 产物为准。
