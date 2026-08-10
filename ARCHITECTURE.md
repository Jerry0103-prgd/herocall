# 技术架构设计

## 1. 结论

建议采用 **Tauri 2 + React + TypeScript + Rust + SQLite** 的本地优先桌面架构。Tauri 负责 Windows/macOS 桌面壳与安全边界，React 负责界面，Rust 承担领域规则、数据采集编排与持久化访问，SQLite 保存长期本地数据。

## 2. 分层

```text
React/TypeScript UI（侧栏、Dashboard、Portfolio、Settings、News、Review、EventCalendar 页面、首次启动向导）
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

Phase 5-C 的 Settings 页面通过 `get_settings_status`、`get_tushare_status`、`save_tushare_token`、`remove_tushare_token`、`get_cash_accounts`、`create_cash_account` 和 `create_database_backup` Command 访问 Rust 服务。Tushare Token 仅由系统凭据库保存（macOS 为 Keychain）；服务只返回“已配置/未配置”，不写入 SQLite、不回传、不记录 Token。现金账户仅支持用户手工维护的 `CNY` 记账余额。备份由 SQLite `VACUUM INTO` 生成一致性副本，写入系统 Documents 下的 `AStock-AI-Workbench/backups`，且绝不覆盖同名文件。

Phase 6-A 的财经资讯页面通过 `get_holding_news_articles` Command 读取 `news_service` 的持仓关联视图。`news_service` 负责验证并存储完整来源、发布时间、抓取时间、摘要、原文地址和关联证券；其 `NewsDataAdapter` 是官方公告、媒体与社区数据源的统一预留端口。本阶段没有外部资讯抓取、没有种子内容，社区记录仅能以 `COMMUNITY` / “社区观点”呈现。

Phase 6-B 的仓位复盘页面通过 `get_daily_review` 与 `generate_daily_review` Command 访问 `review_service`。服务只读取 Portfolio Service 的持仓视图、当日已保存市场快照、Dashboard 的已验证指数视图和 News Service 的持仓关联记录；不请求任何 Provider。复盘把账户、市场、持仓贡献和风险事实保存为类型化 JSON。贡献按已计算的今日盈亏降序排列；风险段只能陈述数据缺失、快照状态和已保存资讯数量，禁止预测、收益承诺与买卖建议。

Phase 7-E1 的 AI 复盘区通过 `get_ai_service_status`、`get_latest_ai_review` 和 `generate_ai_review` Command 访问 `ai_service`；设置页通过 `get_deepseek_status`、`save_deepseek_api_key`、`remove_deepseek_api_key` 管理安全配置。`DeepSeekProviderAdapter` 使用 DeepSeek 的 OpenAI Compatible Chat Completions 非流式 JSON Output 接口，密钥只保存于系统凭据库（macOS 为 Keychain），经 `curl` 标准输入传递授权头，不进入参数、日志、SQLite 或前端。每次生成必须绑定一次已保存的 `manual_refresh_runs`，冻结该次持仓 JSON 与市场快照；本阶段新闻和事件明确输入 `NO_DATA`。模型返回值必须是 JSON 的 `FACTS`、`INFERENCES`、`RISKS` 三段；保存前拒绝买卖推荐、目标价、收益预测/承诺等禁止语言，任何失败、结构异常或安全校验失败都不展示、也不落库。

V1.0.7 将旧腾讯混元原生 Provider 迁移为 `Tencent TokenHub`：`TENCENT_TOKENHUB` 经 TokenHub 的 OpenAI 兼容 `/v1/chat/completions` 调用 `hunyuan-turbos-latest`，与 DeepSeek、豆包保持相同 Adapter 边界。`test_ai_provider_connection` 仅经 `/models` 验证鉴权和当前模型可用性，不会发送 AI 复盘上下文或触发模型生成。TokenHub 使用独立 Keychain 账户；历史 `TENCENT_HUNYUAN` Keychain 项不删除也不会被读取。

Phase 6-D 的事件日历页面通过 `get_calendar_events` Command 访问 `event_service`。服务保存和验证事件类型、原始带时区时间、来源、可选原文地址、确认状态及证券关联；`EventDataAdapter` 是未来官方、媒体或宏观日历来源的预留接口。本阶段不接入外部事件抓取，不猜测事件日期。默认查询把当前持仓关联事件置前，再按实际事件时间升序排列；UI 可按确认状态过滤。

Phase 7-A 的首次启动向导通过 `get_initialization_status` 与 `complete_initialization` Command 访问 `initialization_service`。服务只在 SQLite 的 `app_settings` 保存非敏感完成标志；向导中的现金和初始持仓分别复用既有 `settings_service`、`portfolio_ui_service` Command，数据源步骤只读取安全的“已配置/未配置”状态。所有步骤均可跳过，向导不保存 Token、不连接券商，也不请求行情或 AI 服务。

Phase 7-D 的 Dashboard 通过 `refresh_today_market_snapshot` Command 调用 `market_refresh_service`。它只在用户点击“更新今日市场快照”时执行一次：当前持仓经 Tushare（日线，已配置时优先）/东方财富公开行情/腾讯公开行情的 Adapter 依次获取，四个主要指数经公开行情 Adapter 获取，并分别保存可追溯 SQLite 快照。公开行情始终标记 `DELAYED`，Tushare 日线标记 `CLOSED`；无数据则为 `NO_DATA`，绝不补价或暗中切换来源。当前新闻和事件没有已启用 Adapter，因此同一命令明确报告 `NO_DATA`，不会制造内容。无 Scheduler、定时轮询或常驻行情连接。AI 复盘只使用已保存的结构化复盘及其中关联的快照，不会重新请求行情。资产摘要完全在 Rust 中由现金账户、Portfolio Service 的有效估值和同一最新快照聚合；若任一持仓缺少可用于估值的行情，相关汇总返回空值。

## 4. 数据流与质量控制

1. 用户手动更新今日市场快照调用 Provider Port；系统不调度、不轮询。
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

已建立 Rust Portfolio Engine、Market Data Adapter 契约及 SQLite 快照持久化。行情层包含 Tushare 日线 Adapter（仅 `CLOSED`）、东方财富公开行情 Adapter 和腾讯公开行情 Adapter（公开源始终 `DELAYED`）；HTTP 传输使用系统 `curl`，Tushare Token 只由系统凭据库读取并经标准输入传递，不进入命令行或日志。任何真实数据源接入之前，不向 UI 提供虚假“实时”状态。Portfolio Engine 按已确认交易日顺序处理流水；交易日有效性仍由交易日历模块在后续阶段提供。
