/** 独立 SSH 编辑窗口只启动编辑器，主窗口保持原应用初始化。 */
const editorWindow = new URLSearchParams(window.location.search).get('sshEditor')
if (editorWindow && /^[a-f0-9-]{36}$/.test(editorWindow)) {
  void import('./plugins/ssh/files/native/main')
} else {
  void import('./mainApp')
}
