import { useEffect, useState } from "react";

import { MarketIndexCard } from "../components/MarketIndexCard";
import { MetricCard } from "../components/MetricCard";
import {
  emptyAssetSummary,
  loadAssetSummary,
  loadMarketSnapshot,
  noDataMarketSnapshot,
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

  useEffect(() => {
    let isCurrent = true;
    void Promise.allSettled([loadAssetSummary(), loadMarketSnapshot()]).then((results) => {
      if (!isCurrent) return;

      const [summaryResult, marketResult] = results;
      if (summaryResult.status === "fulfilled") setSummary(summaryResult.value);
      if (marketResult.status === "fulfilled") setIndices(marketResult.value);
      if (summaryResult.status === "rejected" || marketResult.status === "rejected") {
        setConnectionNotice("本地服务暂未返回可验证数据");
      }
    });
    return () => {
      isCurrent = false;
    };
  }, []);

  return (
    <section className="page dashboard-page" aria-labelledby="dashboard-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">Portfolio overview</p>
          <h1 id="dashboard-title">今日总览</h1>
          <p>本地资产与市场快照。仅展示可追溯、已验证的数据。</p>
        </div>
        <span className="readonly-badge">只读模式</span>
      </header>

      {connectionNotice ? <p className="notice" role="status">{connectionNotice}</p> : null}

      <section aria-label="资产摘要">
        <div className="section-heading">
          <div><p className="section-kicker">资产</p><h2>资产摘要</h2></div>
          <span>估值数据：暂无数据</span>
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
