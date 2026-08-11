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
| `watchlist_items` | `id`, `security_id`, `created_at`, `updated_at` | 用户当前关注关系；可由用户代码和名称直接建立，未验证字段明确为 `UNKNOWN`，不会伪造行情或基础信息 |
| `transactions` | `id`, `cash_account_id`, `security_id`, `side`, `status`, `trade_date`, `quantity`, `price`, `commission`, `stamp_tax`, `transfer_fee`, `other_fee`, `minimum_commission` | 完整手工/导入/期初交易流水；状态为 `CONFIRMED` 或 `CANCELLED`，取消代替删除以保留历史 |
| `data_sources` | `id`, `name`, `source_type`, `priority`, `base_url`, `status`, `enabled` | 数据源登记与状态；不保存 API Key |
| `market_snapshots` | `id`, `data_source_id`, `market_timestamp`, `fetched_at`, `delay_status` | 已验证行情抓取批次元数据；无可靠行情时仅保存数据源 `NO_DATA` 状态，不伪造行情时间 |
| `market_quotes` | `id`, `market_snapshot_id`, `security_id`, `symbol`, `security_name`, `market`, `current_price`, `previous_close`, `price_change`, `change_pct`, `volume`, `turnover_amount`, `market_timestamp`, `fetched_at`, `source`, `delay_status` | 单条规范化行情及完整来源/时间/延迟元数据；成交量与成交额同时保存供应商声明单位 |
| `market_index_quotes` | `id`, `market_snapshot_id`, `name`, `symbol`, `current_price`, `change_pct`, `change_percent`, `market_timestamp`, `fetched_at`, `delay_status` | 主要指数快照；`change_pct` 保留供应商原字段，`change_percent` 是供 UI 与 AI Context 使用的兼容字段，并由迁移 `011` 从原字段回填 |
| `corporate_actions` | `id`, `security_id`, `action_type`, `announcement_date`, `effective_date`, `data_source_id`, `source_url`, `details_json`, `status` | 公司行动预留；支持 `DIVIDEND`、`SPLIT`、`EX_RIGHT` 的公告/事件记录，不自动调整持仓 |
| `news_articles` | `id`, `title`, `source`, `source_type`, `published_at`, `fetch_time`, `summary`, `url`, `related_security_id`, `created_at` | 已保存资讯的可追溯正文；可通过 `news_security_links` 关联一只或多只证券 |
| `news_security_links` | `news_article_id`, `security_id` | 资讯与证券的多对多关联；删除一只关注标的时保留仍关联其他证券的资讯正文 |
| `daily_reviews` | `id`, `review_date`, `snapshot_id`, `portfolio_summary`, `market_summary`, `holding_summary`, `risk_summary`, `created_at` | 非 AI 的每日结构化复盘；四个摘要字段保存类型化 JSON，`snapshot_id` 可为空以明确当日市场快照未确认 |
| `manual_refresh_runs` | `id`, `started_at`, `completed_at`, `holdings_snapshot_id`, `indices_snapshot_id`, `portfolio_json`, `news_status`, `events_status`, `status` | 一次用户主动更新今日市场快照的不可变执行边界；资讯/事件由关联表固定到该次刷新，无结果时显式为 `NO_DATA` |
| `manual_refresh_news_articles` | `manual_refresh_run_id`, `news_article_id` | 本次手动快照实际采集并保存的持仓关联资讯；用于冻结 AI Context 的资讯边界 |
| `manual_refresh_events` | `manual_refresh_run_id`, `event_id` | 本次手动快照实际采集并保存的持仓关联事件；用于冻结 AI Context 的事件边界 |
| `ai_review_contexts` | `id`, `review_id`, `manual_refresh_run_id`, `portfolio_json`, `market_json`, `news_json`, `events_json`, `created_at` | 一次成功 AI 调用的审计输入快照；不保存 API Key 或提示词全文 |
| `ai_reviews` | `id`, `review_id`, `context_id`, `provider`, `model`, `prompt_version`, `request_status`, `facts`, `inferences`, `risks`, `report_json`, `created_at` | 对已保存每日复盘的 AI 辅助解释；原三段内容为安全审计记录，`report_json` 保存 V1.0.3 七项投研报告字段，均须经结构与禁止词校验后才可落库 |
| `ai_provider_settings` | `provider`, `endpoint`, `model_id`, `enabled`, `priority`, `updated_at` | 非敏感 AI Provider 偏好；仅保存端点、模型 ID、启用状态与调用优先级，API Key 仅存 macOS Keychain |
| `events` | `id`, `event_type`, `title`, `security_id`, `event_time`, `timezone`, `source`, `source_url`, `status`, `created_at` | 投资事件日历正文；可通过 `event_security_links` 关联多个证券，保留来源、原始带时区时间及确认状态 |
| `event_security_links` | `event_id`, `security_id` | 事件与证券的多对多关联；删除一只关注标的时保护其他证券仍需的事件正文 |
| `app_settings` | `setting_key`, `setting_value`, `updated_at` | 非敏感应用状态；V0.8.1 仅保存首次启动完成标志，禁止存储 API Key、Token 或券商信息 |

`schema_migrations` 是迁移系统内部表，保存迁移版本、校验标识与应用时间。当前已定义 `001`（数据库核心）至 `022`（市场情报与市场雷达）；迁移重复执行不会重新执行已应用版本，已应用迁移的校验标识不匹配会阻止继续启动。

### V1.1.0 Research Agent 追加实体（迁移 `021`）

| 表 | 主要字段 | 说明 |
| --- | --- | --- |
| `research_runs` | `id`, `started_at`, `completed_at`, `indices_snapshot_id`, `status` | 一次 AI 研究的数据边界；同次输出只能引用该边界准备的数据。 |
| `security_price_history` | `security_id`, `trade_date`, `open_price`, `high_price`, `low_price`, `close_price`, `volume`, `amount`, `change_percent`, `source`, `market_timestamp`, `fetched_at` | 来源可追溯的日线历史数据；以 `(security_id, trade_date, source)` 去重，缺失时明确为不可用而非补值。 |
| `research_evidence` | `research_run_id`, `security_id`, `evidence_type`, `source*`, `payload_json` | 冻结实际送入模型前的市场、资讯、事件证据载荷，保留可审计追溯信息。 |

`ai_review_contexts.research_run_id` 与 `ai_reviews.research_run_id` 由 `021` 追加，旧记录为 `NULL` 且保持可读；新生成的逐证券报告必须绑定当前 `research_runs`。历史 `portfolio_json` 字段为兼容审计列，V1.1.0 Provider 输入不会序列化账户、数量、成本或盈亏字段。

### V1.1.1 市场情报追加实体（迁移 `022`）

| 表/字段 | 主要字段 | 说明 |
| --- | --- | --- |
| `intelligence_items` | `title`, `summary`, `source`, `source_type`, `source_url`, `published_at`, `fetched_at`, `credibility_level`, `dedup_key`, `topic_key`, `importance_score`, `heat_score`, `status` | 可追溯市场情报正文；来源类型限定为 `OFFICIAL`、`NEWS`、`INDUSTRY`、`COMMUNITY`、`SOCIAL`、`RUMOR`，可信度限定 A-E。`UNVERIFIED` 与 `PARTIALLY_CONFIRMED` 明确标识传闻验证状态。 |
| `intelligence_security_relations` | `intelligence_item_id`, `security_id` | 情报与关注标的的多对多关联；删除单一标的时保留共享情报及其他关联。 |
| `manual_refresh_intelligence_items` | `manual_refresh_run_id`, `intelligence_item_id` | 绑定一次用户手动更新实际收集到的情报，供 AI Context 审计和冻结。 |
| `ai_review_contexts.intelligence_json` | JSON | 迁移 `022` 新增的冻结情报摘要。A/B 验证信息、社区观点与传闻分区保存；不保存 Key 或完整提示词。 |

迁移 `022` 只新增三张表、索引及一个带默认值的审计列，不删除或变更既有新闻、事件、AI 记录和用户数据。

## 3. 延后实现的逻辑实体

`position_lots` 等后续账务实体保留在后续阶段实现。资讯和事件已通过东方财富公开公告 Adapter 在用户手动更新时采集、保存并关联当前持仓；不会写入演示或虚构资讯。每日复盘仅汇总本地 Portfolio、市场快照和持仓关联资讯。AI 复盘通过 Provider Adapter 对已保存结构化输入作辅助解释；未配置时不请求外部服务、不生成或保存内容；行情 Adapter 同样仅在被后续应用服务调用时获取并保存可追溯快照。

## 4. 关键约束与索引

- `securities` 的 `(symbol, market)` 唯一；`security_type` 仅允许 `STOCK` 或 `ETF`，`trade_rule` 仅允许 `T_PLUS_1`、`T_PLUS_0` 或 `UNKNOWN`。
- `holdings` 的 `(cash_account_id, security_id)` 唯一，且 `available_quantity` 不可超过持仓数量；`cost_amount` 是为后续精确成本计算预留的 decimal 文本。
- `watchlist_items.security_id` 唯一，按 `created_at DESC, id DESC` 返回，因此最新关注标的位于列表顶部。用户可直接输入六位代码和名称；若本地没有该代码，仅保存用户输入，市场、交易规则等未确认字段必须为 `UNKNOWN`。
- `transactions` 保留佣金、印花税、过户费、其他费用和最低佣金字段，并以 `(cash_account_id, security_id, trade_date)` 及 `(status, trade_date)` 建索引。交易取消只更新状态，不删除历史；本阶段不执行成本、可卖数、T+1 或盈亏规则。
- `corporate_actions.action_type` 仅允许分红、送转/拆分、除权除息三类预留值；本阶段不基于该表修改持仓、成本或交易流水。
- `market_quotes` 必填 `data_source_id`、`market_timestamp`、`fetched_at`、`delay_status`、`source`，并以 `(security_id, data_source_id, market_timestamp)` 去重；没有任何模拟或硬编码行情写入逻辑。
- `market_snapshots` 与 `market_quotes` 分别按来源和证券时间建立索引，支持后续可追溯查询。
- `market_index_quotes` 按指数代码与行情时间保存源数据。迁移 `019` 增加可空的 `open_price`、`high_price`、`low_price`、`turnover_amount`，只保存 Provider 实际返回的日内字段；旧快照保持可读且显示“暂无数据”，不回填或推测。Dashboard 读取最近一次来源可追溯的指数快照。
- `news_articles` 强制要求标题、来源、来源类型、发布时间、抓取时间、摘要和 HTTP(S) 原文地址；原文地址唯一。迁移 `017` 以 `news_security_links` 支持共享关联，资讯页只返回仍关联当前关注标的的记录。
- `daily_reviews.review_date` 唯一；同一日期重新生成时原子更新四个摘要和关联快照，绝不生成第二条同日复盘。快照删除时复盘保留但 `snapshot_id` 设为空，以保留生成时的结构化事实和“未确认”状态。
- `ai_reviews` 使用外键关联 `daily_reviews`，每日复盘删除时其 AI 辅助解释一并删除；`context_id` 关联冻结的 `ai_review_contexts`，后者再关联一次 `manual_refresh_runs`。迁移 `014` 新增可空 `report_json`，迁移 `016` 新增可空 `security_id`；新复盘一条记录仅对应一只关注证券。新记录必须同时保存七项报告与 `FACTS`、`INFERENCES`、`RISKS` 审计内容。运行时 API Key 不进入此表或其他 SQLite 表。
- `ai_provider_settings` 由迁移 `015` 初始化模型、优先级与启用状态；迁移 `018` 将旧 `TENCENT_HUNYUAN` 偏好迁移为 `TENCENT_TOKENHUB`。迁移 `020` 增加非敏感的 `endpoint` 与 `model_id`，并将 TokenHub 默认模型更新为 `hy3`。旧腾讯混元 Keychain 项保留但不被 TokenHub 读取；TokenHub Key 只存新的 macOS Keychain 项。运行时只选择优先级最高且同时“已启用/已配置”的一个 Provider，绝不自动并行调用多个模型。
- `events.event_type` 支持 `EARNINGS`、`COMPANY_ANNOUNCEMENT`、`MAJOR_MATTER`、`DIVIDEND`、`EX_DIVIDEND`、`SHAREHOLDER_MEETING`、`MACRO_DATA`、`FED_MEETING`；`status` 仅允许 `CONFIRMED`、`UNCONFIRMED`、`ARCHIVED`。迁移 `017` 以 `event_security_links` 支持共享关联；查询先标识当前关注关联事件，再按解析后的原始带时区时间排序，不推断日期或状态。

- V1.0.7 的“确认删除关注”是用户确认后的不可恢复操作。Rust `remove_followed_security_completely` 在单一 SQLite transaction 中删除目标证券的关注关系、持仓/交易、个股行情、公司行动、独有资讯/事件、逐证券 AI 复盘与独有 AI Context，并最终删除证券记录；任一步失败即 rollback。市场指数快照、数据源、现金账户、每日全局复盘及仍被其他证券关联的资讯/事件不会被删除。
- `manual_refresh_news_articles` 与 `manual_refresh_events` 仅关联一次用户主动刷新中由 Adapter 实际返回、经校验并已保存的记录。AI Context 从这两张关联表读取，不会在生成 AI 复盘时重新请求数据或补造新闻、事件。
- `app_settings` 仅保存产品运行状态。首次启动向导使用 `initialization_completed=true` 表示完成；该值在数据库重启后保留。不得将配置密钥、个人凭据或任何金融数据写入该表。

## 5. 派生计算（后续阶段）

- **可卖数量：** 截至交易日、尚未卖出的买入 lot 数量之和，排除当日新建 lot。
- **已实现盈亏：** 已卖出 lot 的卖出净额减去对应成本和费用；成本分配规则需固定并版本化（建议 FIFO）。
- **未实现盈亏：** 当前市值减剩余持仓成本；缺少可靠报价时为未知而非零。
- **今日盈亏/总盈亏：** 明确估值时间、昨收/成本和费用口径，且保留所用行情记录引用。
