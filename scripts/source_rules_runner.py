#!/usr/bin/env python3
"""source_rules 测试 target 的调用器（供 check_rust_rules.py / check_docs.py 两个薄 wrapper 复用）。

AR02 起规范检查的实现下沉到 Rust 侧（`src-tauri/tests/source_rules.rs`，理由见任务书 §5：
行级正则无法区分测试模块 / 注释 / 字符串 / 属性，误报与漏报都已复现）。本模块只做调用与取证：

1. 用 `--exact` 精确过滤入口测试，杜绝「过滤器失效 → 0 个测试 → 假通过」；
2. 校验 cargo 输出里确实 running 1 test、该测试 ok、test result ok；
3. 原样透传 cargo 输出（其中含报告：规则数 / 源范围 / 例外命中 / 未覆盖项）。

退出码语义（两个 wrapper 共用）：
  0 = 检查通过；1 = 检查未通过或 cargo 失败；2 = 调用/环境错误（如 cargo 缺失、目录不存在）
"""
from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

#: 仓库根目录（scripts/ 的上一级），所有 cargo 调用都显式带 --manifest-path，不依赖调用 cwd。
ROOT = Path(__file__).resolve().parent.parent
#: 承载两个入口测试的测试 target 名。
TARGET = "source_rules"

EXIT_OK = 0
EXIT_FAILED = 1
EXIT_USAGE = 2


def run_entry(entry: str, env_extra: dict | None = None, note: str = "") -> int:
    """跑一个入口测试并把结论如实传给调用方。"""
    cmd = [
        "cargo",
        "test",
        "--manifest-path",
        str(ROOT / "src-tauri" / "Cargo.toml"),
        "--locked",
        "--no-default-features",
        "--test",
        TARGET,
        entry,
        "--",
        "--exact",
        "--nocapture",
    ]
    env = dict(os.environ)
    if env_extra:
        env.update(env_extra)

    try:
        proc = subprocess.run(
            cmd,
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    except FileNotFoundError:
        print("✗ 找不到 cargo：请确认 Rust 工具链在 PATH 中", file=sys.stderr)
        return EXIT_USAGE

    output = (proc.stdout or "") + (proc.stderr or "")
    sys.stdout.write(output)
    if note:
        print(note)

    if proc.returncode != 0:
        print(
            f"\n✗ 检查未通过（cargo 退出码 {proc.returncode}，入口 {entry}）",
            file=sys.stderr,
        )
        return EXIT_FAILED

    # 三重确认：不能因为过滤器写错导致 0 个测试仍返回 0。
    running = re.search(r"^running 1 test$", output, re.M)
    passed = re.search(rf"^test {re.escape(entry)} \.\.\. ok$", output, re.M)
    result = re.search(r"^test result: ok\. 1 passed;", output, re.M)
    if not (running and passed and result):
        print(
            f"\n✗ 未能确认真实执行了入口测试 {entry}（--exact 过滤器可能失效），本次不算通过",
            file=sys.stderr,
        )
        return EXIT_FAILED

    print(f"\n✅ 检查通过（入口 {entry}，实测运行 1 个测试）")
    return EXIT_OK
