//! 构建脚本：调用 tauri 构建过程，并给测试目标补上 comctl32 v6 的清单依赖。
//!
//! Windows(MSVC) 下测试二进制（`cargo test --lib` 的 libtest 载体、集成测试）没有应用清单：
//! 经 tauri-plugin-dialog → rfd 链接进来的 `TaskDialogIndirect` 只在 comctl32 v6 中导出，
//! 加载时会直接以 0xC0000139(STATUS_ENTRYPOINT_NOT_FOUND) 失败，测试根本起不来。
//! 因此这里只为测试目标声明清单依赖；应用窗口与打包用的清单仍由 tauri 构建过程生成。

fn main() {
    // 只对 MSVC 目标生效：/MANIFESTDEPENDENCY 是 link.exe 的参数。
    // 用无后缀的 rustc-link-arg：单元测试载体（--lib --test）不属于 -tests 覆盖范围。
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
    tauri_build::build()
}
