//! FRP 客户端获取 · tests

#[cfg(test)]
mod cases {

    use super::super::arch;
    use super::super::archive::extract;
    use super::super::archive::has_entries;
    #[cfg(windows)]
    use super::super::archive::ps_literal;
    use super::super::asset_name;
    use super::super::checksum::judge_checksum;
    use super::super::checksum::parse_expected_hash;
    use super::super::checksum::ChecksumStatus;
    use super::super::checksums_name;
    use super::super::detect::version_key;
    use super::super::detect::which;
    use super::super::package_dir_name;
    use super::super::platform;
    use super::super::source_label;
    use super::super::sources_in_order;
    use super::super::DOWNLOAD_SOURCES;
    #[cfg(windows)]
    use tokio::process::Command;

    #[test]
    fn asset_name_follows_upstream_convention() {
        let name = asset_name("0.71.0");
        assert!(name.starts_with("frp_0.71.0_"));
        assert!(name.ends_with(if cfg!(windows) { ".zip" } else { ".tar.gz" }));
    }

    #[test]
    fn platform_and_arch_map_to_upstream_tokens() {
        assert!(["windows", "darwin", "linux"].contains(&platform()));
        assert!(["amd64", "arm64"].contains(&arch()));
    }

    #[test]
    fn package_dir_matches_asset_stem() {
        let dir = package_dir_name("0.71.0");
        let asset = asset_name("0.71.0");
        assert!(asset.starts_with(&dir));
    }

    /// 解压必须能真正解开 zip
    ///
    /// 回归「下载完成却报解压失败」：下载文件曾命名为 `.zip.tmp`，PowerShell 的
    /// `Expand-Archive` 只看后缀就拒绝（NotSupportedArchiveFileExtension），而下载本身
    /// 是完整的——测试同时守住「后缀合法」与「确实解出文件」两点。
    #[cfg(windows)]
    #[tokio::test]
    async fn extract_unpacks_real_zip() {
        let dir = std::env::temp_dir().join("covekit-frp-extract-ok");
        let _ = tokio::fs::remove_dir_all(&dir).await;
        tokio::fs::create_dir_all(&dir).await.expect("建临时目录");
        let source = dir.join("payload");
        tokio::fs::create_dir_all(&source).await.expect("建源目录");
        tokio::fs::write(source.join("frpc.txt"), b"frpc")
            .await
            .expect("写样例文件");
        let archive = dir.join("sample.zip");
        let script = format!(
            "Compress-Archive -Path '{}' -DestinationPath '{}' -Force",
            ps_literal(&source.join("*")),
            ps_literal(&archive)
        );
        let status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status()
            .await
            .expect("调用 PowerShell 造 zip");
        assert!(status.success(), "造测试压缩包失败");

        let out = dir.join("out");
        extract(&archive, &out).await.expect("解压应成功");
        assert!(out.join("frpc.txt").is_file(), "解压后应产出文件");
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    /// 坏包不能假成功
    ///
    /// 回归第二重坑：`Expand-Archive` 遇到坏包时只写 stderr、**退出码仍是 0**，
    /// 只看 `status.success()` 会把「什么都没解出来」当成安装成功。
    #[cfg(windows)]
    #[tokio::test]
    async fn extract_rejects_broken_archive() {
        let dir = std::env::temp_dir().join("covekit-frp-extract-broken");
        let _ = tokio::fs::remove_dir_all(&dir).await;
        tokio::fs::create_dir_all(&dir).await.expect("建临时目录");
        let archive = dir.join("broken.zip");
        tokio::fs::write(&archive, b"this is definitely not a zip archive")
            .await
            .expect("写坏包");

        let out = dir.join("out");
        let result = extract(&archive, &out).await;
        assert!(result.is_err(), "坏包必须报错，不能静默成功");
        assert!(!has_entries(&out).await, "坏包不应产出任何文件");
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[test]
    fn version_key_orders_numerically() {
        // 字符串序会判错：`0.9.0` 按字符串大于 `0.71.0`，按数值则相反
        assert!(version_key("0.71.0") > version_key("0.9.0"));
        assert!(version_key("0.71.0") > version_key("0.70.9"));
        assert_eq!(version_key("0.71.0"), vec![0, 71, 0]);
        // 带预发布后缀时非数字段按 0 处理，不 panic（rc1 解析失败记为 0）
        assert_eq!(version_key("0.71.0-rc1"), vec![0, 71, 0, 0]);
    }

    #[test]
    fn download_sources_try_direct_first_and_hoist_the_working_one() {
        // 直连排首位：能直连就不该绕镜像
        assert_eq!(DOWNLOAD_SOURCES[0], "");
        assert_eq!(source_label(""), "GitHub 官方");
        // 已成功的源提到最前（同源取 checksums），其余顺序不变
        let ordered = sources_in_order("https://ghproxy.net/");
        assert_eq!(ordered[0], "https://ghproxy.net/");
        assert_eq!(ordered.len(), DOWNLOAD_SOURCES.len());
        // 未知来源不改动顺序、不 panic
        assert_eq!(sources_in_order("https://example.invalid/")[0], "");
    }

    #[test]
    fn which_finds_existing_file_on_path() {
        // 用系统上必然存在的可执行文件探测（windows: cmd.exe，其它: sh）
        let probe = which(if cfg!(windows) { "cmd.exe" } else { "sh" });
        assert!(probe.is_some());
    }

    /// checksums 资产名固定，不能按版本拼
    ///
    /// 回归「强校验从未生效」：代码曾请求 `frp_{version}_checksums.txt`，而上游实际
    /// 提供的是固定的 `frp_sha256_checksums.txt`（v0.44 起，更早版本没有这个文件），
    /// 于是每次下载都拿到 404、静默退化成「仅校验解压完整性」。
    #[test]
    fn checksums_name_matches_upstream_asset() {
        assert_eq!(checksums_name(), "frp_sha256_checksums.txt");
    }

    /// 解析上游 checksums：CRLF、Tab 分隔、大写哈希、注释行与格式异常行都要处理对
    #[test]
    fn parse_expected_hash_reads_upstream_format() {
        let text = "a872a46b08ff971462f311dce3d9b3c538f3c130ed7cdee3ea75b6728b9f5d3c  frp_0.70.1_android_arm64.tar.gz\r\n\
                    CBF69CF26E5553E914E97D37F5D4367FA30F5F531D073A889465AF4719281E25\tfrp_0.70.1_darwin_amd64.tar.gz\n";
        let asset = "frp_0.70.1_darwin_amd64.tar.gz";
        assert_eq!(
            parse_expected_hash(text, asset).as_deref(),
            Some("cbf69cf26e5553e914e97d37f5d4367fa30f5f531d073a889465af4719281e25")
        );
        // 未列出的资产、注释行、长度不足的哈希都不认作期望值
        assert_eq!(
            parse_expected_hash(text, "frp_0.70.1_windows_amd64.zip"),
            None
        );
        assert_eq!(parse_expected_hash("# 说明行 frp_x.zip", "frp_x.zip"), None);
        assert_eq!(
            parse_expected_hash("deadbeef  frp_x.zip", "frp_x.zip"),
            None
        );
    }

    /// 「不一致」必须与「拿不到期望值」分开，且原因要能指明具体资产
    #[test]
    fn judge_checksum_separates_mismatch_from_unavailable() {
        let hash = "cbf69cf26e5553e914e97d37f5d4367fa30f5f531d073a889465af4719281e25";
        let other = "0".repeat(64);
        let asset = "frp_0.70.1_darwin_amd64.tar.gz";
        let text = format!("{hash}  {asset}\n");

        assert!(matches!(
            judge_checksum(&text, asset, hash),
            ChecksumStatus::Verified
        ));

        match judge_checksum(&text, asset, &other) {
            ChecksumStatus::Mismatch { expected, actual } => {
                assert_eq!(expected, hash);
                assert_eq!(actual, other);
            }
            _ => panic!("哈希不符必须判 Mismatch"),
        }

        // 资产未列在 checksums 里：是「拿不到期望值」，不是「校验失败」
        match judge_checksum(&text, "frp_0.70.1_windows_amd64.zip", hash) {
            ChecksumStatus::Unavailable(reason) => {
                assert!(
                    reason.contains("frp_0.70.1_windows_amd64.zip"),
                    "原因要指明具体资产：{reason}"
                );
            }
            _ => panic!("未列出应判 Unavailable"),
        }
    }
}
