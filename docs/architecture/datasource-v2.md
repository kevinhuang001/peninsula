# Peninsula Data Source V2

V2 是声明式数据源的多指标格式。**一个数据源可以声明一个或多个 HTTP 请求**，每个响应可以提取不同指标。既有 `peninsula.datasource/v1` 单指标文件继续有效。

## 字段

- `schema`：`peninsula.datasource/v2`。
- `id` / `name` / `description` / `icon`：数据源信息。
- `name_template`（可选）：导入后的默认显示名称模板，支持 `${input.key}`。用户可在 Peninsula 的数据源详情中修改名称模板，留空恢复默认。
- `inputs`：用户填写的字符串或数字参数。
- `request`（单请求）：原有的 HTTP 请求声明，可从同一响应提取多个数值。
- `requests`（多请求）：最多 8 个不同的命名 HTTP 请求，每个声明自己的 method、url、headers、query 和 body。与单个 `request` 二选一，不能混用。所有 URL 都会在导入时列出，并按 Origin 授权。占位符支持 `${input.key}`。
- 每个 `values.<id>.request`：在多请求模式下必填，必须引用 `requests` 中存在的请求 ID。多个指标可以绑定同一个请求，共用那次返回的 JSON。
- `values`：最多 32 个指标，以稳定的 ASCII 指标 ID 为键；每个指标有 `name`、`value`、可选的 `capsule_text` 和 `alert`。V2 不使用根级 `value` 或 `alert`。

每个指标可用 `capsule_text` 定义胶囊展示文字，例如 `"Star: ${value}"` 会显示 `Star: 1 个`（取决于 `suffix` 等格式设定）。不指定时只显示格式化数值；模板必须包含 `${value}`，最多 80 个字符，不允许引用 `${input.*}` 等参数，以免误展示密钥。V1 也允许在根级设置 `capsule_text`。

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

每个 `values` 项由 Core 生成独立胶囊开关；包含 `alert` 的项还生成独立通知开关和最短提醒间隔。数据源只在至少一个指标的胶囊或通知有效启用时刷新；**如果某个 URL 不再被任何启用的指标引用，就不请求这个 URL**。切换指标时取消失效的任务，所有指标都关闭后停止自动刷新。**手动刷新**可以在全部关闭时一次性读取所有指标，但不自动开启轮询或提醒。刷新间隔由 Peninsula 保存，不在 PDS 中配置。

## 多 URL 示例

一个数据源可以聚合不同地址的 JSON。以下为精简示例（省略可选字段）：

```json
{
  "schema": "peninsula.datasource/v2",
  "id": "github-repository",
  "name": "GitHub 仓库",
  "name_template": "${input.repo}",
  "inputs": {
    "owner": {"type": "string", "label": "Owner", "default": "openai"},
    "repo": {"type": "string", "label": "Repo", "default": "example"}
  },
  "requests": {
    "issues": {
      "method": "GET",
      "url": "https://api.github.com/search/issues",
      "query": {"q": "repo:${input.owner}/${input.repo} is:issue is:open"}
    },
    "stars": {
      "method": "GET",
      "url": "https://api.github.com/repos/${input.owner}/${input.repo}"
    }
  },
  "values": {
    "issues": {
      "name": "开放 Issue",
      "request": "issues",
      "value": {"path": "$.total_count", "decimals": 0},
      "alert": {
        "mode": "change", "operator": "gt", "threshold": 0,
        "title": "${input.repo} Issue 增加",
        "message": "开放 Issue 净增 ${delta} 个"
      }
    },
    "stars": {
      "name": "Star 数",
      "request": "stars",
      "capsule_text": "Star: ${value}",
      "value": {"path": "$.stargazers_count", "decimals": 0}
    }
  }
}
```

每个 URL 独立做 Origin、DNS、重定向和大小限制校验。即使某个请求失败，其他成功返回的指标仍会更新；失败的指标保留上次成功结果并标明错误。单个数据源按统一刷新间隔调度，多组 URL 顺序执行，不额外引入独立后台进程。

## GitHub 示例

Marketplace 的 `github-repository.pds.json` 使用三个公开的 REST 请求读取开放 PR 数、开放 Issue 数和 Star 数，**一个数据源**提供三个可以独立启用的指标。公开仓库无需 GitHub Token，但匿名 GitHub API 存在速率限制，建议降低刷新频率。请求参数经 URL 编码后传入 API，不在 URL 的 Origin 部分插入占位符。
