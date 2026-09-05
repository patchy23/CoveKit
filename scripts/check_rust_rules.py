#!/usr/bin/env python3
"""patchyBox Rust 代码规范检查（提交前手动/CI 运行）

检查项（对应 docs/05-rust-code-standard.md）：
1. panic 风险：非测试代码中的 .unwrap() / .expect( / panic! / unreachable! / unwrap_unchecked
2. 生命周期滥用：Box::leak / mem::forget / unsafe / transmute
3. clone 统计：只报告不拦截（供人工审查参考）

采用棘轮基线（ratchet）：违规总数只允许 ≤ 基线，允许减少、禁止增加。
整改后请同步下调 scripts/rust_rules_baseline.json 中的数字。

用法：python scripts/check_rust_rules.py
退出码：0 = 通过（违规数 ≤ 基线）；1 = 新增违规或基线文件损坏
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src-tauri" / "src"
BASELINE_FILE = Path(__file__).resolve().parent / "rust_rules_baseline.json"

# 允许清单（规则 §1 的例外：启动期 fail-fast / 编译期可证不变量，逐条登记）
ALLOWLIST_PATTERNS = [
    r'expect\("IPC 命令重复注册"\)',
    r'expect\("error while running tauri application"\)',
    r'expect\("HMAC 接受任意长度密钥"\)',
]

PANIC_PATTERN = re.compile(r"\.unwrap\(\)|\.expect\(|panic!|unreachable!|\.unwrap_unchecked")
LIFETIME_PATTERN = re.compile(r"Box::leak|mem::forget|\bunsafe\b|transmute")
ALLOWLIST_RE = re.compile("|".join(f"(?:{p})" for p in ALLOWLIST_PATTERNS))


def strip_test_code(lines: list[str]) -> list[str]:
    """剥离 #[cfg(test)] 起的测试模块（本仓库约定测试在文件末尾）。"""
    out = []
    for ln in lines:
        if "#[cfg(test)]" in ln:
            break
        out.append(ln)
    return out


def main() -> int:
    baseline = json.loads(BASELINE_FILE.read_text(encoding="utf-8"))
    max_allowed = int(baseline["panic_violations"])

    panic_hits: list[str] = []
    lifetime_hits: list[str] = []
    clone_count = 0

    for rs in sorted(SRC.rglob("*.rs")):
        lines = strip_test_code(rs.read_text(encoding="utf-8").split("\n"))
        for i, ln in enumerate(lines, 1):
            rel = rs.relative_to(ROOT).as_posix()
            if PANIC_PATTERN.search(ln) and not ALLOWLIST_RE.search(ln):
                panic_hits.append(f"{rel}:{i}: {ln.strip()[:100]}")
            if LIFETIME_PATTERN.search(ln):
                lifetime_hits.append(f"{rel}:{i}: {ln.strip()[:100]}")
            clone_count += ln.count(".clone()")

    print(f"clone 总数（仅报告）: {clone_count}")

    if lifetime_hits:
        print(f"\n❌ 生命周期/unsafe 违规 {len(lifetime_hits)} 处（零容忍，无基线）：")
        for h in lifetime_hits:
            print(f"  {h}")

    n = len(panic_hits)
    if n > max_allowed:
        print(f"\n❌ panic 风险 {n} 处，超过基线 {max_allowed}（新增 {n - max_allowed} 处）：")
        for h in panic_hits:
            print(f"  {h}")
        return 1

    print(f"\npanic 风险 {n} 处（基线 {max_allowed}，{'已下降，请下调基线' if n < max_allowed else '持平'}）")
    if n and n <= max_allowed:
        print("存量清单（整改时按此消除）：")
        for h in panic_hits:
            print(f"  {h}")
    if lifetime_hits:
        return 1
    print("\n✅ Rust 代码规范检查通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
