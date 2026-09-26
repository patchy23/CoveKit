//! 数据包 JSON 的流式输出：复用序列化器，同时在写入前执行已有字节上限。

use std::io::{self, Write};

/// 装饰任意写入端；拒绝会越过既有上限的片段，不先分配超额正文。
pub(super) struct LimitedWriter<W> {
    inner: W,
    remaining: usize,
    exceeded: bool,
}

impl<W> LimitedWriter<W> {
    /// 上限来自容器或记录既有契约，不调整用户可导出的范围。
    pub(super) fn new(inner: W, limit: usize) -> Self {
        Self {
            inner,
            remaining: limit,
            exceeded: false,
        }
    }

    /// 区分超限与接收端自身的 I/O 失败。
    pub(super) fn exceeded(&self) -> bool {
        self.exceeded
    }

    /// 取出已写入的正文或原接收端。
    pub(super) fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            self.exceeded = true;
            return Err(io::Error::other("JSON 超过既有字节上限"));
        }
        let written = self.inner.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_boundary_counts_utf8_and_escaping_without_changing_bytes() {
        let value = serde_json::json!({"text": "中文😀\n\"", "items": [1, true, null]});
        let expected = serde_json::to_vec(&value).unwrap();
        let mut exact = LimitedWriter::new(Vec::new(), expected.len());
        serde_json::to_writer(&mut exact, &value).unwrap();
        assert!(!exact.exceeded());
        assert_eq!(exact.into_inner(), expected);
        let mut short = LimitedWriter::new(Vec::new(), expected.len() - 1);
        assert!(serde_json::to_writer(&mut short, &value).is_err());
        assert!(short.exceeded());
        assert!(short.into_inner().len() < expected.len());
    }

    #[test]
    fn oversized_fragment_is_rejected_before_inner_allocation() {
        let mut writer = LimitedWriter::new(Vec::new(), 16);
        assert!(writer.write_all(&[b'x'; 4096]).is_err());
        let inner = writer.into_inner();
        assert_eq!(inner.capacity(), 0);
    }
}
