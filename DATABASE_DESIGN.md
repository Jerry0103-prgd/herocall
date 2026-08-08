# 数据库设计（逻辑模型）

本文件描述拟采用 SQLite 的逻辑模型，不代表当前阶段已创建数据库或迁移脚本。所有时间字段使用 ISO 8601 UTC；交易日相关规则同时保存 `trading_date`（`Asia/Shanghai`）。

## 1. 建模原则

- 用户输入的交易流水不可被汇总值覆盖；持仓与盈亏可重算。
- 外部数据需保存来源、市场时间、抓取时间、质量/延迟状态与原始链接或标识。
- 金额、价格、数量存为 decimal 文本或定点整数及其 scale，禁止以浮点数作为权威核算值。
- 外键、唯一索引和事务保障关联完整性；数据迁移版本化。

## 2. 核心实体

| 表 | 主要字段 | 说明 |
| --- | --- | --- |
| `accounts` | `id`, `name`, `currency`, `cash_balance`, `created_at` | 本地资产账户；不含券商授权 |
| `instruments` | `id`, `symbol`, `name`, `market`, `instrument_type`, `industry`, `concepts_json` | 股票、指数或 ETF；`symbol + market` 唯一 |
| `trade_records` | `id`, `account_id`, `instrument_id`, `side`, `trade_date`, `quantity`, `price`, `fees`, `created_at` | 用户手工买入/卖出记录，`side` 为 BUY/SELL |
| `position_lots` | `id`, `buy_trade_id`, `account_id`, `instrument_id`, `acquired_date`, `original_quantity`, `remaining_quantity` | 用于 T+1 可卖数与先进先出/指定成本规则的可审计计算 |
| `position_snapshots` | `id`, `account_id`, `instrument_id`, `as_of`, `quantity`, `cost_basis`, `market_value`, `unrealized_pnl` | 可重建的查询快照，标记计算版本 |
| `market_quotes` | `id`, `instrument_id`, `price`, `change_pct`, `market_timestamp`, `fetched_at`, `source`, `delay_status` | 股票、指数、ETF 行情；需包含数据质量元数据 |
| `sources` | `id`, `name`, `source_type`, `base_url`, `trust_level`, `enabled` | 数据源登记；不保存 API Key |
| `information_items` | `id`, `source_id`, `kind`, `title`, `published_at`, `fetched_at`, `url`, `content_hash`, `community_opinion` | 新闻、公告、社区内容；社区项 `community_opinion=true` |
| `information_instruments` | `information_id`, `instrument_id`, `relation_type` | 资讯与证券多对多关联 |
| `calendar_events` | `id`, `instrument_id?`, `event_type`, `event_date`, `title`, `status`, `source_id`, `source_url`, `confirmed_at` | 公司/宏观事件；无确认时状态为 UNCONFIRMED |
| `daily_reviews` | `id`, `review_date`, `facts_md`, `inferences_md`, `risks_md`, `evidence_json`, `model_info`, `created_at` | AI 复盘和可追溯证据 |
| `review_notes` | `id`, `review_id`, `content`, `created_at`, `updated_at` | 用户个人复盘笔记 |

## 3. 关键约束与索引

- `trade_records.quantity > 0`、`price >= 0`、`fees >= 0`；卖出在写入事务中校验截至该交易日的可卖数量。
- 当日买入的 `position_lots` 不计入同一 `trade_date` 的可卖数量，满足 A 股 T+1。
- `market_quotes` 必填 `source`、`market_timestamp`、`fetched_at`、`delay_status`；按 `(instrument_id, market_timestamp, source)` 去重或保留版本。
- `information_items` 对 `(source_id, url)` 或 `(source_id, content_hash)` 建唯一约束；来源不可确认时不得入库为事实资讯。
- 为 `trade_records(account_id, instrument_id, trade_date)`、`market_quotes(instrument_id, market_timestamp DESC)`、`calendar_events(event_date)`、`information_instruments(instrument_id)` 建索引。
- `daily_reviews` 必须同时保留事实、推断、风险三个字段；`evidence_json` 仅引用已保存的行情、资讯或事件记录。

## 4. 派生计算

- **可卖数量：** 截至交易日、尚未卖出的买入 lot 数量之和，排除当日新建 lot。
- **已实现盈亏：** 已卖出 lot 的卖出净额减去对应成本和费用；成本分配规则需固定并版本化（建议 FIFO）。
- **未实现盈亏：** 当前市值减剩余持仓成本；缺少可靠报价时为未知而非零。
- **今日盈亏/总盈亏：** 明确估值时间、昨收/成本和费用口径，且保留所用行情记录引用。
