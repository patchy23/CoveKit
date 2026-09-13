"""文档守卫负例：临时目录隔离，不污染实际仓库或提交。"""
import contextlib
import io
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import check_doc_budget as budget
import check_progress as progress
import check_markdown as markdown


class BudgetTests(unittest.TestCase):
    def run_case(self, extra="", missing_todo=False):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "docs").mkdir()
            (root / "AGENTS.md").write_text("入口", encoding="utf-8")
            (root / "TODO.md").write_text("裁决", encoding="utf-8")
            refs = ["../AGENTS.md"] if missing_todo else ["../AGENTS.md", "../TODO.md"]
            (root / "docs/README.md").write_text(
                "## 最小必读集\n" + "\n".join(f"- `{r}`" for r in refs), encoding="utf-8"
            )
            (root / "docs/extra.md").write_text(extra, encoding="utf-8")
            with patch.object(budget, "ROOT", root), patch.object(budget, "BASELINE", {}):
                with contextlib.redirect_stdout(io.StringIO()):
                    return budget.main()

    def test_new_oversize_fails(self):
        self.assertNotEqual(self.run_case("x" * 30001), 0)

    def test_new_too_many_lines_fails(self):
        self.assertNotEqual(self.run_case("x\n" * 501), 0)

    def test_missing_decision_entry_fails(self):
        self.assertNotEqual(self.run_case(missing_todo=True), 0)

    def test_small_document_passes(self):
        self.assertEqual(self.run_case("正常"), 0)


class ProgressTests(unittest.TestCase):
    def run_case(self, mark="✅", evidence="`abcdef1`", document="任务书.md", git_ok=True):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            batch = root / "docs/batches/probe"
            batch.mkdir(parents=True)
            (batch / "任务书.md").write_text("任务", encoding="utf-8")
            ledger = root / "docs/进度台账.md"
            ledger.write_text(
                "| 批次 | 任务书 | 状态 | 证据 | 未完成／备注 |\n"
                "| --- | --- | --- | --- | --- |\n"
                f"| `probe` | `{document}` | {mark} | {evidence} | 说明 |\n", encoding="utf-8"
            )
            with patch.object(progress, "ROOT", root), patch.object(progress, "LEDGER", ledger), \
                    patch.object(progress, "BATCHES", root / "docs/batches"), \
                    patch.object(progress, "commit_exists", return_value=git_ok):
                with contextlib.redirect_stdout(io.StringIO()):
                    return progress.main()

    def test_bad_status_with_valid_emoji_fails(self):
        self.assertNotEqual(self.run_case(mark="✅ 胡乱状态"), 0)

    def test_done_requires_evidence(self):
        self.assertNotEqual(self.run_case(evidence="无"), 0)

    def test_unknown_commit_fails(self):
        self.assertNotEqual(self.run_case(git_ok=False), 0)

    def test_missing_document_fails(self):
        self.assertNotEqual(self.run_case(document="不存在.md"), 0)

    def test_document_only_can_be_pending(self):
        self.assertEqual(self.run_case(mark="⬜", evidence="未验证"), 0)

    def test_valid_done_passes(self):
        self.assertEqual(self.run_case(), 0)


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

    def test_table_headers_choose_status_column(self):
        rows = list(progress.table_rows("| 状态 | 批次 |\n| --- | --- |\n| 🔶 | probe |"))
        self.assertEqual(rows[0][1]["状态"], "🔶")

    def test_fake_row_outside_table_does_not_count(self):
        self.assertEqual(list(progress.table_rows("probe 任务书.md ✅")), [])

    def test_template_paths_are_current(self):
        for path in (Path(__file__).resolve().parent.parent / "docs/standards/templates").glob("*.md"):
            self.assertNotIn("docs/03-plugin-development.md", path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
