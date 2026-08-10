//! TTS 插件 · 合成能力（Edge TTS websocket 协议实现）
//! 协议：wss://speech.platform.bing.com 的 readaloud websocket，
//! 发 speech.config + SSML 两段 JSON，收音频二进制分片（mp3）。
//! 鉴权：TrustedClientToken（公开常量）+ Sec-MS-GEC（Windows 文件时间签名）。
//! 消息格式：2 字节前缀 + 头部行（CRLF 分隔，含 Path:audio 标记行）+ MP3 数据。

use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use sha2::Digest;

/// Edge TTS 公开客户端令牌（微软 readaloud 服务固定常量）
const TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
/// Edge TTS 服务端点
const EDGE_TTS_URL: &str =
    "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1";
/// 浏览器版本（Sec-MS-GEC-Version，与 edge-tts 库同步）
const SEC_MS_GEC_VERSION: &str = "1-143.0.3650.75";
/// Windows 文件时间纪元偏移（1601-01-01 → 1970-01-01，秒）
const WIN_EPOCH: u64 = 11_644_473_600;
/// 秒 → 100ns 间隔
const TICKS_PER_SEC: u64 = 10_000_000;

/// 生成 Sec-MS-GEC 签名（edge-tts 同款算法）：
/// 1) unix 秒 → Windows 文件时间（+WIN_EPOCH），向下取整到 5 分钟
/// 2) 转 100ns 间隔 → 拼接 TOKEN → SHA256 hex 大写（非 HMAC）
fn sec_ms_gec() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ticks = (now + WIN_EPOCH) - (now + WIN_EPOCH) % 300;
    let ticks_ns = ticks * TICKS_PER_SEC;
    let str_to_hash = format!("{ticks_ns}{TRUSTED_CLIENT_TOKEN}");
    let digest = sha2::Sha256::digest(str_to_hash.as_bytes());
    digest.iter().map(|b| format!("{b:02X}")).collect()
}

/// 内置语音列表（中文优先 + 常用英文）
pub(crate) fn builtin_voices() -> Vec<super::TtsVoice> {
    vec![
        // 中文（普通话）
        super::TtsVoice {
            name: "zh-CN-XiaoxiaoNeural".into(),
            label: "晓晓 · 女声（温暖）".into(),
            lang: "zh-CN".into(),
        },
        super::TtsVoice {
            name: "zh-CN-YunxiNeural".into(),
            label: "云希 · 男声（阳光）".into(),
            lang: "zh-CN".into(),
        },
        super::TtsVoice {
            name: "zh-CN-YunyangNeural".into(),
            label: "云扬 · 男声（新闻）".into(),
            lang: "zh-CN".into(),
        },
        super::TtsVoice {
            name: "zh-CN-XiaoyiNeural".into(),
            label: "晓伊 · 女声（活泼）".into(),
            lang: "zh-CN".into(),
        },
        super::TtsVoice {
            name: "zh-CN-YunjianNeural".into(),
            label: "云健 · 男声（浑厚）".into(),
            lang: "zh-CN".into(),
        },
        super::TtsVoice {
            name: "zh-CN-liaoning-XiaobeiNeural".into(),
            label: "晓北 · 东北话女声".into(),
            lang: "zh-CN".into(),
        },
        // 粤语 / 台湾
        super::TtsVoice {
            name: "zh-HK-HiuMaanNeural".into(),
            label: "曉曼 · 粤语女声".into(),
            lang: "zh-HK".into(),
        },
        super::TtsVoice {
            name: "zh-TW-HsiaoChenNeural".into(),
            label: "曉臻 · 台湾女声".into(),
            lang: "zh-TW".into(),
        },
        // 常用英文
        super::TtsVoice {
            name: "en-US-JennyNeural".into(),
            label: "Jenny · 美式女声".into(),
            lang: "en-US".into(),
        },
        super::TtsVoice {
            name: "en-US-GuyNeural".into(),
            label: "Guy · 美式男声".into(),
            lang: "en-US".into(),
        },
        super::TtsVoice {
            name: "en-GB-SoniaNeural".into(),
            label: "Sonia · 英式女声".into(),
            lang: "en-GB".into(),
        },
    ]
}

/// 合成语音（纯网络部分，可独立测试）：文本 + 语音 + 语速/音调 → mp3 字节
pub(crate) async fn synth_bytes(
    text: &str,
    voice: &str,
    rate: Option<i32>,
    pitch: Option<i32>,
) -> Result<Vec<u8>, String> {
    // rustls 进程级 CryptoProvider（ring 后端，与 russh 一致；幂等）
    let _ = rustls::crypto::ring::default_provider().install_default();

    if text.trim().is_empty() {
        return Err("请输入要合成的文字".into());
    }
    if text.len() > 2000 {
        return Err("文本过长（最多 2000 字）".into());
    }

    // 组装 wss URL（鉴权参数；ConnectionId 要求 32 位 hex UUID 格式）
    let mut conn_bytes = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut conn_bytes);
    let connection_id: String = conn_bytes.iter().map(|b| format!("{b:02x}")).collect();
    let url = format!(
        "{EDGE_TTS_URL}?TrustedClientToken={TRUSTED_CLIENT_TOKEN}&Sec-MS-GEC={}&Sec-MS-GEC-Version={SEC_MS_GEC_VERSION}&ConnectionId={}",
        sec_ms_gec(),
        connection_id
    );

    // Edge 服务要求浏览器 UA + Origin（朗读扩展）等头，否则 403
    let mut request =
        tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(url)
            .map_err(|e| format!("构造请求失败: {e}"))?;
    {
        let headers = request.headers_mut();
        headers.insert(
            "User-Agent",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
            ),
        );
        // Origin 必须是 Edge 朗读扩展（服务端校验）
        headers.insert(
            "Origin",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static(
                "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold",
            ),
        );
        headers.insert(
            "Pragma",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static("no-cache"),
        );
        headers.insert(
            "Cache-Control",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static("no-cache"),
        );
        headers.insert(
            "Accept-Language",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(
            "Accept-Encoding",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static(
                "gzip, deflate, br, zstd",
            ),
        );
        // muid cookie（32 位大写 hex）
        let muid: String = (0..16)
            .map(|_| {
                let b = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0) as u8;
                let n = b % 16;
                char::from(if n < 10 { b'0' + n } else { b'A' + n - 10 })
            })
            .collect();
        headers.insert(
            "Cookie",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_str(&format!("muid={muid};"))
                .map_err(|e| format!("Cookie 构造失败: {e}"))?,
        );
    }

    let (mut ws, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| format!("连接语音服务失败: {e}"))?;

    // 1) speech.config（音频格式：24kHz 48kbps mp3）
    let config_msg = serde_json::json!({
        "context": {
            "synthesis": {
                "audio": { "metadataoptions": { "sentenceBoundaryEnabled": "false", "wordBoundaryEnabled": "false" }, "outputFormat": "audio-24khz-48kbitrate-mono-mp3" }
            }
        },
        "message": { "locale": "zh-CN", "format": "audio-24khz-48kbitrate-mono-mp3" }
    })
    .to_string();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        format!("X-Timestamp:{}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n{}", now_iso(), config_msg),
    ))
    .await
    .map_err(|e| format!("发送配置失败: {e}"))?;

    // 2) SSML 消息
    let rate_str = format!("{:+}%", rate.unwrap_or(0));
    let pitch_str = format!("{:+}Hz", pitch.unwrap_or(0));
    let ssml = format!(
        "<speak version='1.0' xml:lang='zh-CN'><voice name='{}'><prosody pitch='{}' rate='{}'>{}</prosody></voice></speak>",
        voice,
        pitch_str,
        rate_str,
        xml_escape(text)
    );
    ws.send(tokio_tungstenite::tungstenite::Message::Text(format!(
        "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nPath:ssml\r\n\r\n{}",
        connection_id, ssml
    )))
    .await
    .map_err(|e| format!("发送文本失败: {e}"))?;

    // 3) 收音频分片，直到 TurnEnd（消息 = 2 字节前缀 + 头部行 + MP3 数据）
    let mut audio: Vec<u8> = Vec::new();
    loop {
        let Some(msg) = ws.next().await else {
            break;
        };
        let msg = msg.map_err(|e| format!("接收音频失败: {e}"))?;
        match msg {
            tokio_tungstenite::tungstenite::Message::Binary(data) => {
                // 头部行以 Path:audio 标记行为结尾，其后为 MP3 数据（[13,10] = CRLF）
                if let Some(p) = data.windows(10).position(|w| w == b"Path:audio") {
                    let mut s = p + 10;
                    if data.get(s) == Some(&13) {
                        s += 1;
                    }
                    if data.get(s) == Some(&10) {
                        s += 1;
                    }
                    audio.extend_from_slice(&data[s..]);
                } else {
                    // 无 Path:audio 标记：按纯数据块追加（理论上不会出现）
                    audio.extend_from_slice(&data);
                }
            }
            tokio_tungstenite::tungstenite::Message::Text(t) => {
                if t.contains("Path:turn.end") {
                    break;
                }
            }
            tokio_tungstenite::tungstenite::Message::Close(_) => break,
            _ => {}
        }
    }

    Ok(audio)
}

/// 当前时间（RFC 3339 格式，用于 X-Timestamp 头）
fn now_iso() -> String {
    // 简化：unix 秒转 UTC ISO8601（Edge 服务不校验严格格式）
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let rem = secs % 86400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// 天数 → 公历日期（Howard Hinnant 算法）
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// XML 转义（SSML 文本安全）
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sec_ms_gec_format() {
        let gec = sec_ms_gec();
        // 纯 SHA256 hex 大写（64 位，无分隔符）
        assert_eq!(gec.len(), 64, "SHA256 hex 应为 64 位");
        assert!(gec.chars().all(|c| c.is_ascii_hexdigit()), "应为 hex");
        assert!(gec.chars().any(|c| c.is_ascii_uppercase()), "应为大写");
    }

    #[test]
    fn xml_escape_special_chars() {
        assert_eq!(xml_escape("<a & b>"), "&lt;a &amp; b&gt;");
    }

    #[test]
    fn civil_from_days_epoch() {
        // 1970-01-01
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2026-08-09（与 Python date 交叉验证）
        let days = 20674;
        let (y, m, d) = civil_from_days(days);
        assert_eq!((y, m, d), (2026, 8, 9));
    }

    #[tokio::test]
    #[ignore = "需要网络（Edge TTS 服务）"]
    async fn live_synth_chinese() {
        // 真实合成：中文文本 + 晓晓语音 → mp3 字节（验证协议与鉴权）
        let audio = synth_bytes(
            "你好，这是 patchyBox 的文字转语音测试。",
            "zh-CN-XiaoxiaoNeural",
            None,
            None,
        )
        .await
        .expect("合成失败");
        assert!(!audio.is_empty(), "音频为空");
        // MP3 帧头（0xFF 0xFB）或 ID3 头（"ID3"）
        let is_mp3 = audio.starts_with(b"ID3") || (audio[0] == 0xFF && audio[1] & 0xE0 == 0xE0);
        assert!(
            is_mp3,
            "音频格式异常，前 4 字节: {:?}",
            &audio[..4.min(audio.len())]
        );
        assert!(audio.len() > 1000, "音频过短: {} 字节", audio.len());
    }
}
