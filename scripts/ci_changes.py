#!/usr/bin/env python3
"""CI 变更分类：只豁免已知文档路径，未知路径保守执行产品检查。"""
from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path

DOC_TOOLS = {
    "scripts/check_markdown.py", "scripts/check_doc_budget.py",
    "scripts/test_doc_checks.py",
}
ROOT_DOCS = {"AGENTS.md", "TODO.md", "DESIGN.md", "README.md", "CHANGELOG.md", "IDEA.md"}


def classify(paths: list[str]) -> dict[str, bool]:
    product = False
    doc_tools = False
    for path in paths:
        if path in DOC_TOOLS:
            doc_tools = True
        elif path in ROOT_DOCS or (path.startswith(("docs/", "sketches/")) and path.endswith(".md")):
            continue
        else:
            product = True
            # 工程配置也可能影响文档工具的运行方式。
            doc_tools = True
    return {"product": product, "doc_tools": doc_tools}


def changed_paths(root: Path, base: str, head: str) -> list[str] | None:
    if not base or set(base) == {"0"}:
        return None
    result = subprocess.run(
        ["git", "diff", "--no-renames", "--name-only", "-z", base, head, "--"],
        cwd=root, capture_output=True,
    )
    if result.returncode:
        return None
    return [name for name in result.stdout.decode("utf-8").split("\0") if name]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", required=True)
    args = parser.parse_args()
    paths = changed_paths(Path(__file__).resolve().parent.parent, args.base, args.head)
    flags = classify(paths) if paths is not None else {"product": True, "doc_tools": True}
    output = "\n".join(f"{key}={str(value).lower()}" for key, value in flags.items()) + "\n"
    print(output, end="")
    if destination := os.environ.get("GITHUB_OUTPUT"):
        with open(destination, "a", encoding="utf-8") as stream:
            stream.write(output)


if __name__ == "__main__":
    main()
