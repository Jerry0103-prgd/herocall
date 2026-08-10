import { useCallback, useEffect, useState } from "react";

import { MarketIndexCard } from "../components/MarketIndexCard";
import { DataStatusCard, type DataStatusTone } from "../components/DataStatusCard";
import type { PageId } from "../components/Sidebar";
import { loadAiProviderConfigs, type AiProviderConfig } from "../services/ai";
import {
  loadDashboardDataStatus,
  loadMarketSnapshot,
  noDataMarketSnapshot,
  refreshTodayMarketSnapshot,
  type DashboardDataStatus,
  type DataSectionStatus,
  type MarketIndexQuote,
} from "../services/dashboard";

const noDataSection: DataSectionStatus = {
  status: "NO_DATA",
  source: null,
  itemCount: 0,
  updatedAt: null,
  delayStatus: null,
};

const noDataStatus: DashboardDataStatus = { market: noDataSection, news: noDataSection };

function formatStatusTime(value: string | null) {
  if (!value) return "暂无数据";
  const timestamp = new Date(value);
  if (Number.isNaN(timestamp.getTime())) return value;
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit",
    hour: "2-digit", minute: "2-digit", hourCycle: "h23",
  }).formatToParts(timestamp);
  const part = (type: Intl.DateTimeFormatPartTypes) => parts.find((item) => item.type === type)?.value ?? "";
  return `${part("year")}-${part("month")}-${part("day")} ${part("hour")}:${part("minute")}`;
}

function sectionTone(section: DataSectionStatus): DataStatusTone {
  return section.status === "SYNCED" ? "success" : "empty";
}

type DashboardPageProps = {
  onNavigate: (page: PageId) => void;
};

export function DashboardPage({ onNavigate }: DashboardPageProps) {
  const [indices, setIndices] = useState<MarketIndexQuote[]>(noDataMarketSnapshot);
  const [dataStatus, setDataStatus] = useState<DashboardDataStatus>(noDataStatus);
  const [dataStatusFailed, setDataStatusFailed] = useState(false);
  const [aiProviders, setAiProviders] = useState<AiProviderConfig[]>([]);
  const [aiProviderStatusFailed, setAiProviderStatusFailed] = useState(false);
  const [connectionNotice, setConnectionNotice] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  const loadDashboard = useCallback(async () => {
    const [marketResult, dataStatusResult, providerResult] = await Promise.allSettled([
      loadMarketSnapshot(),
      loadDashboardDataStatus(),
      loadAiProviderConfigs(),
    ]);
    if (marketResult.status === "fulfilled") setIndices(marketResult.value);
    if (dataStatusResult.status === "fulfilled") {
      setDataStatus(dataStatusResult.value);
      setDataStatusFailed(false);
    } else {
      setDataStatusFailed(true);
    }
    if (providerResult.status === "fulfilled") {
      setAiProviders(providerResult.value);
      setAiProviderStatusFailed(false);
    } else {
      setAiProviderStatusFailed(true);
    }
    if (marketResult.status === "rejected") {
      setConnectionNotice("本地服务暂未返回可验证数据");
    }
  }, []);

  const currentAiProvider = aiProviders.find((provider) => provider.isCurrent);

  useEffect(() => {
    void loadDashboard();
  }, [loadDashboard]);

  async function refreshMarketData() {
    setIsRefreshing(true);
    try {
      const result = await refreshTodayMarketSnapshot();
      const holdingsMessage = result.holdings.status === "NO_DATA"
        ? (result.holdings.message ?? "暂无可验证持仓行情")
        : `关注标的行情 ${result.holdings.quoteCount} 条（${result.holdings.source} · ${result.holdings.status}）`;
      const indicesMessage = result.indices.status === "NO_DATA"
        ? "指数暂无数据"
        : `指数 ${result.indices.itemCount} 条（${result.indices.source} · ${result.indices.status}）`;
      const optionalMessage = [result.news, result.events]
        .filter((section) => section.status === "NO_DATA")
        .map((section) => section.message)
        .filter((message): message is string => Boolean(message))
        .join(" ");
      setConnectionNotice(`${holdingsMessage}；${indicesMessage}${optionalMessage ? `。${optionalMessage}` : "。"}`);
      await loadDashboard();
    } catch (error) {
      const reason = error instanceof Error && error.message ? error.message : "本地行情服务不可用";
      setConnectionNotice(`行情更新失败：${reason}`);
    } finally {
      setIsRefreshing(false);
    }
  }

  return (
    <section className="page dashboard-page" aria-labelledby="dashboard-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">Research overview</p>
          <h1 id="dashboard-title">今日总览</h1>
          <p>关注标的与市场快照。仅展示可追溯、已验证的数据。</p>
        </div>
        <div className="dashboard-actions"><button className="secondary-button" disabled={isRefreshing} onClick={() => void refreshMarketData()} type="button">{isRefreshing ? "正在更新…" : "更新今日市场快照"}</button></div>
      </header>

      {connectionNotice ? <p className="notice" role="status">{connectionNotice}</p> : null}

      <section className="data-status-section" aria-label="数据状态">
        <div className="section-heading">
          <div><p className="section-kicker">数据</p><h2>数据状态</h2></div>
          <span>仅反映最近一次手动更新保存的本地记录</span>
        </div>
        <div className="data-status-grid">
          <DataStatusCard title="行情" tone={dataStatusFailed ? "failed" : sectionTone(dataStatus.market)} headline={dataStatusFailed ? "状态读取失败" : dataStatus.market.status === "SYNCED" ? "已同步" : "暂无行情数据"}>
            {!dataStatusFailed && dataStatus.market.status === "SYNCED" ? <>
              <div><dt>来源</dt><dd>{dataStatus.market.source}</dd></div>
              <div><dt>更新时间</dt><dd>{formatStatusTime(dataStatus.market.updatedAt)}</dd></div>
              <div><dt>行情状态</dt><dd>{dataStatus.market.delayStatus ?? "未确认"}</dd></div>
            </> : null}
          </DataStatusCard>
          <DataStatusCard title="资讯" tone={dataStatusFailed ? "failed" : sectionTone(dataStatus.news)} headline={dataStatusFailed ? "状态读取失败" : dataStatus.news.status === "SYNCED" ? "已同步" : "暂无关联资讯"}>
            {!dataStatusFailed && dataStatus.news.status === "SYNCED" ? <>
              <div><dt>来源</dt><dd>{dataStatus.news.source}</dd></div>
              <div><dt>数量</dt><dd>{dataStatus.news.itemCount} 条</dd></div>
              <div><dt>更新时间</dt><dd>{formatStatusTime(dataStatus.news.updatedAt)}</dd></div>
            </> : null}
          </DataStatusCard>
          <DataStatusCard title="AI复盘" tone={aiProviderStatusFailed ? "failed" : currentAiProvider ? "success" : "empty"} headline={aiProviderStatusFailed ? "配置状态读取失败" : currentAiProvider ? "已启用" : "未启用模型"}>
            {!aiProviderStatusFailed && currentAiProvider ? <><div><dt>当前模型</dt><dd>{currentAiProvider.displayName}</dd></div><div><dt>模型标识</dt><dd>{currentAiProvider.model}</dd></div></> : null}
          </DataStatusCard>
        </div>
      </section>

      <section className="dashboard-quick-links" aria-label="研究快捷入口">
        <div className="section-heading">
          <div><p className="section-kicker">研究路径</p><h2>从关注到复盘</h2></div>
          <span>查看关注标的、关联信息并生成当日复盘</span>
        </div>
        <div className="quick-link-grid">
          <button className="quick-link-card quick-link-card--primary" disabled={isRefreshing} onClick={() => void refreshMarketData()} type="button"><span>01</span><strong>{isRefreshing ? "正在更新…" : "更新今日市场快照"}</strong><small>保存本次市场、资讯与事件数据</small></button>
          <button className="quick-link-card" onClick={() => onNavigate("holdings")} type="button"><span>02</span><strong>我的关注</strong><small>查看当前关注标的</small></button>
          <button className="quick-link-card" onClick={() => onNavigate("news")} type="button"><span>03</span><strong>个股资讯</strong><small>阅读关联资讯与来源</small></button>
          <button className="quick-link-card" onClick={() => onNavigate("review")} type="button"><span>04</span><strong>AI复盘</strong><small>生成并查看当日复盘</small></button>
        </div>
      </section>

      <section className="market-section" aria-label="A股主要指数">
        <div className="section-heading">
          <div><p className="section-kicker">市场</p><h2>A股主要指数</h2></div>
          <span>仅在手动更新快照后变更</span>
        </div>
        <div className="index-grid">
          {indices.map((quote) => <MarketIndexCard key={quote.symbol} quote={quote} />)}
        </div>
      </section>
    </section>
  );
}
