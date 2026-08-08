import { useCallback, useEffect, useState } from "react";

import { MarketIndexCard } from "../components/MarketIndexCard";
import { MetricCard } from "../components/MetricCard";
import {
  emptyAssetSummary,
  loadAssetSummary,
  loadMarketSnapshot,
  noDataMarketSnapshot,
  refreshTodayMarketSnapshot,
  type AssetSummary,
  type MarketIndexQuote,
} from "../services/dashboard";

const metrics = [
  ["总资产", "totalAssets"],
  ["股票市值", "stockMarketValue"],
  ["现金", "cash"],
  ["今日盈亏", "dailyPnl"],
  ["总盈亏", "totalPnl"],
  ["收益率", "returnRate"],
] as const;

export function DashboardPage() {
  const [summary, setSummary] = useState<AssetSummary>(emptyAssetSummary);
  const [indices, setIndices] = useState<MarketIndexQuote[]>(noDataMarketSnapshot);
  const [connectionNotice, setConnectionNotice] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  const loadDashboard = useCallback(async () => {
    const [summaryResult, marketResult] = await Promise.allSettled([
      loadAssetSummary(),
      loadMarketSnapshot(),
    ]);
    if (summaryResult.status === "fulfilled") setSummary(summaryResult.value);
    if (marketResult.status === "fulfilled") setIndices(marketResult.value);
    if (summaryResult.status === "rejected" || marketResult.status === "rejected") {
      setConnectionNotice("本地服务暂未返回可验证数据");
    }
  }, []);

  useEffect(() => {
    void loadDashboard();
  }, [loadDashboard]);

  async function refreshMarketData() {
    setIsRefreshing(true);
    try {
      const result = await refreshTodayMarketSnapshot();
      const holdingsMessage = result.holdings.status === "NO_DATA"
        ? (result.holdings.message ?? "暂无可验证持仓行情")
        : `持仓 ${result.holdings.quoteCount} 条（${result.holdings.source} · ${result.holdings.status}）`;
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
    } catch {
      setConnectionNotice("行情刷新失败；未写入任何替代价格。");
    } finally {
      setIsRefreshing(false);
    }
  }

  return (
    <section className="page dashboard-page" aria-labelledby="dashboard-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">Portfolio overview</p>
          <h1 id="dashboard-title">今日总览</h1>
          <p>本地资产与市场快照。仅展示可追溯、已验证的数据。</p>
        </div>
        <div className="dashboard-actions"><span className="readonly-badge">只读模式</span><button className="secondary-button" disabled={isRefreshing} onClick={() => void refreshMarketData()} type="button">{isRefreshing ? "正在更新…" : "更新今日市场快照"}</button></div>
      </header>

      {connectionNotice ? <p className="notice" role="status">{connectionNotice}</p> : null}

      <section aria-label="资产摘要">
        <div className="section-heading">
          <div><p className="section-kicker">资产</p><h2>资产摘要</h2></div>
          <span>估值数据：{summary.valuationSource ?? "暂无数据"}{summary.valuationTimestamp ? ` · ${summary.valuationTimestamp}` : ""}</span>
        </div>
        <div className="metric-grid">
          {metrics.map(([label, key]) => (
            <MetricCard key={key} label={label} value={summary[key]} />
          ))}
        </div>
      </section>

      <section className="market-section" aria-label="A股主要指数">
        <div className="section-heading">
          <div><p className="section-kicker">市场</p><h2>A股主要指数</h2></div>
          <span>仅在手动更新快照后变更；显示来源、状态与最后更新时间</span>
        </div>
        <div className="index-grid">
          {indices.map((quote) => <MarketIndexCard key={quote.symbol} quote={quote} />)}
        </div>
      </section>
    </section>
  );
}
