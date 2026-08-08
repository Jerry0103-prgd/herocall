# 数据库设计（逻辑模型）

本文件描述 SQLite 逻辑模型与 V0.2 已落地的数据库核心。迁移由 Rust 服务层版本化管理；所有时间字段使用 ISO 8601 UTC，交易日期字段以 `Asia/Shanghai` 解释。

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
| `market_snapshots` | `id`, `data_source_id`, `market_timestamp`, `fetched_at`, `delay_status` | 行情抓取批次元数据；当前不接入数据源 |
| `market_quotes` | `id`, `market_snapshot_id`, `security_id`, `data_source_id`, `current_price`, `change_pct`, `market_timestamp`, `fetched_at`, `delay_status` | 单条行情及完整来源/时间/延迟元数据；当前不刷新行情 |
| `corporate_actions` | `id`, `security_id`, `action_type`, `announcement_date`, `effective_date`, `data_source_id`, `source_url`, `details_json`, `status` | 公司行动预留；支持 `DIVIDEND`、`SPLIT`、`EX_RIGHT` 的公告/事件记录，不自动调整持仓 |

`schema_migrations` 是迁移系统内部表，保存迁移版本、校验标识与应用时间。当前已定义 `001`（数据库核心）和 `002`（金融领域字段补充）；迁移重复执行不会重新建表，已应用迁移的校验标识不匹配会阻止继续启动。

## 3. 延后实现的逻辑实体

`position_lots`、资讯、事件日历和 AI 复盘相关表保留在后续阶段实现。本阶段没有创建这些表，也没有实现自动交易、金融计算、行情刷新或外部数据接入。

## 4. 关键约束与索引

- `securities` 的 `(symbol, market)` 唯一；`security_type` 仅允许 `STOCK` 或 `ETF`，`trade_rule` 仅允许 `T_PLUS_1`、`T_PLUS_0` 或 `UNKNOWN`。
- `holdings` 的 `(cash_account_id, security_id)` 唯一，且 `available_quantity` 不可超过持仓数量；`cost_amount` 是为后续精确成本计算预留的 decimal 文本。
- `transactions` 保留佣金、印花税、过户费、其他费用和最低佣金字段，并以 `(cash_account_id, security_id, trade_date)` 及 `(status, trade_date)` 建索引。交易取消只更新状态，不删除历史；本阶段不执行成本、可卖数、T+1 或盈亏规则。
- `corporate_actions.action_type` 仅允许分红、送转/拆分、除权除息三类预留值；本阶段不基于该表修改持仓、成本或交易流水。
- `market_quotes` 必填 `data_source_id`、`market_timestamp`、`fetched_at`、`delay_status`，并以 `(security_id, data_source_id, market_timestamp)` 去重；没有任何模拟或硬编码行情写入逻辑。
- `market_snapshots` 与 `market_quotes` 分别按来源和证券时间建立索引，支持后续可追溯查询。

## 5. 派生计算（后续阶段）

- **可卖数量：** 截至交易日、尚未卖出的买入 lot 数量之和，排除当日新建 lot。
- **已实现盈亏：** 已卖出 lot 的卖出净额减去对应成本和费用；成本分配规则需固定并版本化（建议 FIFO）。
- **未实现盈亏：** 当前市值减剩余持仓成本；缺少可靠报价时为未知而非零。
- **今日盈亏/总盈亏：** 明确估值时间、昨收/成本和费用口径，且保留所用行情记录引用。
