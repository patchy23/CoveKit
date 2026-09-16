#!/usr/bin/env python3
"""入口读取预算为硬约束，普通跟踪文档体量仅提醒职责审查。"""
from __future__ import annotations

import re
from pathlib import Path
from check_markdown import tracked_markdown

ROOT = Path(__file__).resolve().parent.parent
AUTO_INJECT_BUDGET = 10_000
READ_SET_BUDGET = 15_000
SINGLE_FILE_BUDGET = 30_000
SINGLE_FILE_LINES = 500


def parse_read_set(root: Path) -> list[str]:
    text = (root / "docs/README.md").read_text(encoding="utf-8")
    match = re.search(r"^##\s*最小必读集\s*$(.*?)(?=^##\s|\Z)", text, re.M | re.S)
    quote = chr(96)
    return re.findall(quote + r"([^" + quote + r"]+\.md)" + quote, match[1]) if match else []


def validate(root: Path, files: list[Path]) -> tuple[list[str], list[str]]:
    errors, warnings = [], []
    agent = root / "AGENTS.md"
    if not agent.is_file():
        errors.append("缺少 AGENTS.md")
    elif len(agent.read_text(encoding="utf-8")) > AUTO_INJECT_BUDGET:
        errors.append("AGENTS.md 超出入口预算")
    try:
        refs = parse_read_set(root)
    except FileNotFoundError:
        refs = []
    if not refs:
        errors.append("文档地图缺少最小必读集")
    resolved = set()
    for ref in refs:
        path = (root / "docs" / ref).resolve()
        if not path.is_file():
            errors.append(f"入口文件不存在：{ref}")
        else:
            resolved.add(path)
    if agent.resolve() not in resolved:
        errors.append("最小必读集缺少 AGENTS.md")
    if sum(len(p.read_text(encoding="utf-8")) for p in resolved) > READ_SET_BUDGET:
        errors.append("最小必读集超出读取预算")
    for path in files:
        text = path.read_text(encoding="utf-8")
        if len(text) > SINGLE_FILE_BUDGET or len(text.splitlines()) > SINGLE_FILE_LINES:
            warnings.append(f"{path.relative_to(root)}：{len(text)}字符/{len(text.splitlines())}行；按职责审查，不强制拆分")
    return errors, warnings


def main() -> int:
    errors, warnings = validate(ROOT, tracked_markdown(ROOT))
    for warning in warnings:
        print(f"提醒：{warning}")
    for error in errors:
        print(f"错误：{error}")
    print(f"入口预算：{len(errors)}错误，普通文档{len(warnings)}条提醒")
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
