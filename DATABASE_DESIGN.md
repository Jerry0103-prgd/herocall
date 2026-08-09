# 数据库设计（逻辑模型）

本文件描述 SQLite 逻辑模型与 V0.8.1 已落地的数据库核心。迁移由 Rust 服务层版本化管理；所有时间字段使用 ISO 8601 UTC，交易日期字段以 `Asia/Shanghai` 解释。

## 1. 建模原则

- 用户输入的交易流水不可被汇总值覆盖；持仓与盈亏可重算。
- 外部数据需保存来源、市场时间、抓取时间、质量/延迟状态与原始链接或标识。
- 金额、价格、数量存为 decimal 文本或定点整数及其 scale，禁止以浮点数作为权威核算值。
- 外键、唯一索引和事务保障关联完整性；数据迁移版本化。

## 2. V0.2 已实现核心实体

| 表 | 主要字段 | 说明 |
| --- | --- | --- |
| `securities` | `id`, `symbol`, `name`, `market`, `exchange`, `security_type`, `trade_rule`, `industry`, `concepts_json` | A 股普通股票与 ETF；`symbol + market` 唯一；`trade_rule` 为 `T_PLUS_1`、`T_PLUS_0` 或 `UNKNOWN` |
| `cash_accounts` | `id`, `name`, `currency`, `available_to_buy`, `withdrawable_cash`, `pending_settlement` | 本地现金账户；不含券商授权 |
| `holdings` | `id`, `cash_account_id`, `security_id`, `quantity`, `available_quantity`, `average_cost`, `cost_amount`, `position_source` | 当前持仓存储；本阶段不自动计算成本、可卖数或盈亏 |
| `transactions` | `id`, `cash_account_id`, `security_id`, `side`, `status`, `trade_date`, `quantity`, `price`, `commission`, `stamp_tax`, `transfer_fee`, `other_fee`, `minimum_commission` | 完整手工/导入/期初交易流水；状态为 `CONFIRMED` 或 `CANCELLED`，取消代替删除以保留历史 |
| `data_sources` | `id`, `name`, `source_type`, `priority`, `base_url`, `status`, `enabled` | 数据源登记与状态；不保存 API Key |
| `market_snapshots` | `id`, `data_source_id`, `market_timestamp`, `fetched_at`, `delay_status` | 已验证行情抓取批次元数据；无可靠行情时仅保存数据源 `NO_DATA` 状态，不伪造行情时间 |
| `market_quotes` | `id`, `market_snapshot_id`, `security_id`, `symbol`, `security_name`, `market`, `current_price`, `previous_close`, `price_change`, `change_pct`, `volume`, `turnover_amount`, `market_timestamp`, `fetched_at`, `source`, `delay_status` | 单条规范化行情及完整来源/时间/延迟元数据；成交量与成交额同时保存供应商声明单位 |
| `market_index_quotes` | `id`, `market_snapshot_id`, `name`, `symbol`, `current_price`, `change_pct`, `change_percent`, `market_timestamp`, `fetched_at`, `delay_status` | 主要指数快照；`change_pct` 保留供应商原字段，`change_percent` 是供 UI 与 AI Context 使用的兼容字段，并由迁移 `011` 从原字段回填 |
| `corporate_actions` | `id`, `security_id`, `action_type`, `announcement_date`, `effective_date`, `data_source_id`, `source_url`, `details_json`, `status` | 公司行动预留；支持 `DIVIDEND`、`SPLIT`、`EX_RIGHT` 的公告/事件记录，不自动调整持仓 |
| `news_articles` | `id`, `title`, `source`, `source_type`, `published_at`, `fetch_time`, `summary`, `url`, `related_security_id`, `created_at` | 已保存资讯的可追溯记录；来源类型仅为 `OFFICIAL`、`MEDIA`、`COMMUNITY`，社区内容必须作为观点展示；可关联一只证券 |
| `daily_reviews` | `id`, `review_date`, `snapshot_id`, `portfolio_summary`, `market_summary`, `holding_summary`, `risk_summary`, `created_at` | 非 AI 的每日结构化复盘；四个摘要字段保存类型化 JSON，`snapshot_id` 可为空以明确当日市场快照未确认 |
| `manual_refresh_runs` | `id`, `started_at`, `completed_at`, `holdings_snapshot_id`, `indices_snapshot_id`, `portfolio_json`, `news_status`, `events_status`, `status` | 一次用户主动更新今日市场快照的不可变执行边界；资讯/事件由关联表固定到该次刷新，无结果时显式为 `NO_DATA` |
| `manual_refresh_news_articles` | `manual_refresh_run_id`, `news_article_id` | 本次手动快照实际采集并保存的持仓关联资讯；用于冻结 AI Context 的资讯边界 |
| `manual_refresh_events` | `manual_refresh_run_id`, `event_id` | 本次手动快照实际采集并保存的持仓关联事件；用于冻结 AI Context 的事件边界 |
| `ai_review_contexts` | `id`, `review_id`, `manual_refresh_run_id`, `portfolio_json`, `market_json`, `news_json`, `events_json`, `created_at` | 一次成功 AI 调用的审计输入快照；不保存 API Key 或提示词全文 |
| `ai_reviews` | `id`, `review_id`, `context_id`, `provider`, `model`, `prompt_version`, `request_status`, `facts`, `inferences`, `risks`, `created_at` | 对已保存每日复盘的 AI 辅助解释；三段内容保存为 JSON 字符串，必须经结构与禁止词校验后才可落库 |
| `events` | `id`, `event_type`, `title`, `security_id`, `event_time`, `timezone`, `source`, `source_url`, `status`, `created_at` | 投资事件日历；支持财报、分红、除权除息、股东大会、宏观数据和美联储会议，保留来源、原始带时区时间及确认状态 |
| `app_settings` | `setting_key`, `setting_value`, `updated_at` | 非敏感应用状态；V0.8.1 仅保存首次启动完成标志，禁止存储 API Key、Token 或券商信息 |

`schema_migrations` 是迁移系统内部表，保存迁移版本、校验标识与应用时间。当前已定义 `001`（数据库核心）至 `012`（真实公告采集与快照关联）；迁移重复执行不会重新执行已应用版本，已应用迁移的校验标识不匹配会阻止继续启动。

## 3. 延后实现的逻辑实体

`position_lots` 等后续账务实体保留在后续阶段实现。资讯和事件已通过东方财富公开公告 Adapter 在用户手动更新时采集、保存并关联当前持仓；不会写入演示或虚构资讯。每日复盘仅汇总本地 Portfolio、市场快照和持仓关联资讯。AI 复盘通过 Provider Adapter 对已保存结构化输入作辅助解释；未配置时不请求外部服务、不生成或保存内容；行情 Adapter 同样仅在被后续应用服务调用时获取并保存可追溯快照。

## 4. 关键约束与索引

- `securities` 的 `(symbol, market)` 唯一；`security_type` 仅允许 `STOCK` 或 `ETF`，`trade_rule` 仅允许 `T_PLUS_1`、`T_PLUS_0` 或 `UNKNOWN`。
- `holdings` 的 `(cash_account_id, security_id)` 唯一，且 `available_quantity` 不可超过持仓数量；`cost_amount` 是为后续精确成本计算预留的 decimal 文本。
- `transactions` 保留佣金、印花税、过户费、其他费用和最低佣金字段，并以 `(cash_account_id, security_id, trade_date)` 及 `(status, trade_date)` 建索引。交易取消只更新状态，不删除历史；本阶段不执行成本、可卖数、T+1 或盈亏规则。
- `corporate_actions.action_type` 仅允许分红、送转/拆分、除权除息三类预留值；本阶段不基于该表修改持仓、成本或交易流水。
- `market_quotes` 必填 `data_source_id`、`market_timestamp`、`fetched_at`、`delay_status`、`source`，并以 `(security_id, data_source_id, market_timestamp)` 去重；没有任何模拟或硬编码行情写入逻辑。
- `market_snapshots` 与 `market_quotes` 分别按来源和证券时间建立索引，支持后续可追溯查询。
- `news_articles` 强制要求标题、来源、来源类型、发布时间、抓取时间、摘要和 HTTP(S) 原文地址；原文地址唯一，关联证券删除时仅清除关联而保留资讯审计记录。按发布时间及关联证券建立索引，持仓页查询只返回存在当前持仓关联的记录。
- `daily_reviews.review_date` 唯一；同一日期重新生成时原子更新四个摘要和关联快照，绝不生成第二条同日复盘。快照删除时复盘保留但 `snapshot_id` 设为空，以保留生成时的结构化事实和“未确认”状态。
- `ai_reviews` 使用外键关联 `daily_reviews`，每日复盘删除时其 AI 辅助解释一并删除；`context_id` 关联冻结的 `ai_review_contexts`，后者再关联一次 `manual_refresh_runs`。保留提供方、模型名、提示词版本和生成时间以支持审计；运行时 API Key 不进入此表或其他 SQLite 表。
- `events.event_type` 支持 `EARNINGS`、`COMPANY_ANNOUNCEMENT`、`MAJOR_MATTER`、`DIVIDEND`、`EX_DIVIDEND`、`SHAREHOLDER_MEETING`、`MACRO_DATA`、`FED_MEETING`；`status` 仅允许 `CONFIRMED`、`UNCONFIRMED`、`ARCHIVED`。`security_id` 可为空以支持宏观/FED 事件；证券删除时只清除关联、保留事件。查询先标识当前持仓关联事件，再按解析后的原始带时区时间排序，不推断日期或状态。
- `manual_refresh_news_articles` 与 `manual_refresh_events` 仅关联一次用户主动刷新中由 Adapter 实际返回、经校验并已保存的记录。AI Context 从这两张关联表读取，不会在生成 AI 复盘时重新请求数据或补造新闻、事件。
- `app_settings` 仅保存产品运行状态。首次启动向导使用 `initialization_completed=true` 表示完成；该值在数据库重启后保留。不得将配置密钥、个人凭据或任何金融数据写入该表。

## 5. 派生计算（后续阶段）

- **可卖数量：** 截至交易日、尚未卖出的买入 lot 数量之和，排除当日新建 lot。
- **已实现盈亏：** 已卖出 lot 的卖出净额减去对应成本和费用；成本分配规则需固定并版本化（建议 FIFO）。
- **未实现盈亏：** 当前市值减剩余持仓成本；缺少可靠报价时为未知而非零。
- **今日盈亏/总盈亏：** 明确估值时间、昨收/成本和费用口径，且保留所用行情记录引用。
