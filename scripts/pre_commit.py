"""轻量提交门禁：按暂存路径检查工作树，避免部分暂存和路径分词造成误验。"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FRONTEND = {'.ts', '.mts', '.vue', '.mjs', '.js', '.css', '.json', '.html', '.yml', '.yaml'}


def git_paths(root: Path, *args: str) -> list[str]:
    output = subprocess.check_output(['git', *args, '-z'], cwd=root)
    return [p for p in output.decode('utf-8').split('\0') if p]


def plan(paths: list[str], deleted: list[str]) -> list[tuple[str, list[str]]]:
    """只规划可本地执行的检查；删除 Markdown 时扩大引用检查范围。"""
    checks = []
    markdown = [p for p in paths if p.endswith('.md')]
    if markdown or any(p.endswith('.md') for p in deleted):
        args = [] if any(p.endswith('.md') for p in deleted) else markdown
        checks.append(('文档链接', [sys.executable, 'scripts/check_markdown.py', *args]))
    if {'AGENTS.md', 'docs/README.md'} & set(paths + deleted):
        checks.append(('入口预算', [sys.executable, 'scripts/check_doc_budget.py']))
    frontend = [p for p in paths if Path(p).suffix in FRONTEND]
    if frontend:
        checks.append(('格式', ['node', 'node_modules/prettier/bin/prettier.cjs', '--check', *frontend]))
    return checks


def main(root: Path = ROOT) -> int:
    paths = git_paths(root, 'diff', '--cached', '--name-only', '--diff-filter=ACMR')
    deleted = git_paths(root, 'diff', '--cached', '--name-only', '--diff-filter=D')
    checks = plan(paths, deleted)
    checked = {p for p in paths if p.endswith('.md') or Path(p).suffix in FRONTEND}
    if any(p.endswith('.md') for p in deleted):
        checked.update(p for p in git_paths(root, 'ls-files') if p.endswith('.md'))
    partial = checked & set(git_paths(root, 'diff', '--name-only'))
    if partial:
        print('被检查文件同时有未暂存修改，工作树结果不能代表暂存版本：', file=sys.stderr)
        print('\n'.join(sorted(partial)), file=sys.stderr)
        print('请先明确提交边界；门禁不会代为暂存或丢弃内容。', file=sys.stderr)
        return 1
    failed = False
    env = {**os.environ, 'PYTHONIOENCODING': 'utf-8'}
    for name, command in checks:
        if not shutil.which(command[0]):
            print(f'{name}未执行：找不到 {command[0]}')
            continue
        if command[0] == 'node' and not (root / command[1]).is_file():
            print(f'{name}未执行：本地 Prettier 依赖未安装')
            continue
        result = subprocess.run(command, cwd=root, env=env, check=False)
        print(f'{name}：' + ('通过' if result.returncode == 0 else '失败'))
        failed |= result.returncode != 0
    if any(p.endswith('.rs') for p in paths):
        print('Rust fmt、AST 规则和行为测试由维护者手动执行，本地钩子未运行 Cargo。')
    return int(failed)


if __name__ == '__main__':
    raise SystemExit(main())
