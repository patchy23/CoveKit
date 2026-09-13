# patchyBox 文档地图

> 导航只说明去哪找，不复制进度。当前范围先看[TODO](../TODO.md)，进度只查[台账](进度台账.md)的相关批次。

## 最小必读集

以下四份总计上限15,000字符；已注入AGENTS无需重复读取。台账不在默认全文读取集合中。

1. `../AGENTS.md`：不可遗漏边界、协作与提交。
2. `../TODO.md`：当前用户裁决和明确不做项。
3. `standards/10-AI开发工作流.md`：定档、授权、执行。
4. `README.md`：本文，按下面路径追加。

## 按任务追加

先查标题与符号，只取相关节。可以列目录和搜索，不无目的全量加载正文。

| 任务 | 去哪读 | 必须找到 |
| --- | --- | --- |
| UI文案/小修 | [11必读摘要](standards/11-插件UI开发约定.md#必读摘要)，实际组件 | token、公共组件、可访问性；不用通读所有渲染坑 |
| 组件API/表单/浮层 | [11a](standards/11a-公共组件API备忘.md)、[13](standards/13-表单排版约定.md)、[12](standards/12-reka-ui坑.md)相关节 | 真实props/emits与相应交互约束 |
| 框架/IPC | [02架构](standards/02-架构.md)、[03模块](standards/03-模块开发规则.md)、[19契约审计](standards/19-Tauri契约审计.md)相关节 | 当前contracts、handler、事件、取消与消费方 |
| Rust | [05规范](standards/05-Rust代码规范.md) | 错误、锁、生命周期、安全与源检查边界 |
| 新功能范围 | [07总纲](standards/07-产品需求.md)，对应分册 | TODO裁决优先；分册不自动授权实现 |
| SSH | [07a](standards/07a-需求-SSH远程管理.md)、[工具文档](plugins/ssh/) | 当前需求与本轮不做项 |
| HTTP/网络、DNS/Hosts、数据/媒体 | [07b](standards/07b-需求-网络与调试.md)、[07c](standards/07c-需求-DNS与Hosts.md)、[07d](standards/07d-需求-数据与媒体工具.md) | 只取对应工具节，必要时读总纲的跨域约束 |
| 测试/浏览器走查 | [20验证矩阵](standards/20-验证矩阵.md)、[15测试方法](standards/15-测试与走查.md)、[11b浏览器](standards/11b-浏览器驱动验证UI.md) | 相关命令、场景及平台局限 |
| 接手任务 | [台账](进度台账.md)搜索批次号，再读对应[batches](batches/)文件 | 下一步、前置、证据、已裁决缺项 |
| 多角色协作 | [16](standards/16-多角色编排.md) | 授权、文件归属、交接与独立验收 |
| 新建/迁移文档 | [17](standards/17-文档组织与批次.md)、[模板](standards/templates/) | 权威分工、命名、两份文档与链接 |
| 搬移/重构 | [18](standards/18-重构与搬移验证.md) | 活跃实现、实际接线与行为等价 |

## 其他入口

| 主题 | 文档 |
| --- | --- |
| 技术选型 | [01](standards/01-技术选型.md) |
| 公共UI规范与tokens | [04](standards/04-公共UI组件.md)、[DESIGN](../DESIGN.md) |
| 发布/平台 | [06](standards/06-发布与更新.md)、[09](standards/09-平台能力矩阵.md)、[发布记录](releases/) |
| 术语、编号 | [08](standards/08-术语与编号.md) |
| 编辑器主题 | [14](standards/14-编辑器主题配方.md) |
| 跨批次决策 | [ADR索引](adr/README.md) |
| 插件现行说明 | [SSH](plugins/ssh/)、[Database](plugins/database/)、[Vault](plugins/vault/) |

命名与维护只见17。批次目录是检索入口，当前状态只见台账，不在本地图另建状态列。
