"""文档工具行为测试：隔离临时 Git 仓库，不修改真实工作区。"""
import contextlib
import io
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import check_doc_budget as budget
import check_markdown as markdown


class BudgetTests(unittest.TestCase):
    def check(self, agent="入口", extra="", refs=None):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "docs").mkdir()
            (root / "AGENTS.md").write_text(agent, encoding="utf-8")
            refs = ["../AGENTS.md"] if refs is None else refs
            quote = chr(96)
            (root / "docs/README.md").write_text(
                "## 最小必读集\n" + "\n".join(quote + ref + quote for ref in refs), encoding="utf-8")
            page = root / "docs/extra.md"
            page.write_text(extra, encoding="utf-8")
            return budget.validate(root, [page])

    def test_ordinary_size_is_advisory(self):
        errors, warnings = self.check(extra="x" * 30001)
        self.assertFalse(errors)
        self.assertTrue(warnings)

    def test_ordinary_lines_are_advisory(self):
        errors, warnings = self.check(extra="x\n" * 501)
        self.assertFalse(errors)
        self.assertTrue(warnings)

    def test_agent_budget_remains_enforced(self):
        self.assertTrue(self.check(agent="x" * 10001)[0])

    def test_missing_entry_fails(self):
        self.assertTrue(self.check(refs=["missing.md"])[0])

    def test_read_set_budget_remains_enforced(self):
        self.assertTrue(self.check(extra="x" * 15001, refs=["../AGENTS.md", "extra.md"])[0])

    def test_todo_is_not_mandatory_reading(self):
        self.assertFalse(self.check()[0])


class TrackedFilesTests(unittest.TestCase):
    def test_local_only_target_cannot_mask_broken_repository_link(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            page = root / "README.md"
            local = root / "TODO.md"
            local.write_text("# 任务", encoding="utf-8")
            page.write_text("[任务](TODO.md#任务)", encoding="utf-8")
            subprocess.run(["git", "add", "--", page.name, local.name], cwd=root, check=True)
            self.assertFalse(markdown.check_file(page, root, markdown.tracked_files(root)))
            subprocess.run(["git", "rm", "--cached", "--", local.name], cwd=root, check=True, capture_output=True)
            self.assertTrue(local.is_file())
            self.assertIn("未被 Git 跟踪", markdown.check_file(page, root, markdown.tracked_files(root))[0])
            page.write_text("[目录](.)", encoding="utf-8")
            self.assertFalse(markdown.check_file(page, root, markdown.tracked_files(root)))

    def test_untracked_draft_excluded_and_explicit_file_checked(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            tracked = root / "已跟踪.md"
            draft = root / "draft.md"
            tracked.write_text("正常", encoding="utf-8")
            draft.write_text("[坏链接](missing.md)", encoding="utf-8")
            subprocess.run(["git", "add", "--", tracked.name], cwd=root, check=True)
            self.assertEqual(markdown.tracked_markdown(root), [tracked])
            with patch.object(markdown, "ROOT", root), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(markdown.main(), 0)
                self.assertEqual(markdown.main([draft.name]), 1)
                self.assertEqual(markdown.main(["absent.md"]), 1)
            tracked.unlink()
            self.assertEqual(markdown.tracked_markdown(root), [])


class MarkdownTests(unittest.TestCase):
    def check(self, text):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / "index.md"
            source.write_text(text, encoding="utf-8")
            (root / "目标.md").write_text("# 标题\n## 重复\n## 重复\n", encoding="utf-8")
            return markdown.check_file(source, root)

    def test_missing_link_fails(self):
        self.assertTrue(self.check("[x](missing.md)"))

    def test_bad_anchor_fails(self):
        self.assertTrue(self.check("[x](目标.md#不存在)"))

    def test_duplicate_anchor_passes(self):
        self.assertFalse(self.check("[x](目标.md#重复-1)"))

    def test_reference_link_fails(self):
        self.assertTrue(self.check("[x][bad]\n[bad]: missing.md"))

    def test_code_and_external_links_ignored(self):
        self.assertFalse(self.check("```md\n[x](missing.md)\n```\n[x](https://example.com)"))

    def test_percent_encoded_path_passes(self):
        self.assertFalse(self.check("[x](%E7%9B%AE%E6%A0%87.md#标题)"))

    def test_unknown_reference_fails(self):
        self.assertTrue(self.check("[x][unknown]"))

    def test_inline_example_is_ignored(self):
        self.assertFalse(self.check("示例 `[x](missing.md)`"))

    def test_relative_parent_passes(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "docs").mkdir()
            (root / "TODO.md").write_text("# 裁决", encoding="utf-8")
            page = root / "docs/index.md"
            page.write_text("[裁决](../TODO.md#裁决)", encoding="utf-8")
            self.assertFalse(markdown.check_file(page, root))

    def test_template_paths_are_current(self):
        for path in (Path(__file__).resolve().parent.parent / "docs/standards/templates").glob("*.md"):
            self.assertNotIn("docs/03-plugin-development.md", path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
