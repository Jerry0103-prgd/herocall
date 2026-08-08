import { useCallback, useEffect, useState } from "react";

import { generateDailyReview, loadDailyReview, type DailyReview } from "../services/review";

function chinaToday() {
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit",
  }).formatToParts(new Date());
  const value = (type: string) => parts.find((part) => part.type === type)?.value ?? "";
  return `${value("year")}-${value("month")}-${value("day")}`;
}

function valueOrUnavailable(value: string | null) {
  return value ?? "暂无数据";
}

export function ReviewPage() {
  const [reviewDate, setReviewDate] = useState(chinaToday);
  const [review, setReview] = useState<DailyReview | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isGenerating, setIsGenerating] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const load = useCallback(async (date: string) => {
    setIsLoading(true);
    try {
      setReview(await loadDailyReview(date));
      setMessage(null);
    } catch (error) {
      setReview(null);
      const text = error instanceof Error ? error.message : "";
      setMessage(text.includes("暂无当日复盘") ? "暂无当日复盘" : "本地复盘服务暂不可用");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { void load(reviewDate); }, [load, reviewDate]);

  async function generate() {
    setIsGenerating(true);
    try {
      setReview(await generateDailyReview(reviewDate));
      setMessage("结构化复盘已生成");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "复盘生成失败");
    } finally {
      setIsGenerating(false);
    }
  }

  return (
    <section className="page review-page" aria-labelledby="review-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">Structured daily review</p>
          <h1 id="review-title">仓位复盘</h1>
          <p>仅汇总本地 Portfolio、市场快照和持仓关联资讯；不使用 AI，不提供预测或买卖建议。</p>
        </div>
        <div className="review-actions"><label>复盘日期<input aria-label="复盘日期" onChange={(event) => setReviewDate(event.target.value)} type="date" value={reviewDate} /></label><button className="primary-button" disabled={isGenerating} onClick={() => void generate()} type="button">{isGenerating ? "正在生成…" : "生成当日复盘"}</button></div>
      </header>

      {message ? <p className="notice" role="status">{message}</p> : null}
      {isLoading ? <p className="table-state review-state">正在读取本地复盘…</p> : null}
      {!isLoading && !review ? <p className="table-state review-state">暂无当日复盘</p> : null}
      {!isLoading && review ? <div className="review-content">
        <section className="review-section" aria-labelledby="account-review-title"><div className="section-heading"><div><p className="section-kicker">Account</p><h2 id="account-review-title">账户表现</h2></div><span>{review.reviewDate}</span></div><div className="review-metrics"><div className="settings-card"><span>总资产</span><strong>{valueOrUnavailable(review.portfolioSummary.totalAssets)}</strong></div><div className="settings-card"><span>今日盈亏</span><strong>{valueOrUnavailable(review.portfolioSummary.dailyPnl)}</strong></div><div className="settings-card"><span>收益率</span><strong>{valueOrUnavailable(review.portfolioSummary.returnRate)}</strong></div></div></section>

        <section className="review-section" aria-labelledby="market-review-title"><div className="section-heading"><div><p className="section-kicker">Market</p><h2 id="market-review-title">市场表现</h2></div><span>{review.marketSummary.snapshot ? `快照：${review.marketSummary.snapshot.status}` : "暂无当日市场快照"}</span></div><div className="review-index-list">{review.marketSummary.majorIndices.map((index) => <div className="settings-card review-index" key={index.symbol}><strong>{index.name}</strong><span>{index.symbol}</span><b>{valueOrUnavailable(index.changePercent)}</b><small>来源：{index.source ?? "暂无数据"} · 状态：{index.status}</small></div>)}</div></section>

        <section className="review-section" aria-labelledby="holding-review-title"><div className="section-heading"><div><p className="section-kicker">Holdings</p><h2 id="holding-review-title">持仓影响</h2></div><span>按今日盈亏贡献排序</span></div><div className="review-contribution-list">{review.holdingSummary.contributions.length === 0 ? <p className="table-state">暂无持仓</p> : review.holdingSummary.contributions.map((holding) => <div className="review-contribution" key={holding.symbol}><div><strong>{holding.name}</strong><span>{holding.symbol}</span></div><span>今日盈亏：{valueOrUnavailable(holding.dailyPnl)}</span><span>涨跌幅：{valueOrUnavailable(holding.changePercent)}</span></div>)}</div></section>

        <section className="review-section" aria-labelledby="risk-review-title"><div className="section-heading"><div><p className="section-kicker">Facts only</p><h2 id="risk-review-title">风险提示</h2></div></div><div className="settings-card risk-card"><p>以下仅为已保存数据的事实性状态说明，不构成预测或交易建议。</p><ul>{review.riskSummary.facts.map((fact) => <li key={fact}>{fact}</li>)}</ul></div></section>
      </div> : null}
    </section>
  );
}
