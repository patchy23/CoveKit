#!/usr/bin/env python3
"""版本一致性检查（T08-5）

发布标签必须和三个版本号完全一致，否则用户拿到的安装包版本与标签对不上：
- 标签（CHANGELOG 语义化版本）
- `package.json` version（前端）
- `src-tauri/tauri.conf.json` version（安装包版本）
- `src-tauri/Cargo.toml` version（Rust crate）

用法：
    python scripts/check_versions.py            # 只校验三处版本号互相一致
    python scripts/check_versions.py v1.2.3     # 额外校验标签与它们一致
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def read_package_version() -> str:
    data = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
    return str(data["version"])


def read_tauri_version() -> str:
    data = json.loads((ROOT / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
    return str(data["version"])


def read_cargo_version() -> str:
    text = (ROOT / "src-tauri" / "Cargo.toml").read_text(encoding="utf-8")
    # 只取 [package] 段落里的 version，避免匹配到依赖项的 version
    package_section = re.split(r"^\[", text, flags=re.MULTILINE)
    for section in package_section:
        if section.startswith("package]"):
            match = re.search(r'^version\s*=\s*"([^"]+)"', section, flags=re.MULTILINE)
            if match:
                return match.group(1)
    raise SystemExit("Cargo.toml 里找不到 [package].version")


def normalize(tag: str) -> str:
    return tag[1:] if tag.startswith("v") else tag


def main() -> int:
    versions = {
        "package.json": read_package_version(),
        "tauri.conf.json": read_tauri_version(),
        "Cargo.toml": read_cargo_version(),
    }
    errors: list[str] = []
    unique = set(versions.values())
    if len(unique) != 1:
        errors.append("版本号不一致：" + "，".join(f"{k}={v}" for k, v in versions.items()))

    will_tag = ""
    for argument in sys.argv[1:]:
        if argument in ("--tag", "-t"):
            continue
        will_tag = argument
    if will_tag:
        expected = normalize(will_tag)
        for name, value in versions.items():
            if value != expected:
                errors.append(f"标签 {will_tag} 与 {name} 的版本 {value} 不一致")

    if errors:
        print("版本一致性检查未通过：")
        for error in errors:
            print(f"  - {error}")
        return 1
    version = next(iter(unique))
    print(f"版本一致性检查通过：{version}" + (f"（标签 {will_tag}）" if will_tag else ""))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
