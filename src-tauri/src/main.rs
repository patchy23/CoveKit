//! CoveKit 的可执行入口：启动 tauri 运行时（业务全在 lib.rs 侧）。
//! Windows 注意：release 下隐藏附加控制台窗口，DO NOT REMOVE 该属性。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// 二进制入口：调用 lib 的 run()
fn main() {
    covekit_lib::run()
}
