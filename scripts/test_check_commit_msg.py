"""提交标题检查器行为测试：只验判定规则与退出码，不触碰真实仓库或历史提交。"""
import contextlib
import io
import tempfile
import unittest
from pathlib import Path

import check_commit_msg as ccm


class SubjectTests(unittest.TestCase):
    def test_first_effective_line_is_subject(self):
        message = "# 注释行\n\nfeat(core): 拆出存储层路径解析\n\n正文\n"
        self.assertEqual(ccm.subject_of(message), "feat(core): 拆出存储层路径解析")

    def test_comment_lines_skipped(self):
        message = "\n# 请输入提交信息\n# 以 # 开头的行为注释\nfix(core): 修掉路径解析的回退分支\n"
        self.assertEqual(ccm.subject_of(message), "fix(core): 修掉路径解析的回退分支")

    def test_cut_marker_stops_parsing(self):
        message = "fix(core): 修掉路径解析的回退分支\n\n# ------------------------ >8 ------------------------\nfeat(core): 这一段不是标题\n"
        self.assertEqual(ccm.subject_of(message), "fix(core): 修掉路径解析的回退分支")

    def test_no_subject(self):
        self.assertEqual(ccm.subject_of("# 全是注释\n"), "")
        self.assertEqual(ccm.validate(""), ["标题为空"])


class TitleRuleTests(unittest.TestCase):
    def assert_ok(self, subject):
        self.assertEqual(ccm.validate(subject), [], f"不应判为违规：{subject}")

    def assert_rejected(self, subject, keyword):
        problems = ccm.validate(subject)
        self.assertTrue(problems, f"应判为违规：{subject}")
        self.assertIn(keyword, "；".join(problems))

    def test_compliant_titles(self):
        for subject in (
            "feat(sync): 支持勾选数据集导出加密数据包",
            "feat(sync): 导入数据包时经隔离空间写入并校验",
            "fix(ssh): 取消连接后不再残留会话页签",
            "docs(core): 回填数据导出导入的实现记录与人工验证项",
            "chore(core): 把 lint 与格式检查并成一条命令",
        ):
            self.assert_ok(subject)

    def test_batch_number_rejected(self):
        self.assert_rejected("docs(core): 明确存储迁移策略 rel-202609-001", "批次号或任务号")

    def test_item_numbers_rejected(self):
        for subject in (
            "feat(sync): 落 L2 导出链路",
            "feat(sync): 补齐 C1 的包容器实现",
            "feat(ssh): 新建记录改用 UUIDv4（D3）",
            "docs(core): 登记 M15 到人工清单",
            "docs(core): 收口剩余项计划与台账的 P3 状态",
            "feat(core): 按 T11 口径收口错误信封",
        ):
            self.assert_rejected(subject, "编号")

    def test_section_number_rejected(self):
        self.assert_rejected("docs(sync): 按 §8.1 更新门禁行", "文档章节号")

    def test_parentheses_rejected(self):
        self.assert_rejected("feat(ssh): 标识改用 UUIDv4（跨机器不撞号）", "括号")

    def test_dash_rejected(self):
        self.assert_rejected("feat(sync): 落导出链路——适配器契约与导出目录", "破折号")
        self.assert_rejected("feat(sync): 落导出链路--适配器契约", "连续横杠")

    def test_vague_description_rejected(self):
        self.assert_rejected("docs(core): 完成整改", "笼统过程描述")
        self.assert_rejected("docs(core): 补充若干内容", "笼统过程描述")
        self.assert_rejected("docs(core): 更新清单", "笼统过程描述")

    def test_format_violations(self):
        self.assert_rejected("落导出链路", "格式")
        self.assert_rejected("feat: 支持导出加密数据包", "格式")
        self.assert_rejected("feat(Sync): 支持导出加密数据包", "scope")
        self.assert_rejected("feat(L2): 支持导出加密数据包", "scope")
        self.assert_rejected("feat(sync): 支持导出加密数据包（可选）", "括号")
        # scope 里塞批次号同样被拦（命中批次号规则，提示比 scope 形态更具体）
        self.assert_rejected("feat(sync-202609-001): 支持导出加密数据包", "批次号或任务号")

    def test_unknown_type_rejected(self):
        self.assert_rejected("style(core): 修掉未格式化文件", "类型")
        self.assert_rejected("feature(core): 支持导出加密数据包", "类型")

    def test_technical_identifiers_allowed(self):
        for subject in (
            "fix(ssh): 修掉 AES-256-GCM 与 Tauri 2 下 sha256 校验误报",
            "fix(core): 拉取失败时按 HTTP 404 提示资源不存在",
            "docs(core): 说明 x86-64 与 arm64 的构建差异",
            "feat(ssh): 新建记录改用 UUIDv4 标识",
            "fix(core): 修掉 Windows 下路径解析成的 C:\\g 前缀",
        ):
            self.assert_ok(subject)

    def test_git_generated_titles_exempt(self):
        for subject in (
            'Merge branch "main" into feature-x',
            'Revert "feat(sync): 支持勾选数据集导出加密数据包"',
            "fixup! feat(sync): 支持勾选数据集导出加密数据包",
            "squash! feat(sync): 支持勾选数据集导出加密数据包",
        ):
            self.assert_ok(subject)


class EntryTests(unittest.TestCase):
    def run_main(self, argv):
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer), contextlib.redirect_stderr(buffer):
            code = ccm.main(argv)
        return code, buffer.getvalue()

    def test_subject_flag_reports_violation(self):
        code, output = self.run_main(["--subject", "feat(sync): 落 L2 导出链路"])
        self.assertEqual(code, 1)
        self.assertIn("L2", output)
        self.assertIn("21-Git提交规范", output)

    def test_subject_flag_passes(self):
        code, output = self.run_main(["--subject", "feat(sync): 支持勾选数据集导出加密数据包"])
        self.assertEqual(code, 0)
        self.assertIn("合规", output)

    def test_message_file_checked(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "COMMIT_EDITMSG"
            path.write_text("feat(sync): 落 L2 导出链路\n\n正文\n", encoding="utf-8")
            code, output = self.run_main([str(path)])
        self.assertEqual(code, 1)
        self.assertIn("L2", output)

    def test_missing_file_is_usage_error(self):
        code, _ = self.run_main([str(Path(tempfile.gettempdir()) / "pb_no_such_commit_msg")])
        self.assertEqual(code, 2)

    def test_no_argument_is_usage_error(self):
        code, _ = self.run_main([])
        self.assertEqual(code, 2)


if __name__ == "__main__":
    unittest.main()
