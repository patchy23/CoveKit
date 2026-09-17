"""提交门禁的路径、变更范围和失败传播测试；不调用产品工具链。"""
import contextlib
import io
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import pre_commit as hook


class PreCommitTests(unittest.TestCase):
    def test_paths_with_spaces_remain_single_arguments(self):
        checks = hook.plan(['docs/含 空格.md', 'src/a b.vue'], [])
        self.assertIn('docs/含 空格.md', checks[0][1])
        self.assertIn('src/a b.vue', checks[1][1])
        self.assertEqual(len(checks), 2)

    def test_rust_never_calls_cargo_even_indirectly(self):
        self.assertEqual(hook.plan(['src-tauri/src/lib.rs'], []), [])

    def test_deleted_document_checks_remaining_links(self):
        self.assertEqual(hook.plan([], ['docs/old.md'])[0][1][1:], ['scripts/check_markdown.py'])

    def test_budget_only_runs_for_entry_changes(self):
        self.assertEqual(len(hook.plan(['docs/a.md'], [])), 1)
        self.assertEqual(len(hook.plan(['AGENTS.md'], [])), 2)

    def test_git_paths_and_partial_stage_in_real_temporary_repo(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            def git(*args):
                subprocess.run(['git', *args], cwd=root, check=True, capture_output=True)
            git('init', '-q')
            path = root / '含 空格.md'
            path.write_text('暂存内容', encoding='utf-8')
            git('add', '--', path.name)
            self.assertEqual(hook.git_paths(root, 'diff', '--cached', '--name-only'), [path.name])
            path.write_text('不同的工作树内容', encoding='utf-8')
            with contextlib.redirect_stderr(io.StringIO()) as error, patch('pre_commit.plan') as planned:
                planned.return_value = []
                self.assertEqual(hook.main(root), 1)
            self.assertIn('暂存版本', error.getvalue())

    def test_check_failure_is_not_reported_as_success(self):
        with patch('pre_commit.git_paths', side_effect=[['docs/a.md'], [], []]), \
                patch('pre_commit.shutil.which', return_value='python'), \
                patch('pre_commit.subprocess.run', return_value=subprocess.CompletedProcess([], 1)), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(hook.main(), 1)

    def test_missing_tool_reports_unexecuted(self):
        with patch('pre_commit.git_paths', side_effect=[['docs/a.md'], [], []]), \
                patch('pre_commit.shutil.which', return_value=None), \
                contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(hook.main(), 0)
        self.assertIn('未执行', output.getvalue())


if __name__ == '__main__':
    unittest.main()
