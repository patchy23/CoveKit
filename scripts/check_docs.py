#!/usr/bin/env python3
"""Rust 注释覆盖率检查（薄 wrapper）。

检查项（实现见 src-tauri/tests/source_rules.rs，入口 `scan_docs`）：
1. 每个 .rs 文件有 `//!` 文件头（模块职责）；
2. 每个 pub / pub(crate) / pub(super) 项（含 impl 内缩进的方法）有 `///` 注释；
3. pub 结构体字段缺少 `///` 仅提示，非显然字段的语义由开发者核对；
4. 枚举变体与 trait 关联项不在覆盖范围（报告会打印覆盖边界，不声称语义全覆盖）。

与旧实现的区别：判定基于 AST，因此缩进的 impl 方法、跨行属性不再漏判，
注释、字符串、属性里的同名文本也不再误判。

用法：python scripts/check_docs.py [src 路径]   （默认 src-tauri/src）
退出码：0 = 通过；1 = 检查失败 / cargo 失败；2 = 参数或目录错误
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import source_rules_runner as runner  # noqa: E402  （需先注入 scripts/ 到 sys.path）

DEFAULT_ROOT = "src-tauri/src"

if __name__ == "__main__":
    raw = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_ROOT
    target = Path(raw)
    if not target.is_absolute():
        target = runner.ROOT / target
    target = target.resolve()
    if not target.is_dir():
        print(f"✗ 目录不存在：{raw}（解析为 {target}）", file=sys.stderr)
        sys.exit(runner.EXIT_USAGE)
    note = f"扫描目录：{target}"
    sys.exit(runner.run_entry("scan_docs", {"PATCHYBOX_DOCS_ROOT": str(target)}, note))
