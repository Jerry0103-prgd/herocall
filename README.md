# Hero Call

> AI Portfolio Assistant

> A股个人 AI 投研工作台

一个本地优先、面向个人长期使用的 A 股投研与每日复盘桌面应用。它用于帮助用户了解资产变化、持仓异动、市场主线、相关新闻/公告/事件，并沉淀投资复盘。

## 当前状态

项目已完成基础工程初始化、金融领域数据库、Portfolio Engine、Market Data Engine、Dashboard、我的关注、设置、个股资讯、AI复盘、事件日历和首次启动向导：包含 Tauri 2、React、TypeScript、Rust、SQLite 迁移框架，以及 Tushare/东方财富/腾讯 Adapter、行情规范化与用户主动保存的市场快照。财经资讯、事件只保存可追溯的真实来源结果；没有可验证的数据时显示“暂无数据”或“未确认”。AI复盘针对每只关注标的独立生成报告，固定保留 `FACTS`、`INFERENCES`、`RISKS` 审计内容，并按当前情况、市场环境、板块、消息、技术、策略参考与结论展示。支持 DeepSeek、腾讯混元（TokenHub）、豆包的独立 Keychain 配置；每次只调用优先级最高且已开启、已配置的一个 Provider。腾讯 TokenHub 使用 OpenAI 兼容接口与 `hunyuan-turbos-latest`，连接测试只请求模型列表，不会发送投研上下文。API Key 不写入 SQLite、源码、日志或前端。AI 不提供直接交易指令、目标价、收益预测或收益承诺。

关注标的可由用户直接输入六位代码和名称创建；本地没有可验证证券资料时仅保存用户输入，后续无法识别的行情、资讯或事件明确显示“暂无数据”。“取消关注”必须经二次确认，确认后会在单一 SQLite transaction 中永久清理该标的在 Hero Call 内保存的个股数据；共享资讯/事件和全局市场指数数据会保留给其他标的或全局视图。

Dashboard 的“更新今日市场快照”是唯一的行情触发入口：每次点击只执行一次请求并保存当前持仓与四个主要指数的快照，绝不轮询、常驻连接或高频请求。默认按东方财富公开行情、腾讯公开行情的 Adapter 顺序尝试；两者都必须显示 `DELAYED`，不得称为实时。配置 Tushare Token 后会优先使用其可追溯日线数据（`CLOSED`）；Token 只保存在 macOS Keychain，前端只能看到“已配置/未配置”。新闻和事件 Adapter 尚未配置时，本次更新明确返回 `NO_DATA`，不会写入演示内容。资产摘要由 Rust 根据本地现金、有效收盘报价和持仓成本计算；缺少任何必要报价时保持“暂无数据”。

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

发布构建和 Windows CI 说明见 [RELEASE_BUILD.md](docs/RELEASE_BUILD.md)。

## 当前工程结构

```text
frontend/    React + TypeScript + Vite
src-tauri/   Tauri 2 + Rust + SQLite 迁移与数据库服务层
database/    本地 SQLite 数据文件的位置（由应用运行时创建，已忽略）
docs/        后续集中存放扩展文档
tests/       后续测试目录
```
