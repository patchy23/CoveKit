/**
 * hosts 修改 · 纯函数（行解析 + 语法校验）
 */

export interface HostsLine {
  /** 原始行（含换行前的原样内容） */
  raw: string;
  /** 行内注释（# 开头或行尾 # 后内容） */
  comment: string;
  ip?: string;
  hosts: string[];
  valid: boolean;
  error?: string;
}

/** IPv4 校验（四段 0-255） */
export function isValidIp(ip: string): boolean {
  const parts = ip.split(".");
  return parts.length === 4 && parts.every((p) => /^\d{1,3}$/.test(p) && Number(p) <= 255);
}

/** 主机名校验（字母数字、点、连字符） */
export function isValidHostname(h: string): boolean {
  return /^[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)*$/.test(
    h
  );
}

/** 解析 hosts 内容为行记录（空行/注释行 = valid） */
export function parseHostsLines(content: string): HostsLine[] {
  return content.split("\n").map((raw) => {
    const line = raw.trim();
    if (!line || line.startsWith("#")) {
      return { raw, comment: line, valid: true, hosts: [] };
    }
    // 去行尾注释
    let body = line;
    let comment = "";
    const ci = line.indexOf("#");
    if (ci >= 0) {
      body = line.slice(0, ci).trim();
      comment = line.slice(ci);
    }
    const parts = body.split(/\s+/).filter(Boolean);
    const ip = parts[0];
    const hosts = parts.slice(1);
    if (!isValidIp(ip ?? "")) {
      return { raw, comment, ip, hosts, valid: false, error: "IP 地址格式无效" };
    }
    if (hosts.length === 0) {
      return { raw, comment, ip, hosts, valid: false, error: "缺少主机名" };
    }
    const bad = hosts.find((h) => !isValidHostname(h));
    if (bad) {
      return { raw, comment, ip, hosts, valid: false, error: `主机名格式无效: ${bad}` };
    }
    return { raw, comment, ip, hosts, valid: true };
  });
}

/** 错误行数 */
export function countErrors(lines: HostsLine[]): number {
  return lines.filter((l) => !l.valid).length;
}

/** 有效的映射条数（非注释行） */
export function countMappings(lines: HostsLine[]): number {
  return lines.filter((l) => l.valid && l.ip !== undefined).length;
}
