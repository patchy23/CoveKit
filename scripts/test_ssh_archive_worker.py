"""归档工作器的隔离轻量测试，不访问 SSH 或真实用户文件。"""
import contextlib
import gzip
import importlib.util
import io
import json
import os
import subprocess
import sys
from pathlib import Path
import tempfile
import unittest
import zipfile

WORKER = Path(__file__).resolve().parents[1] / "src-tauri/src/plugins/ssh/archive/worker.py"
spec = importlib.util.spec_from_file_location("ssh_archive_worker", WORKER)
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class ArchiveWorkerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.source = self.root / "输入 空格.txt"
        self.content = ("abc中文\n" * 100000).encode()
        self.source.write_bytes(self.content)
        self.addCleanup(self.temp.cleanup)

    def request(self, fmt, operation="compress"):
        return dict(format=fmt, operation=operation, paths=[str(self.source)], output=str(self.root / ("包." + fmt)))

    def test_三种格式往返保留原文(self):
        for fmt in ("zip", "gz", "tar.gz"):
            request = self.request(fmt)
            stage = self.root / ("stage." + fmt)
            with contextlib.redirect_stdout(io.StringIO()):
                worker.compress(request, str(stage))
                destination = self.root / ("out-" + fmt)
                if fmt != "gz":
                    destination.mkdir()
                worker.read_archive(dict(request, operation="extract", paths=[str(stage)]), str(destination))
            restored = destination if fmt == "gz" else destination / self.source.name
            self.assertEqual(restored.read_bytes(), self.content)
            self.assertEqual(self.source.read_bytes(), self.content)

    def test_预览只返回目录且不解压(self):
        package = self.root / "test.zip"
        with zipfile.ZipFile(package, "w") as archive:
            archive.writestr("a/b.txt", "hello")
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            worker.read_archive(dict(self.request("zip"), operation="preview", paths=[str(package)]), None)
        entries = [entry for line in output.getvalue().splitlines() for entry in json.loads(line).get("entries", [])]
        self.assertEqual(entries[0]["path"], "a/b.txt")
        self.assertFalse((self.root / "a").exists())

    def test_拒绝越界及链接(self):
        for name in ("../outside", "/absolute", "a/../../outside", "C:/outside", "a" + chr(92) + "b"):
            with self.assertRaises(ValueError):
                worker.safe_name(name)
        package = self.root / "bad.zip"
        link = zipfile.ZipInfo("link")
        link.external_attr = 0o120777 << 16
        with zipfile.ZipFile(package, "w") as archive:
            archive.writestr(link, "../outside")
        with self.assertRaises(ValueError), contextlib.redirect_stdout(io.StringIO()):
            worker.read_archive(dict(self.request("zip"), operation="extract", paths=[str(package)]), str(self.root / "extract"))

    def test_压缩输出不能进入输入目录(self):
        request = dict(self.request("zip"), paths=[str(self.root)])
        with self.assertRaises(ValueError), contextlib.redirect_stdout(io.StringIO()):
            worker.compress(request, str(self.root / "stage"))

    def test_文件提交不覆盖既有目标(self):
        stage = self.root / "stage"
        stage.write_bytes(b"new")
        with self.assertRaises(FileExistsError):
            worker.publish(str(stage), str(self.source), False)
        self.assertEqual(self.source.read_bytes(), self.content)
        self.assertTrue(stage.exists())

    @unittest.skipUnless(hasattr(os, "fork"), "进程监督需 POSIX；Windows 本地仅验证归档读写逻辑")
    def test_取消等待远端子进程结束并清理临时产物(self):
        source = self.root / "large.bin"
        source.write_bytes(os.urandom(12 * 1024 * 1024))
        request = dict(self.request("zip"), paths=[str(source)])
        payload = (json.dumps(request) + chr(10) + "cancel" + chr(10)).encode()
        result = subprocess.run([sys.executable, str(WORKER)], input=payload, capture_output=True, timeout=12)
        events = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(any(event.get("status") == "cancelled" for event in events), events)
        self.assertFalse(Path(request["output"]).exists())
        self.assertEqual(source.stat().st_size, 12 * 1024 * 1024)
        self.assertFalse(list(self.root.glob(".covekit-archive-*")))

    def test_gz预览不伪造大文件原始长度(self):
        package = self.root / "file.gz"
        with gzip.open(package, "wb") as target:
            target.write(b"abc")
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            worker.read_archive(dict(self.request("gz"), operation="preview", paths=[str(package)]), None)
        entries = [entry for line in output.getvalue().splitlines() for entry in json.loads(line).get("entries", [])]
        self.assertIsNone(entries[0]["size"])


if __name__ == "__main__":
    unittest.main()
