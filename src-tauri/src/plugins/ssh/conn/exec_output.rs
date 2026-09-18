//! 非交互命令输出收集：EOF 只结束数据流，成功必须由退出码与通道关闭共同确认。

use russh::ChannelMsg;

#[derive(Default)]
pub(super) struct ExecOutput {
    output: String,
    status: Option<u32>,
    signaled: bool,
    closed: bool,
}

impl ExecOutput {
    /// 返回 true 表示服务端已关闭通道，可以结束读取；EOF 不代表命令成功。
    pub(super) fn receive(&mut self, message: ChannelMsg) -> Result<bool, String> {
        match message {
            ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                if self.output.len().saturating_add(data.len()) > 16 * 1024 * 1024 {
                    return Err("远程命令输出超过 16 MiB 安全上限".into());
                }
                self.output.push_str(&String::from_utf8_lossy(&data));
            }
            ChannelMsg::ExitStatus { exit_status } => self.status = Some(exit_status),
            ChannelMsg::ExitSignal { .. } => self.signaled = true,
            ChannelMsg::Close => self.closed = true,
            _ => {}
        }
        Ok(self.closed)
    }

    /// 缺少退出状态、信号终止及传输中断均不能伪装成执行成功。
    pub(super) fn finish(self) -> Result<String, String> {
        if !self.closed {
            return Err("连接中断，命令未完整执行，请检查连接后重试".into());
        }
        if self.signaled {
            return Err("远程命令被信号终止".into());
        }
        match self.status {
            Some(0) => Ok(self.output),
            Some(status) => Err(if self.output.trim().is_empty() {
                format!("远程命令失败（退出码 {status}）")
            } else {
                format!("远程命令失败（退出码 {status}）：{}", self.output.trim())
            }),
            None => Err("远程命令未返回退出状态，无法确认执行结果".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eof_does_not_hide_a_later_failed_exit_status() {
        let mut output = ExecOutput::default();
        assert!(!output.receive(ChannelMsg::Eof).unwrap());
        assert!(!output
            .receive(ChannelMsg::ExitStatus { exit_status: 1 })
            .unwrap());
        assert!(output.receive(ChannelMsg::Close).unwrap());
        assert!(output.finish().unwrap_err().contains("退出码 1"));
    }

    #[test]
    fn successful_status_is_accepted_before_or_after_eof() {
        for messages in [
            vec![ChannelMsg::Eof, ChannelMsg::ExitStatus { exit_status: 0 }],
            vec![ChannelMsg::ExitStatus { exit_status: 0 }, ChannelMsg::Eof],
        ] {
            let mut output = ExecOutput::default();
            for message in messages {
                assert!(!output.receive(message).unwrap());
            }
            assert!(output.receive(ChannelMsg::Close).unwrap());
            assert_eq!(output.finish().unwrap(), "");
        }
    }

    #[test]
    fn missing_status_or_interrupted_transport_is_not_success() {
        let mut closed = ExecOutput::default();
        closed.receive(ChannelMsg::Eof).unwrap();
        closed.receive(ChannelMsg::Close).unwrap();
        assert!(closed.finish().unwrap_err().contains("未返回退出状态"));
        let mut interrupted = ExecOutput::default();
        interrupted
            .receive(ChannelMsg::ExitStatus { exit_status: 0 })
            .unwrap();
        assert!(interrupted.finish().unwrap_err().contains("连接中断"));
    }
}
