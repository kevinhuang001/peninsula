# Peninsula Data Source V2

V2 是声明式数据源的多指标格式。**一个数据源发起一次 HTTP 请求**，从同一份响应 JSON 中提取多个数值。既有 `peninsula.datasource/v1` 单指标文件继续有效。

## 字段

- `schema`：`peninsula.datasource/v2`。
- `id` / `name` / `description` / `icon`：数据源信息。
- `name_template`（可选）：导入后的默认显示名称模板，支持 `${input.key}`。用户可在 Peninsula 的数据源详情中修改名称模板，留空恢复默认。
- `inputs`：用户填写的字符串或数字参数。
- `request`：请求方法、地址、请求头、查询参数和可选请求体。支持 `${input.key}` 占位符。
- `values`：最多 32 个指标，以稳定的 ASCII 指标 ID 为键；每个指标有 `name`、`value` 和可选的 `alert`。V2 不使用根级 `value` 或 `alert`。

指标 `value` 的数值提取与 V1 相同：`path` 为 JSONPath，必须恰好匹配一个有限数值；支持 `multiply`、`divide`、`offset`、`decimals`、`separator`、`prefix`、`suffix`。

## 变化提醒

`alert` 定义：

```json
{
  "mode": "change",
  "operator": "gt",
  "threshold": 0,
  "title": "${input.repo} Issue 有增加",
  "message": "开放 Issue 净增 ${delta} 个，目前 ${value}。"
}
```

- `mode: "value"`（默认）：把当前数值与阈值比较。
- `mode: "change"`：把当前与历史采样的**累计净变化**与阈值比较。`gt 0` 表示只提醒增长，`lt 0` 可用于减少，`ne 0` 可用于任意净变化。支持 `gt`、`gte`、`lt`、`lte`、`eq`、`ne` 运算符。
- 提醒文本支持 `${value}`（当前格式化值）、`${previous}`（上次采样值）、`${delta}`（累计净变化），也支持 `${input.key}`。阈值可以是数值或数字类型的 `${input.key}`。
- **首次成功请求只建立基准值**，不作为变化发出通知。
- 上次值、累计变化量、各指标的提醒冷却与待确认提醒状态单独持久化。通知被接受后清除对应累计变化量；冷却或通知拒绝时不丢弃未通知的净变化。
- 变化是**两次采样间的净差**，不能识别两次刷新之间创建又关闭的 Issue 或 PR；需要逐个事件时应使用 Webhook，而不是数量采样。

## 独立设置与资源管理

每个 `values` 项由 Core 生成独立胶囊开关；包含 `alert` 的项还生成独立通知开关和最短提醒间隔。数据源的请求只在至少一个指标的胶囊或通知处于有效启用状态时执行；全部关闭后取消进行中的请求并停止后续刷新。刷新间隔由 Peninsula 保存，不在 PDS 中配置。

## GitHub 示例

Marketplace 的 `github-repository.pds.json` 使用 GitHub GraphQL：同一请求读取开放 PR 数、开放 Issue 数和 Star 总数。GraphQL 需要 GitHub Token；在 Peninsula 中可以通过 `$名称$` 引用「秘密」中存储的密钥。请求变量通过 JSON `body.variables` 传入，而不是在 GraphQL 查询文本中拼接用户输入。
