# 技术架构设计

## 1. 结论

建议采用 **Tauri 2 + React + TypeScript + Rust + SQLite** 的本地优先桌面架构。Tauri 负责 Windows/macOS 桌面壳与安全边界，React 负责界面，Rust 承担领域规则、数据采集编排与持久化访问，SQLite 保存长期本地数据。

## 2. 分层

```text
React/TypeScript UI
  └─ Tauri command / event 边界
       └─ Rust application services
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
| Information | 新闻、公告、社区内容与证券关联 | 原文链接可追溯；社区内容非事实 |
| Calendar | 公司和宏观事件 | 类型、日期、来源、确认状态均需保留 |
| Review AI | 证据选择、结构化生成、人工复盘保存 | 强制 FACTS/INFERENCES/RISKS；不得提供交易承诺 |

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
