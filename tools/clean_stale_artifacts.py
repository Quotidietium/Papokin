#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""清理 cargo 构建缓存中确认不会复用的陈旧产物。

默认干跑只统计；加 ``--apply`` 才真正删除。从仓库根目录运行：

    python tools/clean_stale_artifacts.py           # 干跑预览
    python tools/clean_stale_artifacts.py --apply   # 执行清理
    python tools/clean_stale_artifacts.py --apply --keep 1  # 深度档

规则（按安全性从高到低，与 AGENTS.md「编译缓存清理」一致，只动纯缓存，
绝不动 deps 里在册可复用产物、dist/、~/.cargo/registry）：

1. 一切 ``incremental/`` 目录——纯增量重建缓存（规范强制清理项）。
2. 旧命名遗留产物（crate 改名后永不再被产出的名字，如 ``pumpkin*``）。
3. 孤儿指纹产物：``deps/``、``build/`` 里的 ``<crate>-<hash>`` 条目若
   ``.fingerprint/<pkg>-<hash>`` 已不存在，且其 mtime 早于该 profile
   最新指纹目录 10 分钟以上。cargo 只复用指纹在册的产物，指纹缺失即
   陈旧；个别指纹目录会被并发/残留构建意外修剪，mtime 防护把这类条目
   保留下来，最坏代价也只是多付一次重编，绝无正确性风险。
4. 被取代的同单元产物：deps 里同一 crate、同一单元种类（从指纹目录内
   的 ``lib-*``/``bin-*``/``test-lib-*``/``test-bin-*``/``doctest-*`` 等
   主文件名识别）、同一产物签名（簇内扩展名集合，区分 rlib 的 build 单元
   与仅 rmeta 的 check/clippy 单元）存在更新的哈希簇时，仅保留最新
   ``KEEP_PER_UNIT`` 个簇，更旧的整簇删除（连同其指纹目录）。同一哈希
   链上新簇已产出即意味着旧簇对应的源码状态已被取代，常规增量构建不会
   再引用；仅当 checkout 回旧提交重建时才会需要，代价也只是重编。按
   单元种类+签名分组保证并发的不同单元（如 bin 与 test-bin、check 与
   build）互不误伤。

2026-09-30 首次实跑：释放 24.08 GB（孤儿 16.97 GB ∪ pumpkin 旧命名，
含 examples/e2e-plugin 三个 profile）。教训：不带 mtime 防护的孤儿规则
曾把一个刚构建完、指纹目录被残留进程修剪掉的 rlib 误判为陈旧，代价是
一次 2 分钟重编——防护规则由此而来。

2026-09-30 第二轮勘误与加固：入库版脚本经实测存在两个恰好互相抵消的
bug——① ``find_targets`` 只检查子目录的 ``target``，仓库根自己的
``target/`` 从未被扫描（干跑恒为 0）；② 孤儿判定的键名比较忽略了
产物文件名用下划线（``papokin_data-<hash>``）而指纹目录用包名连字符
（``papokin-data-<hash>``）的差异，多词 crate 永远匹配不上，一旦修好
①就会把在册产物误判成孤儿大规模误删。本轮修复两处并把键名比较统一
为归一化形式，同时新增规则 4 清理同日多轮门禁积累的被取代哈希簇。

2026-10-06 新增 ``--keep`` 参数（默认仍 2）：``--keep 1`` 深度档在版本
递增后清掉上一代指纹的全部 deps（该代确认不会复用，代价仅为 checkout
回旧提交重建时重编）。首次深度实跑：常规档 1.76 GB + 深度档 7.28 GB
（大头为两代 debug 主 crate rlib 各 ~1.08 GB 与旧 test exe），另删
``target/cpu-repro``（note/16 已定案的诊断物料 102 MB）；target 总量
20 GB → 12 GB；清理后 ``cargo check`` 增量命中 12 秒，无误删。
``target/papo-ref``（Papo 对拍环境）为活的分析基准，保留不动。
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import sys
import time

# 文件名形如 <crate>-<16位十六进制>.<ext> 或 lib<crate>-<hash>.<ext>
HASH_RE = re.compile(r"^(?:lib)?(.+)-([0-9a-f]{16})$")
ARTIFACT_EXTS = {".rlib", ".rmeta", ".d", ".exe", ".pdb", ".dll", ".so", ".lib"}
# 指纹缺失但 mtime 距该 profile 最新指纹不足该秒数的条目视为可能在册，保留
FRESH_MARGIN_SECONDS = 600
# 规则 4：同一 (crate, 单元种类) 保留的最新哈希簇数
KEEP_PER_UNIT = 2
# 指纹目录内主文件名的单元种类前缀（长前缀优先匹配）
UNIT_KIND_PREFIXES = (
    "build-script-build",
    "custom-build",
    "test-lib",
    "test-bin",
    "doctest",
    "lib",
    "bin",
)
# 这些名字的产物永不会再被构建产出（crate 已改名）
DEAD_NAMES = ("pumpkin",)


def path_size(path: str) -> int:
    if os.path.isfile(path):
        return os.path.getsize(path)
    total = 0
    for root, _dirs, files in os.walk(path):
        for name in files:
            try:
                total += os.path.getsize(os.path.join(root, name))
            except OSError:
                pass
    return total


def norm_key(name: str) -> str:
    """归一化 crate/包名键：产物用下划线、指纹目录用连字符，统一后比较。"""
    return name.replace("-", "_")


def newest_fingerprint_mtime(fp_dir: str) -> float:
    if not os.path.isdir(fp_dir):
        return 0.0
    return max(
        (os.path.getmtime(os.path.join(fp_dir, d)) for d in os.listdir(fp_dir)),
        default=0.0,
    )


def unit_kind(fp_unit_dir: str) -> str:
    """从指纹目录内的主文件名识别单元种类（lib/bin/test-lib/test-bin/…）。"""
    try:
        entries = os.listdir(fp_unit_dir)
    except OSError:
        return "other"
    mains = [
        e
        for e in entries
        if e != "invoked.timestamp" and not e.endswith(".json") and not e.startswith("dep-")
    ]
    for prefix in UNIT_KIND_PREFIXES:
        if any(e == prefix or e.startswith(prefix + "-") for e in mains):
            return prefix
    return "other"


def scan_profile(profile_dir: str, keep: int = KEEP_PER_UNIT) -> list[str]:
    """返回该 profile（如 target/debug）下确认不会复用的条目路径。

    deps 条目按 (crate, hash) 聚簇：指纹键缺失的整簇判孤儿（规则 3）；
    指纹在册的按 (crate, 单元种类, 产物签名) 分组，组内保最新
    ``keep`` 簇，更旧的整簇连同指纹目录一起删除（规则 4）。
    """
    stale: list[str] = []
    fp_dir = os.path.join(profile_dir, ".fingerprint")
    # 归一键 -> 指纹目录原始名（删除指纹目录时要用原始名）
    fp_keys: dict[str, str] = {}
    if os.path.isdir(fp_dir):
        for key in os.listdir(fp_dir):
            fp_keys[norm_key(key)] = key
    fp_newest = newest_fingerprint_mtime(fp_dir)

    deps = os.path.join(profile_dir, "deps")
    if os.path.isdir(deps):
        clusters: dict[tuple[str, str], dict] = {}
        for name in os.listdir(deps):
            path = os.path.join(deps, name)
            if not os.path.isfile(path):
                continue
            stem, ext = os.path.splitext(name)
            if ext not in ARTIFACT_EXTS:
                continue
            m = HASH_RE.match(stem)
            if not m:
                continue
            crate, h = m.group(1), m.group(2)
            cluster = clusters.setdefault((crate, h), {"files": [], "mtime": 0.0})
            cluster["files"].append(path)
            cluster["mtime"] = max(cluster["mtime"], os.path.getmtime(path))

        # 规则 3：孤儿（指纹缺失，含 mtime 防护例外）
        # 规则 4：被取代（同 crate 同单元种类同签名，保留最新若干簇）
        superseded: dict[tuple[str, str, tuple[str, ...]], list[dict]] = {}
        for (crate, h), cluster in clusters.items():
            key = norm_key(f"{crate}-{h}")
            fp_name = fp_keys.get(key)
            if fp_name is None:
                if fp_newest - cluster["mtime"] >= FRESH_MARGIN_SECONDS:
                    stale.extend(cluster["files"])
                continue
            fp_unit_dir = os.path.join(fp_dir, fp_name)
            kind = unit_kind(fp_unit_dir)
            sig = tuple(sorted({os.path.splitext(f)[1] for f in cluster["files"]} - {".d"}))
            cluster["fp_dir"] = fp_unit_dir
            superseded.setdefault((norm_key(crate), kind, sig), []).append(cluster)

        for _group, members in superseded.items():
            members.sort(key=lambda c: c["mtime"], reverse=True)
            newest_mtime = members[0]["mtime"]
            for cluster in members[keep:]:
                if newest_mtime - cluster["mtime"] < FRESH_MARGIN_SECONDS:
                    continue  # 同批产出，可能仍是并发在册单元，保留
                stale.extend(cluster["files"])
                stale.append(cluster["fp_dir"])

    build = os.path.join(profile_dir, "build")
    if os.path.isdir(build):
        for name in os.listdir(build):
            path = os.path.join(build, name)
            if not os.path.isdir(path):
                continue
            m = HASH_RE.match(name)
            if not m or norm_key(name) in fp_keys:
                continue
            if fp_newest - os.path.getmtime(path) < FRESH_MARGIN_SECONDS:
                continue
            stale.append(path)
    return stale


def find_targets(root: str) -> list[str]:
    """仓库内全部构建 target 目录（跳过 REF/ 参考库与 .git）。

    每一层目录（含仓库根本身）的子目录里出现 ``target`` 即收集。
    """
    targets: list[str] = []
    for dirpath, dirnames, _files in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in ("REF", ".git")]
        if "target" in dirnames:
            targets.append(os.path.join(dirpath, "target"))
            dirnames.remove("target")  # 不嵌套下钻
    return targets


def main() -> int:
    parser = argparse.ArgumentParser(description="清理 cargo 陈旧构建产物（默认干跑）")
    parser.add_argument("--apply", action="store_true", help="真正删除（默认只统计）")
    parser.add_argument(
        "--keep",
        type=int,
        default=KEEP_PER_UNIT,
        help=f"每组保留的最新哈希簇数（默认 {KEEP_PER_UNIT}；"
        "--keep 1 为深度档，版本递增后清掉上一代，代价是回退重建需重编）",
    )
    args = parser.parse_args()
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

    to_delete: set[str] = set()
    for target_dir in find_targets(root):
        # 一切 incremental（纯增量缓存）
        for dirpath, dirnames, _files in os.walk(target_dir):
            if "incremental" in dirnames:
                to_delete.add(os.path.join(dirpath, "incremental"))
                dirnames.remove("incremental")
        # 旧命名遗留（crate 改名后永不复用）
        for dirpath, dirnames, filenames in os.walk(target_dir):
            for name in list(dirnames):
                if any(dead in name for dead in DEAD_NAMES):
                    to_delete.add(os.path.join(dirpath, name))
                    dirnames.remove(name)
            for name in filenames:
                if any(dead in name for dead in DEAD_NAMES):
                    to_delete.add(os.path.join(dirpath, name))
        # 每个 profile（含 wasm32-wasip2/<profile> 等）的孤儿/被取代产物
        for dirpath, dirnames, _files in os.walk(target_dir):
            for name in list(dirnames):
                pdir = os.path.join(dirpath, name)
                if os.path.isdir(os.path.join(pdir, "deps")) or os.path.isdir(
                    os.path.join(pdir, ".fingerprint")
                ):
                    to_delete.update(scan_profile(pdir, args.keep))

    total = sum(path_size(p) for p in to_delete)
    verb = "删除" if args.apply else "可释放（干跑，加 --apply 执行）"
    print(f"{verb} {len(to_delete)} 个条目，共 {total / 1e9:.2f} GB")
    for path in sorted(to_delete):
        print(f"  {path_size(path) / 1e6:10.1f} MB  {os.path.relpath(path, root)}")
        if args.apply:
            try:
                if os.path.isfile(path):
                    os.remove(path)
                else:
                    shutil.rmtree(path)
            except OSError as exc:  # 被占用的条目跳过即可
                print(f"    跳过（占用/失败）：{exc}", file=sys.stderr)

    if args.apply:
        print(f"已释放约 {total / 1e9:.2f} GB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
