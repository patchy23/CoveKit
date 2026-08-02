/**
 * 二维码 · 纯函数封装（qrcode 库浏览器 API）
 */
import QRCode from "qrcode";

export interface QrOptions {
  width?: number;
  margin?: number;
  errorCorrectionLevel?: "L" | "M" | "Q" | "H";
}

/** 生成二维码 PNG dataURL（默认 240px，容错 M） */
export function renderQrDataUrl(text: string, opts: QrOptions = {}): Promise<string> {
  return QRCode.toDataURL(text || " ", {
    width: opts.width ?? 240,
    margin: opts.margin ?? 2,
    errorCorrectionLevel: opts.errorCorrectionLevel ?? "M",
  });
}

/** 输入长度对容错级别的建议（内容越长越需要低容错保证可扫） */
export function suggestLevel(text: string): Exclude<QrOptions["errorCorrectionLevel"], undefined> {
  const len = text.length;
  if (len > 700) return "L";
  if (len > 400) return "M";
  if (len > 200) return "Q";
  return "H";
}
