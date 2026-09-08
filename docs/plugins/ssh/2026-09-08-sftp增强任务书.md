# SFTP 双栏增强任务书（2026-09-08，设计稿 v2 已获用户确认）

## 范围
src/plugins/ssh/**、src-tauri/src/plugins/ssh/**、src/core/ui/**（新增图标）。
按 P1→P4 分批提交，每批过全量门禁（lint/format/test/build + clippy + cargo test + check_docs + check_rust_rules）。

## P1 多选 + 批量
- useFileSelection 共用 composable：单选/Ctrl 切换/Shift 范围/空白清空；多选行 = 高亮 + 4px tertiary 竖条
- 左侧批量：批量下载（N）、批量删除（N，ConfirmDialog 危险档，目录递归写明不可恢复）
- 右侧批量：批量上传（N）、批量删除（N，本地直接删除，确认写明）
- 批量确认弹窗最多列 5 个名 + 等 N 项；传输逐项入队复用现有队列

## P2 位置感知右键 + 新建/刷新
- 菜单矩阵（全部带刷新）：
  - 远程单选文件：下载/重命名/修改权限/删除
  - 远程单选目录：下载/重命名/修改权限/添加书签/删除
  - 远程多选：批量下载/批量删除
  - 远程空白：新建文件/新建目录/上传文件/上传目录
  - 本地单选：上传/重命名/删除；本地多选：批量上传/批量删除；本地空白：新建文件/新建目录
- 新建文件：远程 SFTP create、本地 fs create；InputDialog 校验（非空、禁 /、..，本地禁 <>:"|?*），重名报错；成功后滚动定位选中
- 刷新 = 清缓存强刷

## P3 chmod（仅远程单选）
- 弹窗：勾选矩阵（所有者/组/其他 × 读/写/执行）↔ 八进制输入（3 位 0-7 或 4 位含特殊位）双向实时联动；非法输入红边+错误行，矩阵不动；目录显示「递归应用到子项」（默认不勾）
- 确定 → ConfirmDialog 二次确认（路径 + 旧→新权限 + 递归与否）
- 安全限制（后端强制，前端只藏菜单项）：
  - 系统目录：/ /bin /boot /dev /etc /lib /lib64 /proc /run /sbin /sys /usr /var
  - 删除：系统目录本体及子树一律禁止；根下自定义目录允许
  - chmod：系统目录本体禁止；内部文件允许；/proc /sys /dev 整树禁止；系统目录内递归需 acknowledge_risk=true（前端弹窗红字警告勾选）
  - 符号链接：删除只删链接；chmod 跟进真实目标后判定

## P4 底栏双按钮（左侧远程栏）
- 状态栏右侧两个 UiIconButton sm：☆书签 / ⇅传输
- 书签：Teleport 下拉（z-220）列表点跳、行尾 × 删；目录右键「添加书签」；存 ssh.db `profile_bookmarks(profile_id,name,path,sort)`（user_version 迁移），按 profile 隔离
- 传输：按钮本体即进度条（tertiary 填充动画 + 总进度），文字 `⇅ N 项 · 当前文件名`（文件名固定 12 字符宽 truncate）；点击开/关底部上拉明细面板
- 面板：max-h 40%，每行 类型徽标+文件名 truncate+进度条+百分比+单项取消；头部「全部取消」（ConfirmDialog）；完成项 10s 淡出；**关闭只允许 ✕ 或再点进度条按钮，禁 Esc/遮罩点击**

## 后端新命令
ssh_file_create / ssh_file_chmod / ssh_local_create / ssh_local_delete / ssh_bookmark_list / ssh_bookmark_add / ssh_bookmark_delete

## 不做
双栏互拖、断点续传、远程压缩、文件预览、本地侧 chmod
