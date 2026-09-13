#!/usr/bin/env python3
"""校验台账结构、批次文件覆盖与提交存在；不证明实现正确或测试通过。"""
from __future__ import annotations

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEDGER = ROOT / "docs/进度台账.md"
BATCHES = ROOT / "docs/batches"
TRACKED_KEYWORDS = ("任务书", "实现方案", "执行计划")
ALLOWED_MARKS = {"✅", "🔶", "⬜", "⏸"}
HASH_RE = re.compile(r"`([0-9a-f]{7,40})`")


def commit_exists(value: str) -> bool:
    return subprocess.run(
        ["git", "cat-file", "-e", f"{value}^{{commit}}"], cwd=ROOT,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    ).returncode == 0


def table_rows(text: str):
    """按状态表头解析；不靠第3列猜测所有Markdown表格。"""
    header = []
    for number, line in enumerate(text.splitlines(), 1):
        if not line.startswith("|"):
            header = []
            continue
        cells = [cell.strip() for cell in re.split(r"(?<!\\)\|", line.strip().strip("|"))]
        if all(re.fullmatch(r":?-+:?", cell) for cell in cells):
            continue
        if "状态" in cells:
            header = cells
        elif header:
            yield number, dict(zip(header, cells)), len(cells) == len(header)


def validate(root: Path, text: str) -> list[str]:
    errors = []
    covered = set()
    rows = list(table_rows(text))
    if not rows:
        errors.append("台账没有可解析的状态表")
    for line, row, complete in rows:
        if not complete:
            errors.append(f"第{line}行列数与表头不符")
        mark = row.get("状态", "")
        if mark not in ALLOWED_MARKS:
            errors.append(f"第{line}行非法状态：{mark}")
        evidence = next((v for k, v in row.items() if k.startswith("证据")), "")
        # 批次要求提交证据；子项由父批次与说明承载证据。
        if "批次" in row:
            if mark == "✅" and not HASH_RE.search(evidence):
                errors.append(f"第{line}行已完成却没有提交证据")
            if mark == "⏸" and not ("裁决" in " ".join(row.values()) and
                                   re.search(r"20\d{2}-\d{2}-\d{2}", " ".join(row.values()))):
                errors.append(f"第{line}行不补项缺裁决日期和依据")
            batch = row["批次"].strip("`")
            folder = BATCHES / batch
            if not folder.is_dir() or folder.resolve().parent != BATCHES.resolve():
                errors.append(f"第{line}行批次不存在或路径越界：{batch}")
            documents = re.findall(r"`([^`]+\.md)`", row.get("任务书", ""))
            if not documents and "无独立任务书" not in row.get("任务书", ""):
                errors.append(f"第{line}行缺明确任务文件")
            for doc in documents:
                target = folder / doc
                if not target.is_file() or not target.resolve().is_relative_to(folder.resolve()):
                    errors.append(f"第{line}行任务文件不存在或越界：{batch}/{doc}")
                if target in covered:
                    errors.append(f"重复登记：{batch}/{doc}")
                covered.add(target)
    for target in BATCHES.rglob("*.md"):
        if any(key in target.name for key in TRACKED_KEYWORDS) and target not in covered:
            errors.append(f"未登记：{target.relative_to(root)}")
    for value in sorted(set(HASH_RE.findall(text))):
        if not commit_exists(value):
            errors.append(f"提交不存在：{value}（CI需完整Git历史）")
    return errors


def main() -> int:
    if not LEDGER.exists():
        print("台账不存在")
        return 1
    errors = validate(ROOT, LEDGER.read_text(encoding="utf-8"))
    for error in errors:
        print(f"✗ {error}")
    print(f"台账结构检查：{len(errors)}处错误；实现语义与验收真实性需人工复核")
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
