// patchyBox 桌面工具箱 · Rust 侧框架装配入口
// M0：最小骨架——插件注册 + 命令装配处。
// M1 起业务模块放 src-tauri/modules/，此处只做装配（新增命令 = 模块 + 注册一行）。

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
