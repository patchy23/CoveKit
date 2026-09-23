"""远端归档工作器。只依赖 Python 3.8+ 标准库；监督进程负责取消、提交与临时目录清理。"""
import ctypes
import gzip
import json
import os
import select
import shutil
import signal
import stat
import struct
import sys
import tarfile
import tempfile
import time
import zipfile

LIMIT = 100000
CHUNK = 256 * 1024


def emit(kind, **values):
    print(json.dumps(dict(kind=kind, **values), ensure_ascii=True), flush=True)


def safe_name(name):
    # 归档路径统一按 POSIX 解释；反斜杠拒绝，避免跨平台提取语义不同。
    if not name or name.startswith("/") or "\\" in name or "\x00" in name:
        raise ValueError("归档包含不支持的路径：" + repr(name))
    while name.startswith("./"):
        name = name[2:]
    parts = name.rstrip("/").split("/")
    if any(part in ("", ".", "..") for part in parts) or ":" in parts[0]:
        raise ValueError("归档路径越界：" + repr(name))
    return "/".join(parts)


def inspect_zip(path):
    # 在 ZipFile 构建内存索引之前限制中心目录大小，同时支持 ZIP64 大文件。
    with open(path, "rb") as source:
        source.seek(0, 2)
        size = source.tell()
        source.seek(max(0, size - 65557))
        tail = source.read()
        offset = tail.rfind(b"PK\x05\x06")
        if offset < 0 or len(tail) - offset < 22:
            raise ValueError("ZIP 中心目录损坏")
        record = struct.unpack("<4s4H2LH", tail[offset:offset + 22])
        count, length = record[4], record[5]
        if record[1] or record[2]:
            raise ValueError("暂不支持分卷 ZIP")
        if count == 65535 or length == 0xffffffff:
            end_offset = size - len(tail) + offset
            source.seek(end_offset - 20)
            locator = source.read(20)
            if locator[:4] != b"PK\x06\x07":
                raise ValueError("ZIP64 索引损坏")
            index_offset = struct.unpack("<4sLQL", locator)[2]
            source.seek(index_offset)
            record64 = source.read(56)
            if len(record64) != 56 or record64[:4] != b"PK\x06\x06":
                raise ValueError("ZIP64 索引损坏")
            count, length = struct.unpack_from("<QQ", record64, 32)
        if count > LIMIT or length > 64 * 1024 * 1024:
            raise ValueError("ZIP 目录超过预览/处理上限：10 万条或 64 MiB 索引")


class Progress:
    def __init__(self):
        self.done = 0
        self.total = 0
        self.last = 0

    def report(self, force=False):
        now = time.monotonic()
        if force or now - self.last >= 0.2:
            emit("progress", transferred=self.done, total=self.total)
            self.last = now

    def copy(self, source, target):
        while True:
            data = source.read(CHUNK)
            if not data:
                return
            target.write(data)
            self.done += len(data)
            self.report()


def source_entries(paths, output):
    entries = []
    for raw in paths:
        path = os.path.realpath(raw)
        root = os.path.dirname(path)
        if os.path.islink(raw):
            raise ValueError("暂不支持压缩符号链接")
        if output and (output == path or (os.path.isdir(path) and os.path.commonpath([path, output]) == path)):
            raise ValueError("输出不能位于待压缩目录内")
        pending = [path]
        while pending:
            current = pending.pop()
            info = os.lstat(current)
            if not (stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode)):
                raise ValueError("暂不支持链接或特殊文件：" + current)
            entries.append((current, safe_name(os.path.relpath(current, root)), info))
            if len(entries) > LIMIT:
                raise ValueError("文件数量超过 10 万条上限")
            if stat.S_ISDIR(info.st_mode):
                with os.scandir(current) as children:
                    for child in children:
                        pending.append(child.path)
                        if len(pending) + len(entries) > LIMIT:
                            raise ValueError("文件数量超过 10 万条上限")
    names = [entry[1] for entry in entries]
    if len(set(names)) != len(names):
        raise ValueError("选中项包含重叠目录或同名项目")
    return entries


def compress(request, stage):
    entries = source_entries(request["paths"], os.path.realpath(request["output"]))
    progress = Progress()
    progress.total = sum(info.st_size for _, _, info in entries if stat.S_ISREG(info.st_mode))
    progress.report(True)
    fmt = request["format"]
    if fmt == "gz":
        if len(entries) != 1 or not stat.S_ISREG(entries[0][2].st_mode):
            raise ValueError("GZ 仅支持单个普通文件，目录请选择 TAR.GZ")
        with open(entries[0][0], "rb") as source, gzip.open(stage, "wb") as target:
            progress.copy(source, target)
    elif fmt == "zip":
        with zipfile.ZipFile(stage, "w", compression=zipfile.ZIP_DEFLATED, allowZip64=True) as archive:
            for path, name, info in entries:
                entry = zipfile.ZipInfo.from_file(path, name)
                entry.compress_type = zipfile.ZIP_DEFLATED
                if stat.S_ISDIR(info.st_mode):
                    archive.writestr(entry, b"")
                else:
                    with open(path, "rb") as source, archive.open(entry, "w", force_zip64=True) as target:
                        progress.copy(source, target)
    elif fmt == "tar.gz":
        class Reader:
            def __init__(self, source):
                self.source = source
            def read(self, size):
                data = self.source.read(min(size, CHUNK))
                progress.done += len(data)
                progress.report()
                return data
        with tarfile.open(stage, "w:gz") as archive:
            for path, name, info in entries:
                entry = archive.gettarinfo(path, name)
                if entry.isfile():
                    with open(path, "rb") as source:
                        archive.addfile(entry, Reader(source))
                else:
                    archive.addfile(entry)
    else:
        raise ValueError("不支持的压缩格式")
    progress.report(True)


def read_archive(request, stage):
    path, fmt = request["paths"][0], request["format"]
    preview = request["operation"] == "preview"
    progress = Progress()
    batch = []
    count = 0

    def entry(name, size, directory, modified):
        nonlocal count
        name = safe_name(name)
        count += 1
        if count > LIMIT:
            raise ValueError("条目超过 10 万条上限，已停止扫描")
        if preview:
            batch.append(dict(path=name, size=size, isDir=directory, modifiedAt=modified))
            if len(batch) >= 200:
                emit("entries", entries=batch[:])
                batch.clear()
        return os.path.join(stage, *name.split("/")) if stage else None

    def extract(name, size, directory, modified, source):
        destination = entry(name, size, directory, modified)
        if preview:
            return
        if directory:
            os.makedirs(destination, exist_ok=True)
        else:
            os.makedirs(os.path.dirname(destination), exist_ok=True)
            with open(destination, "xb") as target:
                progress.copy(source, target)

    if fmt == "zip":
        inspect_zip(path)
        with zipfile.ZipFile(path) as archive:
            progress.total = sum(item.file_size for item in archive.infolist())
            for item in archive.infolist():
                mode = item.external_attr >> 16
                if (item.flag_bits & 1) or (stat.S_IFMT(mode) not in (0, stat.S_IFREG, stat.S_IFDIR)):
                    raise ValueError("暂不支持加密或含链接/特殊文件的 ZIP")
                timestamp = int(time.mktime(item.date_time + (0, 0, -1)) * 1000)
                if preview or item.is_dir():
                    extract(item.filename, item.file_size, item.is_dir(), timestamp, None)
                else:
                    with archive.open(item) as source:
                        extract(item.filename, item.file_size, False, timestamp, source)
    elif fmt == "tar.gz":
        # 流式模式不会先构建完整目录或把整个归档展开到磁盘。
        with tarfile.open(path, "r|gz") as archive:
            for item in archive:
                if item.isdir() and item.name in (".", "./"):
                    continue
                if not (item.isfile() or item.isdir()):
                    raise ValueError("暂不支持含链接或特殊文件的 TAR")
                source = None if preview or item.isdir() else archive.extractfile(item)
                try:
                    extract(item.name, item.size, item.isdir(), int(item.mtime * 1000), source)
                finally:
                    if source:
                        source.close()
    elif fmt == "gz":
        with open(path, "rb") as header:
            if header.read(2) != bytes([31, 139]):
                raise ValueError("不是有效的 GZ 文件")
        if preview:
            entry(os.path.basename(path)[:-3] or "解压文件", None, False, 0)
        else:
            with gzip.open(path, "rb") as source, open(stage, "xb") as target:
                progress.copy(source, target)
    else:
        raise ValueError("不支持的压缩格式")
    if batch:
        emit("entries", entries=batch)
    progress.report(True)


def publish(stage, target, directory):
    """同一文件系统提交且不覆盖已有目标。"""
    if not directory:
        os.link(stage, target)
        os.unlink(stage)
        return
    libc = ctypes.CDLL(None, use_errno=True)
    source, destination = os.fsencode(stage), os.fsencode(target)
    if sys.platform.startswith("linux") and hasattr(libc, "renameat2"):
        result = libc.renameat2(-100, ctypes.c_char_p(source), -100, ctypes.c_char_p(destination), 1)
    elif sys.platform == "darwin" and hasattr(libc, "renamex_np"):
        result = libc.renamex_np(ctypes.c_char_p(source), ctypes.c_char_p(destination), 4)
    else:
        raise ValueError("服务器不支持无覆盖的目录提交，未写入最终目标")
    if result:
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code), target)


def run_child(request, stage):
    try:
        if request["operation"] == "compress":
            compress(request, stage)
        else:
            before = os.stat(request["paths"][0])
            read_archive(request, stage)
            after = os.stat(request["paths"][0])
            if (before.st_size, before.st_mtime_ns, before.st_ino) != (after.st_size, after.st_mtime_ns, after.st_ino):
                raise ValueError("归档在读取期间发生变化，请重新打开")
        return 0
    except Exception as error:
        emit("error", error=str(error))
        return 1


def stop_child(pid):
    # pid 只来自本监督进程 fork，且未 wait 回收；不能误杀复用 PID。
    for sig, duration in ((signal.SIGTERM, 2), (signal.SIGKILL, 2)):
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            pass
        deadline = time.monotonic() + duration
        while time.monotonic() < deadline:
            finished, _ = os.waitpid(pid, os.WNOHANG)
            if finished:
                return
            time.sleep(0.02)
    raise RuntimeError("远端任务尚未停止，不能确认取消")


def supervise(request):
    if sys.version_info < (3, 8) or not hasattr(os, "fork"):
        raise ValueError("归档功能需要 Linux/macOS 服务器与 Python 3.8+")
    if request["operation"] not in ("compress", "extract", "preview"):
        raise ValueError("不支持的归档操作")
    paths = request["paths"]
    if not paths or len(paths) > LIMIT or any(not os.path.isabs(path) for path in paths):
        raise ValueError("请选择有效的远程绝对路径")
    preview = request["operation"] == "preview"
    if not preview and not os.path.isabs(request["output"]):
        raise ValueError("输出必须是远程绝对路径")
    directory = request["operation"] == "extract" and request["format"] != "gz"
    temporary = None
    stage = None
    pid = None
    try:
        if not preview:
            output = request["output"] = os.path.abspath(request["output"])
            if os.path.lexists(output):
                raise FileExistsError("目标已存在，请修改输出名称")
            temporary = tempfile.mkdtemp(prefix=".covekit-archive-", dir=os.path.dirname(output))
            stage = os.path.join(temporary, "content")
            if directory:
                os.mkdir(stage)
        emit("progress", transferred=0, total=0)
        pid = os.fork()
        if pid == 0:
            os._exit(run_child(request, stage))
        signal_cancel = [False]
        def request_cancel(_signal, _frame):
            signal_cancel[0] = True
        for sig in (signal.SIGHUP, signal.SIGTERM, signal.SIGINT):
            signal.signal(sig, request_cancel)
        last_heartbeat = time.monotonic()
        pending = b""
        while True:
            finished, status = os.waitpid(pid, os.WNOHANG)
            if finished:
                pid = None
                if not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                    emit("result", status="failed", error="归档处理失败，详情见任务错误")
                    return
                if not preview:
                    publish(stage, request["output"], directory)
                emit("result", status="succeeded")
                return
            ready, _, _ = select.select([sys.stdin], [], [], 0.1)
            cancelled = signal_cancel[0] or time.monotonic() - last_heartbeat > 10
            if ready:
                data = os.read(sys.stdin.fileno(), 4096)
                if not data:
                    cancelled = True
                pending += data
                while b"\n" in pending:
                    line, pending = pending.split(b"\n", 1)
                    if line == b"cancel":
                        cancelled = True
                    elif line == b"ping":
                        last_heartbeat = time.monotonic()
                if len(pending) > 4096:
                    cancelled = True
            if cancelled:
                stop_child(pid)
                pid = None
                emit("result", status="cancelled")
                return
    finally:
        if pid:
            stop_child(pid)
        if temporary:
            try:
                shutil.rmtree(temporary)
            except OSError as error:
                emit("error", error="临时文件清理失败：" + temporary + "：" + str(error))


def main():
    try:
        # 原始 os.read 避免 TextIO 预读吞掉紧随请求的心跳。
        data = bytearray()
        while not data.endswith(b"\n"):
            part = os.read(sys.stdin.fileno(), 1)
            if not part or len(data) > 1024 * 1024:
                raise ValueError("归档请求不完整或过大")
            data.extend(part)
        supervise(json.loads(data))
    except Exception as error:
        emit("result", status="failed", error=str(error))


if __name__ == "__main__":
    main()
