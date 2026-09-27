# 本地依赖补丁

## CodeMirror merge 6.12.2

`@codemirror__merge@6.12.2.patch` 由 `pnpm-workspace.yaml` 的精确版本登记安装。只修改发布包的 ESM、CommonJS 和类型声明；保留上游 MIT 许可证。

上游 `MergeView` 构造及 `dispatch`、`unifiedMergeView` 的 `computeChunks` 在编辑器线程同步计算差异。补丁增加可选 `externalDiff` 和 `setExternalChunks`：默认行为不变；启用后正文变化立即撤销旧差异，仅在两份不可变 `Text` 快照仍匹配时接收新块。异步结果包含版本和 `precise`，消费方恢复 `Chunk`、`Change` 原型。折叠在第一份结果到达后初始化，装饰、接受／拒绝和左右对齐仍使用上游实现。

升级时必须重新核对上述调用点、`ChunkField` 更新顺序、折叠初始化和 `scheduleMeasure`，不能直接迁移补丁上下文。相关回归入口为 `src/core/ui/UiCodeDiff.test.ts`、`src/core/ui/UiCodeDiff.worker.test.ts` 和 `src/core/ui/editor/diff.test.ts`，覆盖 split/unified、文档替换、接受／拒绝、过期结果、选择、取消和释放。完整构建及真实 WebView 的滚动、布局按仓库验证矩阵由维护者执行。
