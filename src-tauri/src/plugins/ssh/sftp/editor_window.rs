//! SSH 编辑器专用原生窗口入口；不开放任意 URL、窗口标签或通用窗口权限。
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// 仅主窗口可创建编辑器；子窗口仅能操作自己。标识来自本次握手随机 UUID。
fn editor_label(caller: &str, token: &str, action: &str) -> Result<String, String> {
    if token.len() != 36
        || !token.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
    {
        return Err("编辑窗口标识无效".into());
    }
    let label = format!("ssh-editor-{token}");
    if caller != "main" && (caller != label || action == "open") {
        return Err("该窗口不能操作指定编辑器".into());
    }
    if !["open", "focus", "show", "close", "return"].contains(&action) {
        return Err("编辑窗口操作无效".into());
    }
    Ok(label)
}

/// 创建、显示、聚焦或清理本次 SSH 编辑窗口；URL 固定为应用编辑器入口。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_editor_window(
    app: AppHandle,
    window: WebviewWindow,
    token: String,
    action: String,
) -> Result<(), String> {
    let label = editor_label(window.label(), &token, &action)?;
    let result = (|| -> Result<(), String> {
        if action == "open" {
            if app.get_webview_window(&label).is_some() {
                return Err("编辑窗口已经存在".into());
            }
            WebviewWindowBuilder::new(
                &app,
                &label,
                WebviewUrl::App(format!("index.html?sshEditor={token}").into()),
            )
            .title("CoveKit · SSH 文件编辑器")
            .inner_size(1100.0, 760.0)
            .min_inner_size(640.0, 420.0)
            .center()
            .decorations(true)
            .general_autofill_enabled(false)
            .visible(false)
            .build()
            .map_err(|e| e.to_string())?;
            return Ok(());
        }
        let Some(editor) = app.get_webview_window(&label) else {
            return if action == "close" {
                Ok(())
            } else {
                Err("编辑窗口已关闭".into())
            };
        };
        if action == "return" {
            let main = app.get_webview_window("main").ok_or("主窗口已关闭")?;
            main.show().map_err(|e| e.to_string())?;
            main.unminimize().map_err(|e| e.to_string())?;
            return main.set_focus().map_err(|e| e.to_string());
        }
        match action.as_str() {
            "close" => editor.destroy().map_err(|e| e.to_string()),
            "show" | "focus" => {
                editor.show().map_err(|e| e.to_string())?;
                editor.unminimize().map_err(|e| e.to_string())?;
                editor.set_focus().map_err(|e| e.to_string())
            }
            _ => unreachable!(),
        }
    })();
    match &result {
        Ok(()) => log::info!("编辑窗口操作完成 action={action}"),
        Err(_) => log::warn!("编辑窗口操作失败 action={action}"),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::editor_label;
    #[test]
    fn only_main_creates_and_children_only_control_themselves() {
        let token = "12345678-1234-1234-1234-123456789abc";
        let label = format!("ssh-editor-{token}");
        assert!(editor_label("main", token, "open").is_ok());
        assert!(editor_label(&label, token, "close").is_ok());
        assert!(editor_label(&label, token, "open").is_err());
        assert!(editor_label("other", token, "close").is_err());
        assert!(editor_label("main", "../main", "open").is_err());
        assert!(editor_label("main", token, "navigate").is_err());
    }
}
