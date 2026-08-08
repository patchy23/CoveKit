#!/usr/bin/env python3
"""Rust 注释覆盖率检查（docs/03-plugin-development.md §5 质量门槛）
用法: python scripts/check_docs.py [src-tauri/src 路径]
要求:
  1. 每个 .rs 文件有文件头注释（//!）
  2. 每个结构体/函数有 /// 用途注释（自动跳过 #[cfg]/#[derive] 属性行）
  3. 结构体每个属性有 /// 注释（同一结构体块内检查）
退出码: 0 = 全过；1 = 有缺失
"""
import re
import sys
from pathlib import Path

ROOT = Path(sys.argv[1] if len(sys.argv) > 1 else "src-tauri/src")

def check_file(path: Path) -> list[str]:
    issues = []
    text = path.read_text(encoding="utf-8")
    lines = text.split("\n")

    # 1. 文件头注释（//! 模块注释，或首行 // 块注释）
    head = lines[:8]
    if not any(l.strip().startswith("//!") for l in head) and not (head and head[0].strip().startswith("//")):
        issues.append(f"{path}: 缺少文件头注释（//!）")

    for i, line in enumerate(lines):
        m = re.match(r"^(pub(?:\(crate\))? (?:async )?fn |fn |pub struct |pub\(crate\) struct |struct )", line)
        if not m:
            continue
        # 2. 函数/结构体注释
        j = i - 1
        while j >= 0 and lines[j].strip().startswith("#["):
            j -= 1
        has_doc = j >= 0 and lines[j].strip().startswith("///")
        if not has_doc:
            issues.append(f"{path}:L{i+1} {line.strip()[:50]} 缺注释")

        # 3. 结构体属性注释（含 serde 属性行跳过的字段）
        if "struct " in m.group(0) and ";" not in line:
            k = i + 1
            while k < len(lines) and "}" not in lines[k]:
                pm = re.match(r"^\s+(pub(?:\(crate\))? )?(\w+):", lines[k])
                if pm:
                    prev = k - 1
                    while prev > i and lines[prev].strip().startswith("#["):
                        prev -= 1
                    if not lines[prev].strip().startswith("///") and not lines[k].strip().startswith("//"):
                        issues.append(f"{path}:L{k+1} 属性 {pm.group(2)} 缺注释")
                k += 1
    return issues

def main() -> int:
    all_issues = []
    files = sorted(ROOT.rglob("*.rs"))
    for f in files:
        all_issues += check_file(f)
    if all_issues:
        print(f"✗ {len(all_issues)} 处注释缺失：")
        for x in all_issues:
            print(f"  {x}")
        return 1
    print(f"✓ 注释检查通过（{len(files)} 个 rs 文件，结构体属性全覆盖）")
    return 0

if __name__ == "__main__":
    sys.exit(main())
