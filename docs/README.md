# CoveKit 文档地图

> 本页只导航仓库中的长期知识。任务进度与接续只在本地维护，按需读取可选的 `TODO.md` 或 `.work/tasks/`；缺失不阻塞开发。

## 最小必读集

1. `../AGENTS.md`：请求分流与不可遗漏边界，已注入无需重复读取。

咨询只追加与问题有关的源码或说明；实施和接手查工作流；本地 TODO 存在时按需查相关项。历史材料不产生执行义务，不按顺序全文加载手册。

## 按任务追加

先查标题与符号，只取相关节。可以列目录和搜索，不无目的全量加载正文。

| 任务 | 去哪读 | 必须找到 |
| --- | --- | --- |
| UI文案/小修 | [11必读摘要](standards/11-插件UI开发约定.md#必读摘要)，实际组件 | token、公共组件、可访问性；不用通读所有渲染坑 |
| 交互设计/工具布局 | [22交互体验规范](standards/22-交互体验设计规范.md#必读摘要)、[11](standards/11-插件UI开发约定.md)相关专题 | 任务流程、组件行为、反馈、工作连续性与工具独立性；区分设计约束与已实现能力 |
| 组件API/表单/浮层 | [11a](standards/11a-公共组件API备忘.md)、[13](standards/13-表单排版约定.md)、[12](standards/12-reka-ui坑.md)相关节 | 真实props/emits与相应交互约束 |
| 框架/IPC | [02架构](standards/02-架构.md)、[03模块](standards/03-模块开发规则.md)、[19契约审计](standards/19-Tauri契约审计.md)相关节 | 当前contracts、handler、事件、取消与消费方 |
| Rust | [05规范](standards/05-Rust代码规范.md) | 错误、锁、生命周期、安全与源检查边界 |
| 日志接入与补充 | [05日志标准](standards/05-Rust代码规范.md#7-日志约定)，实际操作与错误传播链 | 生态日志库接口、输出位置、级别、去重与敏感信息；区分接入标准和已实现能力 |
| 新功能范围 | [07总纲](standards/07-产品需求.md)，对应分册 | 核对当前有效范围裁决；分册不自动授权实现 |
| SSH | [07a](standards/07a-需求-SSH远程管理.md)、[工具文档](plugins/ssh/) | 当前需求与本轮不做项 |
| HTTP/网络、DNS/Hosts、数据/媒体 | [07b](standards/07b-需求-网络与调试.md)、[07c](standards/07c-需求-DNS与Hosts.md)、[07d](standards/07d-需求-数据与媒体工具.md) | 只取对应工具节，必要时读总纲的跨域约束 |
| 测试/浏览器走查 | [20验证矩阵](standards/20-验证矩阵.md)、[15测试方法](standards/15-测试与走查.md)、[11b浏览器](standards/11b-浏览器驱动验证UI.md) | 相关命令、场景及平台局限 |
| 接手任务 | 按需读本地记录；无记录时直接核对规范与实际代码 | 下一步、前置、证据、已裁决缺项 |
| 多会话或明确委派 | [16](standards/16-多角色编排.md) | 任务边界、未提交内容保护与必要接续 |
| 新建/迁移文档 | [17](standards/17-文档组织与批次.md)、[模板](standards/templates/) | 入库门槛、按需方案与链接 |
| 搬移/重构 | [18](standards/18-重构与搬移验证.md) | 活跃实现、实际接线与行为等价 |

## 其他入口

| 主题 | 文档 |
| --- | --- |
| 技术选型 | [01](standards/01-技术选型.md) |
| 公共UI规范与tokens | [22交互体验](standards/22-交互体验设计规范.md)、[04组件](standards/04-公共UI组件.md)、[DESIGN](../DESIGN.md) |
| 发布/平台 | [06](standards/06-发布与更新.md)、[09](standards/09-平台能力矩阵.md)、[发布记录](releases/) |
| 术语、编号 | [08](standards/08-术语与编号.md) |
| Git提交信息 | [21](standards/21-Git提交规范.md)，只约束新提交 |
| 编辑器主题 | [14](standards/14-编辑器主题配方.md) |
| 跨批次决策 | [ADR索引](adr/README.md) |
| 验证分工与命令 | [20验证矩阵](standards/20-验证矩阵.md)；具体待办仅本地 |
| 本地数据传输与凭证边界 | [数据空间与导入导出](standards/07e-数据空间与导入导出.md) |
| 插件现行说明 | [SSH](plugins/ssh/)、[Database](plugins/database/)、[Vault](plugins/vault/)、[FRP](plugins/frp/设计.md) |

命名与维护只见17。[本轮有效范围](standards/07-产品需求.md#当前有效范围裁决)先于旧需求。
