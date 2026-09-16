"""验证 CI 分流的保守边界、删除与重命名，不访问远端。"""
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from ci_changes import classify, changed_paths


class ChangeTests(unittest.TestCase):
    def test_docs_only_skip_product(self):
        self.assertFalse(classify(["AGENTS.md", "TODO.md", "docs/a.md"])["product"])

    def test_doc_tools_run_own_tests(self):
        self.assertEqual(classify(["scripts/check_markdown.py"]), {"product": False, "doc_tools": True})

    def test_code_config_and_unknown_paths_run_product(self):
        for path in ["src/a.ts", "src-tauri/Cargo.toml", "pnpm-lock.yaml", ".github/workflows/ci.yml",
                     "scripts/ci_changes.py", "docs/example.py", "src/README.md", "new.config"]:
            with self.subTest(path=path):
                self.assertTrue(classify([path])["product"])

    def test_mixed_changes_run_product(self):
        self.assertTrue(classify(["docs/a.md", "src/main.ts"])["product"])

    def test_missing_or_unavailable_baseline_is_conservative(self):
        self.assertIsNone(changed_paths(Path("."), "0" * 40, "HEAD"))
        with patch("ci_changes.subprocess.run", return_value=subprocess.CompletedProcess([], 1, b"", b"")):
            self.assertIsNone(changed_paths(Path("."), "missing", "HEAD"))

    def test_rename_from_code_to_docs_keeps_code_path(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            def git(*args):
                return subprocess.run(["git", *args], cwd=root, check=True, capture_output=True).stdout.decode().strip()
            git("init", "-q")
            (root / "code.ts").write_text("content", encoding="utf-8")
            git("add", "--", "code.ts")
            git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
            base = git("rev-parse", "HEAD")
            (root / "docs").mkdir()
            git("mv", "code.ts", "docs/a.md")
            git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "move")
            paths = changed_paths(root, base, "HEAD")
            self.assertIn("code.ts", paths)
            self.assertIn("docs/a.md", paths)
            self.assertTrue(classify(paths)["product"])


if __name__ == "__main__":
    unittest.main()
