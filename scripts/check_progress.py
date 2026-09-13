#!/usr/bin/env python3
"""进度台账守卫。

校验 `docs/进度台账.md` 与仓库真实状态一致，防漂移：

1. **覆盖**：`docs/batches/` 下每份任务书／实现方案类文档都必须在台账中出现（漏登记报错）
2. **链接**：台账里引用的文件必须存在
3. **提交号**：台账「证据」列里形如 `abc1234` 的短哈希必须真实存在于 git 历史
4. **状态口径**：台账中的状态标记只能取 ✅ / 🔶 / ⬜ / ⏸ / — 五种

用法：`python scripts/check_progress.py`（仓库根执行），退出码非 0 即失败。
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEDGER = ROOT / "docs" / "进度台账.md"
BATCHES = ROOT / "docs" / "batches"

# 需要进台账的文档：文件名含以下关键词之一
TRACKED_KEYWORDS = ("任务书", "实现方案", "执行计划")
# 允许出现在台账状态列里的标记
ALLOWED_MARKS = {"✅", "🔶", "⬜", "⏸", "—"}
# 反引号中形如 git 短哈希的内容
HASH_RE = re.compile(r"`([0-9a-f]{7,40})`")
LINK_RE = re.compile(r"\]\(([^)#]+)(?:#[^)]*)?\)")
MARK_RE = re.compile(r"[✅🔶⬜⏸—]|(?:^|\|)\s*-\s*\|")


def fail(msg: str) -> None:
    print(f"❌ {msg}")


def git(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args], cwd=str(ROOT), capture_output=True, text=True, encoding="utf-8", errors="replace"
    )


def main() -> int:
    if not LEDGER.exists():
        fail(f"找不到进度台账：{LEDGER.relative_to(ROOT)}")
        return 1
    text = LEDGER.read_text(encoding="utf-8")
    errors = 0

    # 1. 覆盖检查：批次目录下的任务书 / 实现方案 / 执行计划都必须登记
    tracked = sorted(
        p for p in BATCHES.rglob("*.md")
        if any(k in p.name for k in TRACKED_KEYWORDS)
    )
    # 判据：台账中必须存在一行同时含「批次目录名」与「文件名」——只比文件名会漏掉整批未登记
    rows = text.splitlines()
    missing = [
        p for p in tracked
        if not any(p.parent.name in r and p.name in r for r in rows)
    ]
    if missing:
        for p in missing:
            fail(f"未登记进台账：{p.relative_to(ROOT)}")
        errors += len(missing)
    else:
        print(f"✓ 覆盖：{len(tracked)} 份任务书／方案全部登记")

    # 2. 链接检查
    bad_links = []
    for m in LINK_RE.finditer(text):
        target = m.group(1).strip()
        if target.startswith(("http", "mailto")) or not target:
            continue
        if not (LEDGER.parent / target).resolve().exists():
            bad_links.append(target)
    if bad_links:
        for t in bad_links:
            fail(f"台账链接失效：{t}")
        errors += len(bad_links)
    else:
        print("✓ 链接：全部有效")

    # 3. 提交号检查
    bad_hashes = []
    for m in HASH_RE.finditer(text):
        h = m.group(1)
        r = git("cat-file", "-e", f"{h}^{{commit}}")
        if r.returncode != 0:
            bad_hashes.append(h)
    if bad_hashes:
        for h in bad_hashes:
            fail(f"台账里的提交号在 git 历史中不存在：{h}")
        errors += len(bad_hashes)
    else:
        print(f"✓ 提交号：{len(HASH_RE.findall(text))} 个全部存在于 git 历史")

    # 4. 状态口径：表格「状态」列只允许白名单标记
    bad_marks = []
    for i, line in enumerate(text.splitlines(), 1):
        if not line.startswith("|") or "---" in line:
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 3 or cells[0] in ("批次", "标记", "项"):
            continue
        # 状态列：第 3 列（批次 / 任务书 / 状态 / ...）
        cell = cells[2]
        if not cell or cell in ALLOWED_MARKS:
            continue
        if MARK_RE.search(cell):
            continue
        bad_marks.append((i, cell))
    if bad_marks:
        for i, c in bad_marks:
            fail(f"第 {i} 行状态列用了非白名单标记：{c[:40]}（只允许 ✅ 🔶 ⬜ ⏸ —）")
        errors += len(bad_marks)
    else:
        print("✓ 状态口径：全部符合白名单")

    if errors:
        print(f"\n共 {errors} 处不一致，请修正 docs/进度台账.md")
        return 1
    print("\n进度台账校验通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
