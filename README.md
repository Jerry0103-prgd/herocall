# AStock-AI-Workbench

> A股个人 AI 投研工作台

一个本地优先、面向个人长期使用的 A 股投研与每日复盘桌面应用。它用于帮助用户了解资产变化、持仓异动、市场主线、相关新闻/公告/事件，并沉淀投资复盘。

## 当前状态

项目已完成基础工程初始化：包含 Tauri 2、React、TypeScript、Rust 和 SQLite 插件连接框架。当前只提供初始化状态窗口，不包含金融业务、行情接口、股票数据、AI 功能或 SQLite 业务表。

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
src-tauri/   Tauri 2 + Rust + SQLite 插件连接框架
database/    未来 SQLite 数据文件的位置（当前无数据库、无业务表）
docs/        后续集中存放扩展文档
tests/       后续测试目录
```
