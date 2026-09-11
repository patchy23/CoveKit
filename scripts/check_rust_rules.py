#!/usr/bin/env python3
"""patchyBox Rust 代码规范检查（提交前手动/CI 运行）

检查项（对应 docs/05-rust-code-standard.md）：
1. panic 风险：非测试代码中的 .unwrap() / .expect( / panic! / unreachable! / unwrap_unchecked
2. 生命周期滥用：Box::leak / mem::forget / transmute（零容忍，无基线）
3. unsafe 论证：每处 unsafe 必须带紧邻的 `// SAFETY:` 注释。2026-09-11 用户决策：
   unsafe 在「必要且安全」时允许使用，但必须以 SAFETY 注释说明不变量，缺注释即失败
4. clone 统计：只报告不拦截（供人工审查参考）

采用棘轮基线（ratchet）：panic 违规总数只允许 ≤ 基线，允许减少、禁止增加。
整改后请同步下调 scripts/rust_rules_baseline.json 中的数字。

用法：python scripts/check_rust_rules.py
退出码：0 = 通过；1 = 新增违规 / unsafe 缺 SAFETY 注释 / 基线文件损坏
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
# 生命周期类：零容忍（伪造 'static / 类型欺骗）
HARD_LIFETIME_PATTERN = re.compile(r"Box::leak|mem::forget|transmute")
# unsafe：允许使用，但每处必须带紧邻的 // SAFETY: 论证
UNSAFE_PATTERN = re.compile(r"\bunsafe\b")
SAFETY_MARKER = "SAFETY:"
ALLOWLIST_RE = re.compile("|".join(f"(?:{p})" for p in ALLOWLIST_PATTERNS))


def is_comment(line: str) -> bool:
    """该行是否整体为注释（`//` 起，含文档注释）。

    注释中提及 unsafe 不构成使用，不要求 SAFETY 论证。
    """
    return line.strip().startswith("//")


def strip_test_code(lines: list[str]) -> list[str]:
    """剥离 #[cfg(test)] 起的测试模块（本仓库约定测试在文件末尾）。"""
    out = []
    for ln in lines:
        if "#[cfg(test)]" in ln:
            break
        out.append(ln)
    return out


def has_safety_note(lines: list[str], index: int) -> bool:
    """unsafe 行（0-based `index`）上方是否有紧邻的 `// SAFETY:` 注释块。

    只认紧邻的注释块（中间不得夹代码行）；多行注释块内任意一行含 SAFETY: 即通过。
    """
    j = index - 1
    while j >= 0:
        prev = lines[j].strip()
        if not prev.startswith("//"):
            return False
        if SAFETY_MARKER in prev:
            return True
        j -= 1
    return False


def main() -> int:
    baseline = json.loads(BASELINE_FILE.read_text(encoding="utf-8"))
    max_allowed = int(baseline["panic_violations"])

    panic_hits: list[str] = []
    lifetime_hits: list[str] = []
    unsafe_hits: list[str] = []
    clone_count = 0

    for rs in sorted(SRC.rglob("*.rs")):
        lines = strip_test_code(rs.read_text(encoding="utf-8").split("\n"))
        rel = rs.relative_to(ROOT).as_posix()
        for i, ln in enumerate(lines):
            if PANIC_PATTERN.search(ln) and not ALLOWLIST_RE.search(ln):
                panic_hits.append(f"{rel}:{i + 1}: {ln.strip()[:100]}")
            if HARD_LIFETIME_PATTERN.search(ln):
                lifetime_hits.append(f"{rel}:{i + 1}: {ln.strip()[:100]}")
            if UNSAFE_PATTERN.search(ln) and not is_comment(ln) and not has_safety_note(lines, i):
                unsafe_hits.append(f"{rel}:{i + 1}: {ln.strip()[:100]}")
            clone_count += ln.count(".clone()")

    print(f"clone 总数（仅报告）: {clone_count}")

    if lifetime_hits:
        print(
            f"\n❌ 生命周期违规 {len(lifetime_hits)} 处"
            "（Box::leak / mem::forget / transmute 零容忍，无基线）："
        )
        for h in lifetime_hits:
            print(f"  {h}")

    if unsafe_hits:
        print(
            f"\n❌ unsafe 缺 SAFETY 论证 {len(unsafe_hits)} 处"
            "（每处 unsafe 上方必须紧邻 `// SAFETY:` 注释说明不变量）："
        )
        for h in unsafe_hits:
            print(f"  {h}")

    n = len(panic_hits)
    if n > max_allowed:
        print(f"\n❌ panic 风险 {n} 处，超过基线 {max_allowed}（新增 {n - max_allowed} 处）：")
        for h in panic_hits:
            print(f"  {h}")
        return 1

    print(
        f"\npanic 风险 {n} 处（基线 {max_allowed}，"
        f"{'已下降，请下调基线' if n < max_allowed else '持平'}）"
    )
    if n and n <= max_allowed:
        print("存量清单（整改时按此消除）：")
        for h in panic_hits:
            print(f"  {h}")
    if lifetime_hits or unsafe_hits:
        return 1
    print("\n✅ Rust 代码规范检查通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
