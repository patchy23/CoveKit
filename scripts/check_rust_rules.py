#!/usr/bin/env python3
"""CoveKit Rust 代码规范检查（薄 wrapper）。

检查项（实现见 src-tauri/tests/source_rules.rs，入口 `scan_rust_rules`）：
1. panic 候选：非测试代码里的 unwrap / expect / panic! / unreachable! / todo! / assert! 系列；
2. 生命周期逃逸：Box::leak / mem::forget / transmute（零容忍，无基线）；
3. unsafe 论证：每处 unsafe 必须带紧邻的 `// SAFETY:` 注释（零容忍）；
4. clone 统计：只报告不拦截；
5. 层级依赖：framework 不得引用插件、插件之间不得互相 import（零容忍）。
6. 路径入口：不得绕过 framework::paths 直接取落盘根（零容忍，paths.rs 自举例外）。
7. 业务表：framework 下不得出现插件建表名（零容忍，表名由插件 DDL 推导）。

棘轮基线在 scripts/rust_rules_baseline.json：各分类计数只降不升，例外按「路径 + 符号 + 类别 + 原因」逐条登记。

用法：python scripts/check_rust_rules.py
退出码：0 = 通过；1 = 检查失败 / cargo 失败；2 = 调用环境错误
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import source_rules_runner as runner  # noqa: E402  （需先注入 scripts/ 到 sys.path）

if __name__ == "__main__":
    sys.exit(runner.run_entry("scan_rust_rules"))
