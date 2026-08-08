# AStock-AI-Workbench

> A股个人 AI 投研工作台

一个本地优先、面向个人长期使用的 A 股投研与每日复盘桌面应用。它用于帮助用户了解资产变化、持仓异动、市场主线、相关新闻/公告/事件，并沉淀投资复盘。

## 当前状态

项目已完成基础工程初始化、金融领域数据库、Portfolio Engine、Market Data Engine、Dashboard、持仓管理、设置、财经资讯、仓位复盘和 AI 辅助复盘：包含 Tauri 2、React、TypeScript、Rust、SQLite 迁移框架，纯 Rust 的移动平均成本、已实现/未实现盈亏、T+1/T+0/UNKNOWN 可卖规则计算，以及 Tushare/东方财富 Adapter、行情规范化与快照持久化。财经资讯支持本地可追溯存储、持仓关联查询与官方/媒体/社区 Adapter 契约；本阶段尚未接入外部资讯源，不写入演示或虚构新闻。仓位复盘汇总账户、市场、持仓贡献排序和事实性风险状态；AI 辅助复盘只解释已保存结构化数据，固定输出 `FACTS`、`INFERENCES`、`RISKS`，并拦截投资建议、目标价和收益预测。未配置 AI Provider 时显示“AI服务未配置”，不发起请求。Dashboard、“我的持仓”和“设置”均通过 Tauri Command 访问 Rust 服务；设置页支持安全显示 Tushare 配置状态、手工新增人民币现金账户和 SQLite 立即备份。Token 不保存或显示，数据未验证或尚无后端聚合时显示“暂无数据”。当前不包含行情调度和事件日历。

## 产品边界

- 只读投研工具，不允许自动下单。
- 不连接或保存券商交易权限。
- 不制造行情，不以过期数据伪装实时数据。
- 所有行情必须包含来源、行情时间、抓取时间与延迟状态。
- AI 复盘严格区分 `FACTS`、`INFERENCES`、`RISKS`，不作收益承诺、必涨预测或必买推荐。

## 规划技术栈

| 层 | 技术 |
| --- | --- |
| 桌面应用 | Tauri 2 |
| 前端 | React + TypeScript |
| 应用后端 | Rust |
| 本地数据 | SQLite |
| 支持平台 | Windows、macOS |

## 文档导航

- [工程约定](AGENTS.md)：金融数据、AI、安全和开发不可违反的规则。
- [产品需求](PRD.md)：V1.0 范围、用户场景、验收标准。
- [技术架构](ARCHITECTURE.md)：分层、模块边界、数据流与安全设计。
- [数据库设计](DATABASE_DESIGN.md)：SQLite 逻辑模型、约束和派生计算。
- [测试计划](TEST_PLAN.md)：金融计算、数据质量、AI 输出和跨平台测试策略。

## 本地启动

前端开发服务器：

```bash
cd frontend
npm install
npm run dev
```

桌面应用（需要已安装 Rust/Cargo）：

```bash
cd frontend
npm install
npm run tauri dev
```

## 当前工程结构

```text
frontend/    React + TypeScript + Vite
src-tauri/   Tauri 2 + Rust + SQLite 迁移与数据库服务层
database/    本地 SQLite 数据文件的位置（由应用运行时创建，已忽略）
docs/        后续集中存放扩展文档
tests/       后续测试目录
```
