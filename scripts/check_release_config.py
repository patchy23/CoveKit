#!/usr/bin/env python3
"""发布配置校验（T08-4）

正式发布用的 `src-tauri/tauri.release.conf.json` 由 `pnpm release:config` 生成，
把 环境变量里的真实更新公钥覆盖进配置。这个脚本在打包前确认：

1. 覆盖文件存在（否则发布会带着仓库里的占位公钥）；
2. 公钥不是占位值、也不是空值；
3. 更新下载地址至少有一个。

校验失败返回非零退出码，发布前须修正，避免把「更新不可用」的安装包当成正式版本发出去。
"""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RELEASE_CONFIG = ROOT / "src-tauri" / "tauri.release.conf.json"
PLACEHOLDER_PUBKEY = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IHBsYWNlaG9sZGVyCg=="


def main() -> int:
    if not RELEASE_CONFIG.exists():
        print(f"发布配置缺失：{RELEASE_CONFIG.relative_to(ROOT)}（应先执行 pnpm release:config）")
        return 1
    config = json.loads(RELEASE_CONFIG.read_text(encoding="utf-8"))
    updater = config.get("plugins", {}).get("updater", {})
    pubkey = str(updater.get("pubkey", "")).strip()
    endpoints = [str(item).strip() for item in updater.get("endpoints", []) if str(item).strip()]

    errors: list[str] = []
    if not pubkey:
        errors.append("更新公钥为空")
    elif pubkey == PLACEHOLDER_PUBKEY:
        errors.append("更新公钥仍是占位值，环境变量 TAURI_UPDATER_PUBLIC_KEY 未配置")
    if not endpoints:
        errors.append("更新下载地址为空")
    if not config.get("bundle", {}).get("createUpdaterArtifacts"):
        errors.append("未开启 createUpdaterArtifacts，无法产出更新签名包")

    if errors:
        print("发布配置校验未通过：")
        for error in errors:
            print(f"  - {error}")
        return 1
    print(f"发布配置校验通过：{len(endpoints)} 个更新地址，公钥已覆盖占位值")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
