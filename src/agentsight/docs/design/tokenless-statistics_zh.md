# Tokenless 统计记录

[English](tokenless-statistics.md)

服务使用仅保存在后端的凭据查询已配置的 Tokenless API，将有界查询结果以事务保存到
现有 GenAI store，复用数据库生命周期和保留策略。原始源字段保持不变，本地会话关联
仅采用唯一匹配的已采集 tool-call ID；缺失或歧义匹配保持 null。端点、源 ID 与原始
时间戳用于重复刷新去重，不引入恢复协议。刷新元数据时保留已缓存的完整内容。
失败明确返回；源服务离线时仍可查看已保存的统计数据。

展示层读取单个已配置端点的缓存副本。实际节省要求显式 active 模式和唯一的已采集
关联。获取内容时校验源 ID 与原始时间戳后才更新副本。较大的 auth/savings handler
文件仅增加委托入口；拆分计划是将 auth handler、savings DTO 和测试移到相邻模块并
保留现有路由。新增传输与展示逻辑保存在 Tokenless 模块。Linux 的
`make test-tokenless-api` 门禁运行两个已构建的 CLI 进程；先构建 Tokenless 与
AgentSight，再使用 Python 3.11 或更新版本执行。
