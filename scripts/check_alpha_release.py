"""Alpha 打包约束：只允许预发布，禁用更新产物与更新通道。"""
from __future__ import annotations

import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent


def validate(tag: str, config: dict) -> list[str]:
    errors = []
    if not re.fullmatch(r'v\d+\.\d+\.\d+-alpha\.[1-9]\d*', tag):
        errors.append('当前工作流只接受 vX.Y.Z-alpha.N 标签')
    if tag != 'v' + config.get('version', ''):
        errors.append('标签与安装包版本不一致')
    if config.get('bundle', {}).get('createUpdaterArtifacts') is not False:
        errors.append('Alpha 必须显式关闭更新产物')
    if 'updater' in config.get('plugins', {}):
        errors.append('Alpha 不得配置应用内更新通道')
    if config.get('bundle', {}).get('macOS', {}).get('signingIdentity') != '-':
        errors.append('当前 macOS Alpha 使用 ad-hoc 签名')
    return errors


if __name__ == '__main__':
    config = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text(encoding='utf-8'))
    errors = validate(sys.argv[1] if len(sys.argv) == 2 else '', config)
    print('\n'.join(errors) if errors else 'Alpha 发布配置检查通过')
    raise SystemExit(bool(errors))
