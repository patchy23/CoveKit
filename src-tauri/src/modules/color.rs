//! 颜色模块：color_pick_screen（Windows 屏幕取色，取鼠标所在像素）
//! color_start_picker：创建全屏半透明取色窗口（label=picker，加载主应用 #/picker 路由），
//! 遮罩内实时取色，点击确定后通过 "picker-color" 事件回传主窗口并关闭。

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickColor {
    hex: String,
    rgb: [u8; 3],
}

#[tauri::command]
pub fn color_pick_screen() -> Result<PickColor, String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC};
        use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

        let mut pos = POINT { x: 0, y: 0 };
        // SAFETY: 标准 Win32 调用，指针均为有效栈对象或空
        unsafe {
            if GetCursorPos(&mut pos) == 0 {
                return Err("获取鼠标位置失败".into());
            }
            let dc = GetDC(std::ptr::null_mut());
            if dc.is_null() {
                return Err("获取屏幕 DC 失败".into());
            }
            let pixel = GetPixel(dc, pos.x, pos.y);
            ReleaseDC(std::ptr::null_mut(), dc);
            if pixel == u32::MAX {
                // CLR_INVALID
                return Err("读取像素失败".into());
            }
            // COLORREF 布局为 0x00BBGGRR
            let rgb = [
                (pixel & 0xFF) as u8,
                ((pixel >> 8) & 0xFF) as u8,
                ((pixel >> 16) & 0xFF) as u8,
            ];
            Ok(PickColor {
                hex: format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]),
                rgb,
            })
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("屏幕取色暂仅支持 Windows".into())
    }
}

/// 打开全屏半透明取色窗口（#/picker 路由渲染遮罩 UI；已打开则复用）
#[tauri::command]
pub fn color_start_picker(app: AppHandle) -> Result<(), String> {
    if app.get_webview_window("picker").is_some() {
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, "picker", WebviewUrl::App("index.html#/picker".into()))
        .title("屏幕取色")
        .fullscreen(true)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}
