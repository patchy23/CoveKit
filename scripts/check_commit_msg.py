"""提交标题检查：把 `docs/standards/21-Git提交规范.md` 的标题条款变成可执行门禁。

用法：
    python scripts/check_commit_msg.py <提交信息文件>     # 由 .githooks/commit-msg 调用
    python scripts/check_commit_msg.py --subject "<标题>"  # 单条标题自检（人工或测试用）
退出码：0 = 通过；1 = 标题违规；2 = 参数错误

只检查标题（正文允许技术标识符与编号）；检查的是**待写入的新提交**，
所以它拦得住问题，而不是事后追认。确认规范判断有误时用 `git commit --no-verify` 跳过。
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

# 规范允许的类型
ALLOWED_TYPES = ("feat", "fix", "refactor", "test", "docs", "build", "ci", "chore")

# 标题格式：类型(scope): 描述
SUBJECT_RE = re.compile(r"^(?P<type>[a-z]+)\((?P<scope>[^()]+)\)[:：]\s*(?P<desc>\S.*)$")
# scope 只看形态：小写字母开头的英文标识（禁止用任务编号充当 scope）
SCOPE_RE = re.compile(r"^[a-z][a-z0-9_-]*$")

# 批次号/任务号形态：rel-202609-001、sync-202609-001、arch-202609-001
BATCH_RE = re.compile(r"[a-z]{2,8}-\d{6}-\d{3}", re.IGNORECASE)
# 评审号与工作项号形态：AR06、P3、L2、C1、D3、M15、T11、RV02
ITEM_RE = re.compile(r"(?<![A-Za-z0-9_])(?:AR|RV|M|L|C|D|T|P)\d{1,3}(?![A-Za-z0-9_])")
# 文档章节号：§11
SECTION_RE = re.compile(r"§\s*\d+")
# 规范点名的笼统过程描述
VAGUE_RE = re.compile(r"完成整改|更新清单|处理评审问题|落实任务书|补充若干|若干项|相关调整|进行了一些")
# 标题描述里的补充说明符号（正文列表标记不受限）
SYMBOL_RULES = (("——", "破折号"), ("--", "连续横杠"))

# git 自己生成的标题不适用本规范
EXEMPT_PREFIXES = ("Merge ", "Revert \"", "fixup!", "squash!", "amend!", "Initial commit")

CUT_MARKER = "------------------------ >8 ------------------------"

RULES_DOC = "docs/standards/21-Git提交规范.md"


def subject_of(message: str) -> str:
    """取提交信息的第一行有效标题（跳过注释行与 git 的剪切标记）。"""
    for line in message.splitlines():
        if CUT_MARKER in line:
            break
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        return stripped
    return ""


def validate(subject: str) -> list[str]:
    """校验单条标题，返回违规说明列表（空列表 = 合规）。"""
    problems: list[str] = []
    if not subject:
        return ["标题为空"]
    if subject.startswith(EXEMPT_PREFIXES):
        return []

    matched = SUBJECT_RE.match(subject)
    if not matched:
        problems.append("标题不符合 `类型(scope): 中文变化描述` 格式")
    else:
        kind = matched.group("type")
        scope = matched.group("scope")
        if kind not in ALLOWED_TYPES:
            problems.append(f"类型 `{kind}` 不在规范允许的范围内（{'/'.join(ALLOWED_TYPES)}）")
        if not SCOPE_RE.match(scope):
            problems.append(f"scope `{scope}` 应是受影响模块的英文标识，不用任务编号")

    # 编号、章节号、补充说明符号严格禁止出现在标题任何位置（含 scope）
    for label, pattern in (
        ("批次号或任务号", BATCH_RE),
        ("任务、评审或工作项编号", ITEM_RE),
        ("文档章节号", SECTION_RE),
    ):
        found = pattern.search(subject)
        if found:
            problems.append(f"标题里出现{label}：{found.group(0)}")
    for symbol, name in SYMBOL_RULES:
        if symbol in subject:
            problems.append(f"标题描述里用了{name}做补充说明：{symbol}")
    if "（" in subject or "）" in subject:
        problems.append("标题描述里用了括号做补充说明")
    if VAGUE_RE.search(subject):
        problems.append("标题用了笼统过程描述，没写清实际变化")
    return problems


def check_file(path: Path) -> list[str]:
    """读取提交信息文件并校验其标题。"""
    return validate(subject_of(path.read_text(encoding="utf-8", errors="replace")))


def report(subject: str, problems: list[str]) -> None:
    """打印违规定位与改法提示。"""
    print(f"✗ 提交标题不符合规范（{RULES_DOC}）")
    print(f"  当前标题：{subject or '(空)'}")
    for problem in problems:
        print(f"  - {problem}")
    print("  改法：标题只写本次实际变化，读标题即知改了什么；编号放正文末尾的辅助关联项，")
    print("        标题描述里不用括号和破折号补充说明。确认规范判断有误可用 git commit --no-verify 跳过。")


def main(argv: list[str]) -> int:
    """命令行入口：提交信息文件或单条标题。"""
    if not argv:
        print(__doc__.strip().splitlines()[2].strip(), file=sys.stderr)
        return 2
    if argv[0] == "--subject":
        if len(argv) < 2:
            print("--subject 需要一条标题", file=sys.stderr)
            return 2
        subject = argv[1].strip()
        problems = validate(subject)
    else:
        path = Path(argv[0])
        if not path.is_file():
            print(f"✗ 提交信息文件不存在：{path}", file=sys.stderr)
            return 2
        subject = subject_of(path.read_text(encoding="utf-8", errors="replace"))
        problems = validate(subject)
    if problems:
        report(subject, problems)
        return 1
    print(f"✓ 提交标题合规：{subject}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
