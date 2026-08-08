# 技术架构设计

## 1. 结论

建议采用 **Tauri 2 + React + TypeScript + Rust + SQLite** 的本地优先桌面架构。Tauri 负责 Windows/macOS 桌面壳与安全边界，React 负责界面，Rust 承担领域规则、数据采集编排与持久化访问，SQLite 保存长期本地数据。

## 2. 分层

```text
React/TypeScript UI（侧栏、Dashboard、Portfolio、Settings、News 页面）
  └─ Tauri command / event 边界
       └─ Rust application services（含只读 Dashboard 查询服务）
            ├─ Domain：持仓、交易、T+1、盈亏、复盘规则
            ├─ Ports：行情/资讯/公告/事件/AI 的抽象接口
            └─ Infrastructure：SQLite repositories、Provider Adapters、密钥存储
                 └─ 外部数据源与本地数据库
```

界面层不得直接调用供应商接口或 SQLite。领域层不得依赖 HTTP、供应商字段或 UI 类型。每个外部供应商实现一个 Adapter，并返回统一的规范化模型和质量元数据。

## 3. 模块边界

| 模块 | 职责 | 关键限制 |
| --- | --- | --- |
| Portfolio | 账户、现金、持仓快照与汇总 | 不直接修改交易结果；由交易流水驱动核算 |
| Trading Ledger | 买卖记录、成本、可卖数、已/未实现盈亏 | 以交易日和 T+1 校验为准；精确金额计算 |
| Portfolio Engine | 由已确认流水推导数量、可卖数、移动平均成本、已实现/未实现盈亏和市值 | 纯 Rust 服务层；使用 decimal，不访问 UI、数据库、行情接口或 AI |
| Market Data | 股票/指数/ETF 抓取、规范化、缓存与质量标记 | `market_service` 通过 Adapter 契约接入来源；仅真实数据；保存 source、market timestamp、fetched_at、delay status |
| Information | 新闻、公告、社区内容的存储、持仓关联与来源 Adapter | 原文链接、发布时间、抓取时间和来源可追溯；社区内容非事实 |
| Calendar | 公司和宏观事件 | 类型、日期、来源、确认状态均需保留 |
| Review AI | 证据选择、结构化生成、人工复盘保存 | 强制 FACTS/INFERENCES/RISKS；不得提供交易承诺 |

Phase 5-A 的 Dashboard 通过 `get_asset_summary` 与 `get_market_snapshot` 两个 Tauri Command 读取 Rust 只读服务。当前报告聚合和指数持久化尚未实现，因此 Command 返回 `null` / `NO_DATA`，界面必须显示“暂无数据”，不得将缺失数据替换为零或演示价格。

Phase 5-B 的 Portfolio 页面通过 `get_portfolio_holdings`、`create_portfolio_holding`、`update_portfolio_holding` 与 `delete_portfolio_holding` Command 访问 Rust 应用服务。前端不得计算成本金额、市值、今日盈亏或总盈亏；Rust 使用 Portfolio Service 的精确 decimal 计算及 Market Service 的有效行情状态判断。没有可验证行情时，相关字段为“暂无数据”。

Phase 5-C 的 Settings 页面通过 `get_settings_status`、`get_cash_accounts`、`create_cash_account` 和 `create_database_backup` Command 访问 `settings_service`。Tushare Token 仅从运行时 `TUSHARE_TOKEN` 读取，服务只返回“已配置/未配置”，不保存、不回传、不记录 Token。现金账户仅支持用户手工维护的 `CNY` 记账余额。备份由 SQLite `VACUUM INTO` 生成一致性副本，写入系统 Documents 下的 `AStock-AI-Workbench/backups`，且绝不覆盖同名文件。

Phase 6-A 的财经资讯页面通过 `get_holding_news_articles` Command 读取 `news_service` 的持仓关联视图。`news_service` 负责验证并存储完整来源、发布时间、抓取时间、摘要、原文地址和关联证券；其 `NewsDataAdapter` 是官方公告、媒体与社区数据源的统一预留端口。本阶段没有外部资讯抓取、没有种子内容，社区记录仅能以 `COMMUNITY` / “社区观点”呈现。

## 4. 数据流与质量控制

1. Scheduler 或用户手动刷新调用 Provider Port。
2. Adapter 获取数据、执行格式/时间/来源校验，返回规范化记录和状态。
3. Application service 去重、持久化原始引用与规范化数据，发送更新事件给界面。
4. UI 依据 `delay_status`、`source`、`market_timestamp`、`fetched_at` 渲染数据质量；异常或缺失时显示“暂无数据/未确认”。
5. AI 仅读取已保存、可引用的证据集合，输出结构化复盘和证据链接。

## 5. 安全与跨平台

- 禁用任何券商下单集成；Tauri capabilities 采用最小权限，网络访问只开放给已登记的 Adapter 域名。
- API Key 放入 OS Keychain 或运行时环境，不进入前端包、源码、日志、数据库明文或版本控制。
- SQLite 使用迁移管理、事务和本地备份策略；敏感导出须由用户显式确认。
- 时间统一存储 ISO 8601 UTC，并保存交易所时区语义；界面以 `Asia/Shanghai` 展示交易日。
- 核算采用定点 decimal/integer，不使用 JavaScript `number` 或 Rust `f64` 作为金额权威值。

## 6. 建议的交付顺序

已建立 Rust Portfolio Engine、Market Data Adapter 契约及 SQLite 快照持久化。行情层包含 Tushare 日线 Adapter（仅 `CLOSED`）与东方财富公开行情 Adapter（交易时段始终 `DELAYED`）；HTTP 传输使用系统 `curl`，Tushare Key 只在运行时由 `TUSHARE_TOKEN` 读取且经标准输入传递，不进入命令行或日志。任何真实数据源接入之前，不向 UI 提供虚假“实时”状态。Portfolio Engine 按已确认交易日顺序处理流水；交易日有效性仍由交易日历模块在后续阶段提供。
