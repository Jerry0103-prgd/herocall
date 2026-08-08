import { useCallback, useEffect, useState } from "react";

import { MarketIndexCard } from "../components/MarketIndexCard";
import { MetricCard } from "../components/MetricCard";
import {
  emptyAssetSummary,
  loadAssetSummary,
  loadMarketSnapshot,
  noDataMarketSnapshot,
  refreshTushareMarketData,
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
      const result = await refreshTushareMarketData();
      if (result.configurationStatus === "UNCONFIGURED") {
        setConnectionNotice("Tushare 未配置：请在受保护的运行时环境中设置 TUSHARE_TOKEN。");
      } else if (result.status === "NO_DATA") {
        setConnectionNotice(result.message ?? "暂无可验证行情数据");
      } else {
        setConnectionNotice(`已保存 ${result.quoteCount} 条 ${result.source} 收盘行情。`);
      }
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
        <div className="dashboard-actions"><span className="readonly-badge">只读模式</span><button className="secondary-button" disabled={isRefreshing} onClick={() => void refreshMarketData()} type="button">{isRefreshing ? "正在刷新…" : "刷新持仓行情"}</button></div>
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
          <span>行情未接入页面刷新</span>
        </div>
        <div className="index-grid">
          {indices.map((quote) => <MarketIndexCard key={quote.symbol} quote={quote} />)}
        </div>
      </section>
    </section>
  );
}
