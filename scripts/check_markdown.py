#!/usr/bin/env python3
"""本地Markdown显式链接检查；不联网，不编译Rust，不校验普通代码路径。"""
from __future__ import annotations

import re
import unicodedata
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent.parent


def prose(text: str) -> str:
    """去掉围栏与行内代码，保留行号。"""
    result = []
    fence = ""
    for line in text.splitlines():
        marker = re.match(r"^\s{0,3}(`{3,}|~{3,})", line)
        if marker:
            token = marker[1]
            if not fence:
                fence = token
            elif token[0] == fence[0] and len(token) >= len(fence):
                fence = ""
            result.append("")
        elif fence:
            result.append("")
        else:
            result.append(re.sub(r"(`+).*?\1", "", line))
    return "\n".join(result)


def anchors(text: str) -> set[str]:
    """仓库ATX标题的GitHub式slug，含重复标题后缀及显式HTML id。"""
    found = set(re.findall(r"\bid=[\"']([^\"']+)", text))
    counts = {}
    # 保留标题中的行内代码文字；围栏内容不得产生锚点。
    clean = prose(re.sub(r"`([^`\n]+)`", r"\1", text))
    for title in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", clean, re.M):
        title = re.sub(r"\[([^]]+)\]\([^)]*\)", r"\1", title).lower()
        slug = "".join(c for c in title if not unicodedata.category(c).startswith(("P", "S")) or c in "-_")
        slug = re.sub(r"\s", "-", slug)
        count = counts.get(slug, 0)
        counts[slug] = count + 1
        found.add(f"{slug}-{count}" if count else slug)
    return found


def check_file(path: Path, root: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    clean = prose(text)
    definitions = {key.casefold(): value for key, value in re.findall(
        r"^\s{0,3}\[([^]]+)\]:\s*<?([^\s>]+)>?", clean, re.M
    )}
    targets = [(m.start(), m[1]) for m in re.finditer(
        r"!?\[[^]\n]*\]\(\s*(<[^>\n]+>|(?:[^\s()\\]|\\.|\([^()]*\))*)(?:\s+\"[^\"]*\")?\s*\)", clean
    )]
    errors = []
    for m in re.finditer(r"\[([^]\n]+)\]\[([^]\n]*)\]", clean):
        key = (m[2] or m[1]).casefold()
        if key not in definitions:
            errors.append(f"{path.relative_to(root)}: 未定义引用式链接 {key}")
        else:
            targets.append((m.start(), definitions[key]))
    for position, value in targets:
        value = value.removeprefix("<").removesuffix(">")
        if "<" in value or ">" in value:
            continue  # 模板占位符，无真实目标
        url = urlsplit(value)
        if url.scheme or value.startswith("//"):
            continue
        name = unquote(url.path)
        target = (root / name.lstrip("/")) if name.startswith("/") else (path.parent / name)
        if not name:
            target = path
        target = target.resolve()
        line = clean[:position].count("\n") + 1
        prefix = f"{path.relative_to(root)}:{line}"
        if not target.exists():
            errors.append(f"{prefix}: 失效链接 {value}")
        elif url.fragment and target.is_file() and target.suffix.lower() == ".md":
            if unquote(url.fragment) not in anchors(target.read_text(encoding="utf-8")):
                errors.append(f"{prefix}: 不存在的标题锚点 {value}")
    return errors


def main() -> int:
    files = sorted((ROOT / "docs").rglob("*.md")) + sorted(ROOT.glob("*.md"))
    errors = [error for path in files for error in check_file(path, ROOT)]
    for error in errors:
        print(f"✗ {error}")
    print(f"Markdown：{len(files)}文件，{len(errors)}错误；仅显式本地链接/ATX锚点，外链/示例/普通代码路径不验证")
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
