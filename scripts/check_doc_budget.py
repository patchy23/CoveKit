#!/usr/bin/env python3
"""文档体量预算守卫（棘轮式：只减不增）。

字符数是跨模型可比较的体量代理，不等于token数。限制默认读取和新文档超限。

四条硬约束：

1. **自动注入类** —— 每次会话必定加载的文件（`AGENTS.md`）有硬上限
2. **最小必读集** —— 新会话按 `docs/README.md` §最小必读集 列出的文件，总量有硬上限
3. **单份文档** —— 超过单文件上限的必须拆分；已在基线里的按**棘轮**处理（不得增长）
4. **单份行数** —— 过长文件即使字符不多也难分片读取

基线里的历史存量只允许缩小——这就是棘轮：不改存量，但禁止新增与增长。

用法：`python scripts/check_doc_budget.py`（仓库根执行），退出码非 0 即失败。
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# ---- 预算（字符）----
AUTO_INJECT_BUDGET = 10_000   # 每次会话自动注入的文件（AGENTS.md）
READ_SET_BUDGET = 15_000      # 新会话最小必读集总量
SINGLE_FILE_BUDGET = 30_000   # 单份文档
SINGLE_FILE_LINES = 500       # 单份文档行数

# ---- 自动注入文件 ----
AUTO_INJECT = ["AGENTS.md"]

# ---- 棘轮基线：历史存量，只减不增（字符数）----
# 棘轮基线：仅收「行数超标但字符数达标」的表格密集文档——行数上限本意是便于分片读取，
# 这类文档每行短、总量可控（20,484 字符可一次读完），加基线后仍不许增长。
# 长期例外（不是待办）：07-产品需求.md 是表格密集型总纲，已拆出 07a–07d 四个分册，
# 总纲本体保留全貌，按「超线只触发职责审查、不为数字机械拆分」处理。
BASELINE: dict[str, int] = {
    "docs/standards/07-产品需求.md": 20_484,
}

# 文档总检范围
SCAN_DIRS = ["docs", "sketches"]
EXTRA_ROOT_FILES = ["AGENTS.md", "TODO.md", "DESIGN.md", "README.md", "CHANGELOG.md"]


def read(p: Path) -> str:
    return p.read_text(encoding="utf-8", errors="replace")


def parse_read_set() -> list[str]:
    """从 docs/README.md 的 §最小必读集 一节解析出必读文件列表（反引号里的路径或文件名）。"""
    idx = ROOT / "docs" / "README.md"
    if not idx.exists():
        return []
    text = read(idx)
    m = re.search(r"^##\s*最小必读集\s*$(.*?)(?=^##\s|\Z)", text, re.M | re.S)
    if not m:
        return []
    out = []
    for ref in re.findall(r"`([^`]+\.md)`", m.group(1)):
        out.append(ref)
    return out


def resolve(ref: str) -> Path:
    """把必读集里的引用解析成真实路径（相对 docs/ 或仓库根）。"""
    for cand in (ROOT / "docs" / ref, ROOT / ref):
        if cand.exists():
            return cand
    return ROOT / ref


def main() -> int:
    errors: list[str] = []
    warnings: list[str] = []

    # 1. 自动注入文件
    for rel in AUTO_INJECT:
        f = ROOT / rel
        if not f.exists():
            errors.append(f"自动注入文件缺失：{rel}")
            continue
        n = len(read(f))
        if n > AUTO_INJECT_BUDGET:
            errors.append(
                f"{rel} 有 {n:,} 字符，超出自动注入上限 {AUTO_INJECT_BUDGET:,}"
                f"（每个会话都要吃，必须精简或把细节挪去按需读的文档）"
            )
        else:
            print(f"✓ 自动注入 {rel}：{n:,} / {AUTO_INJECT_BUDGET:,} 字符")

    # 2. 最小必读集
    read_set = parse_read_set()
    if not read_set:
        errors.append("docs/README.md 里找不到 §最小必读集 一节（新会话不知道该读哪几份）")
    else:
        resolved = {resolve(ref).resolve() for ref in read_set}
        if (ROOT / "TODO.md").resolve() not in resolved:
            errors.append("最小必读集必须包含TODO.md中的用户裁决")
        if (ROOT / "AGENTS.md").resolve() not in resolved:
            errors.append("最小必读集必须包含AGENTS.md")
        total = 0
        detail = []
        seen = set()
        for ref in read_set:
            f = resolve(ref)
            if f.resolve() in seen:
                continue
            seen.add(f.resolve())
            if not f.exists():
                errors.append(f"最小必读集引用了不存在的文件：{ref}")
                continue
            n = len(read(f))
            total += n
            detail.append(f"{ref}={n:,}")
        if total > READ_SET_BUDGET:
            errors.append(
                f"最小必读集共 {total:,} 字符，超出上限 {READ_SET_BUDGET:,}（{' + '.join(detail)}）"
            )
        else:
            print(f"✓ 最小必读集：{len(read_set)} 份 / {total:,} / {READ_SET_BUDGET:,} 字符")

    # 3/4. 单份文档体量 + 行数
    files: list[Path] = []
    for d in SCAN_DIRS:
        p = ROOT / d
        if p.exists():
            files += [f for f in p.rglob("*.md") if "node_modules" not in f.parts]
    files += [ROOT / f for f in EXTRA_ROOT_FILES if (ROOT / f).exists()]

    oversized = []
    for f in files:
        rel = f.relative_to(ROOT).as_posix()
        t = read(f)
        n, lines = len(t), len(t.splitlines())
        base = BASELINE.get(rel)
        if base is not None:
            if n > base:
                errors.append(
                    f"{rel} 从基线 {base:,} 涨到 {n:,} 字符——棘轮只许减不许增，请拆分或压缩"
                )
            elif n < base:
                warnings.append(f"{rel} 已缩到 {n:,}（基线 {base:,}），可下调脚本里的 BASELINE")
            continue
        if n > SINGLE_FILE_BUDGET or lines > SINGLE_FILE_LINES:
            oversized.append((n, lines, rel))
    for n, lines, rel in sorted(oversized, reverse=True):
        errors.append(
            f"{rel}：{n:,} 字符 / {lines} 行，超出单份上限（{SINGLE_FILE_BUDGET:,} 字符 / {SINGLE_FILE_LINES} 行）——按读取时机拆分，不得加基线隐藏新增超限"
        )

    for w in warnings:
        print(f"⚠️  {w}")
    if not oversized:
        print(f"✓ 单份文档：{len(BASELINE)} 份基线内、其余 {len(files) - len(BASELINE)} 份全部达标")

    if errors:
        print()
        for e in errors:
            print(f"❌ {e}")
        print(f"\n共 {len(errors)} 处超预算")
        return 1
    if warnings:
        print(f"\n{len(warnings)} 处提醒（不阻塞，但应尽快处理）")
    print("文档体量预算校验通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
