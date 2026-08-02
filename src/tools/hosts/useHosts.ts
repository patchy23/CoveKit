/**
 * hosts 修改 · 纯函数（行解析 + 语法校验 + 条目化（列表模式））
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

/** 列表模式的条目（映射行 + 被禁用的映射行；非映射行以 raw 保留原位） */
export interface HostsEntry {
  id: string;
  /** 是否启用（禁用 = 行首加 #） */
  enabled: boolean;
  ip: string;
  hosts: string[];
  comment: string;
  /** 非映射行（空行/纯注释）原样内容；映射行无此字段 */
  raw?: string;
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

/* ── 列表模式：条目解析 / 重组 ── */

/** 解析 hosts 内容为条目列表（保留全部行；映射行可编辑，其余行 raw 原样保留） */
export function parseEntries(content: string): HostsEntry[] {
  return content.split("\n").map((raw, i) => {
    const line = raw.trim();
    const id = `l${i}`;
    // 空行
    if (!line) return { id, enabled: true, ip: "", hosts: [], comment: "", raw, valid: true };
    // 注释行：形如 "# 127.0.0.1 example.com" 视为被禁用的映射
    if (line.startsWith("#")) {
      const inner = line.slice(1).trim();
      const parts = inner.split(/\s+/).filter(Boolean);
      if (isValidIp(parts[0] ?? "") && parts.length > 1) {
        return {
          id,
          enabled: false,
          ip: parts[0],
          hosts: parts.slice(1),
          comment: "",
          valid: true,
        };
      }
      // 纯注释行
      return { id, enabled: true, ip: "", hosts: [], comment: line, raw, valid: true };
    }
    // 映射行（含非法行，保留供用户修正）
    let body = line;
    let comment = "";
    const ci = line.indexOf("#");
    if (ci >= 0) {
      body = line.slice(0, ci).trim();
      comment = line.slice(ci);
    }
    const parts = body.split(/\s+/).filter(Boolean);
    const ip = parts[0] ?? "";
    const hosts = parts.slice(1);
    let valid = true;
    let error: string | undefined;
    if (!isValidIp(ip)) {
      valid = false;
      error = "IP 地址格式无效";
    } else if (hosts.length === 0) {
      valid = false;
      error = "缺少主机名";
    } else {
      const bad = hosts.find((h) => !isValidHostname(h));
      if (bad) {
        valid = false;
        error = `主机名格式无效: ${bad}`;
      }
    }
    return { id, enabled: true, ip, hosts, comment, valid, error };
  });
}

/** 条目列表重组为 hosts 文本（映射行生成，非映射行原样） */
export function entriesToText(entries: HostsEntry[]): string {
  return entries
    .map((e) => {
      if (e.raw !== undefined) return e.raw;
      const hostStr = e.hosts.join(" ");
      let line = `${e.ip} ${hostStr}`.trimEnd();
      if (!e.enabled) line = `# ${line}`;
      if (e.comment.trim()) line += ` ${e.comment.trim()}`;
      return line;
    })
    .join("\n");
}

/** 校验单个条目（列表模式行内即时校验） */
export function validateEntry(ip: string, hosts: string[]): { valid: boolean; error?: string } {
  if (!isValidIp(ip)) return { valid: false, error: "IP 无效" };
  if (hosts.length === 0) return { valid: false, error: "缺少主机名" };
  const bad = hosts.find((h) => !isValidHostname(h));
  if (bad) return { valid: false, error: `主机名无效: ${bad}` };
  return { valid: true };
}
