// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// 二进制入口：调用 lib 的 run()
fn main() {
    patchybox_lib::run()
}
