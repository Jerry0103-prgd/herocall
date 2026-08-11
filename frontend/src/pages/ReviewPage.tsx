import { useCallback, useEffect, useState } from "react";

import {
  generateAiReviews,
  loadAiProviderConfigs,
  loadAiReviewsForDate,
  type AiReview,
  type AiProviderConfig,
  type ResearchScore,
} from "../services/ai";

const reportLabels = [
  ["当前个股情况", "stockStatus"],
  ["市场环境分析", "marketAnalysis"],
  ["所属板块分析", "sectorAnalysis"],
  ["消息面分析", "newsAnalysis"],
  ["技术面分析", "technicalAnalysis"],
  ["策略参考", "strategyReference"],
  ["研究型操作策略", "actions"],
  ["综合结论", "conclusion"],
] as const;

type AiGenerationState = "idle" | "generating" | "success" | "failed";

function chinaToday() {
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit",
  }).formatToParts(new Date());
  const value = (type: string) => parts.find((part) => part.type === type)?.value ?? "";
  return `${value("year")}-${value("month")}-${value("day")}`;
}

function safeErrorMessage(error: unknown) {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return "AI复盘服务暂不可用";
}

const impactStars: Record<string, string> = { HIGH: "★★★★★", MEDIUM: "★★★", LOW: "★★" };
const riskLabel: Record<string, string> = { HIGH: "高风险", MEDIUM: "中风险", LOW: "低风险" };
const catalystLabel: Record<string, string> = { "1D": "未来1天", "3D": "未来3天", "7D": "未来7天" };

function ScoreMetric({ label, value }: { label: string; value: number }) {
  return <div className="research-score-metric"><span>{label}</span><strong>{value}</strong><div aria-hidden="true"><i style={{ width: `${Math.max(0, Math.min(value, 100))}%` }} /></div></div>;
}

function ResearchScoreCard({ score }: { score: ResearchScore }) {
  return <section className="research-card research-score-card"><div className="research-card-heading"><p className="section-kicker">Research score</p><h4>AI研究评分</h4><strong className="research-score-overall">{score.overall}</strong></div><p>研究状态评分，不是涨跌预测或投资建议。</p><div className="research-score-grid"><ScoreMetric label="基本面关注度" value={score.fundamentalAttention} /><ScoreMetric label="技术状态" value={score.technicalState} /><ScoreMetric label="市场热度" value={score.marketHeat} /><ScoreMetric label="情绪状态" value={score.sentimentState} /><ScoreMetric label="风险水平" value={score.riskLevel} /></div><small>{score.explanation}</small></section>;
}

function ResearchV2Cards({ review }: { review: AiReview }) {
  const report = review.report!;
  return <section className="research-v2-grid" aria-label="AI研究报告 V2">
    <section className="research-card drivers-card"><div className="research-card-heading"><p className="section-kicker">Core drivers</p><h4>今日核心驱动</h4></div><ol>{report.coreDrivers?.map((driver) => <li key={`${driver.title}-${driver.impactLevel}`}><div><strong>{driver.title}</strong><span className={`impact impact--${driver.impactLevel.toLowerCase()}`}>影响程度：{impactStars[driver.impactLevel]}</span></div><p>{driver.rationale}</p><small>依据：{driver.evidenceIds.join(" · ")}</small></li>)}</ol></section>
    {report.marketThesis ? <section className="research-card thesis-card"><div className="research-card-heading"><p className="section-kicker">Market thesis</p><h4>市场交易逻辑</h4></div><p className="thesis-summary">{report.marketThesis.summary}</p><dl><div><dt>事实</dt><dd>{report.marketThesis.facts}</dd></div><div><dt>市场预期</dt><dd>{report.marketThesis.expectations}</dd></div><div><dt>情绪驱动</dt><dd>{report.marketThesis.sentiment}</dd></div></dl><small>依据：{report.marketThesis.evidenceIds.join(" · ")}</small></section> : null}
    {report.bullBearAnalysis ? <section className="research-card bull-bear-card"><div className="research-card-heading"><p className="section-kicker">Bull / bear</p><h4>多空博弈</h4></div><div className="bull-bear-columns"><div><strong>看多逻辑</strong><ul>{report.bullBearAnalysis.bull.map((point) => <li key={point.view}><b>{point.view}</b><span>依据：{point.basis}</span></li>)}</ul></div><div><strong>看空逻辑</strong><ul>{report.bullBearAnalysis.bear.map((point) => <li key={point.view}><b>{point.view}</b><span>依据：{point.basis}</span></li>)}</ul></div></div><p className="key-divergence"><strong>当前最大分歧：</strong>{report.bullBearAnalysis.keyDivergence}</p></section> : null}
    <section className="research-card catalysts-card"><div className="research-card-heading"><p className="section-kicker">Catalysts</p><h4>未来催化</h4></div><div className="catalyst-list">{report.futureCatalysts?.map((item) => <article key={`${item.timeWindow}-${item.title}`}><strong>{catalystLabel[item.timeWindow] ?? item.timeWindow}</strong><div><b>{item.title}</b><span>{item.source} · {item.credibility}级 · {item.time}</span></div></article>)}</div></section>
    <section className="research-card risks-card"><div className="research-card-heading"><p className="section-kicker">Risk factors</p><h4>风险因素</h4></div><ol>{report.riskFactors?.map((risk) => <li key={`${risk.level}-${risk.title}`}><span className={`risk-level risk-level--${risk.level.toLowerCase()}`}>{riskLabel[risk.level] ?? risk.level}</span><div><strong>{risk.title}</strong><p>{risk.reason}</p></div></li>)}</ol></section>
    {report.researchScore ? <ResearchScoreCard score={report.researchScore} /> : null}
  </section>;
}

function ReportTable({ review }: { review: AiReview }) {
  const title = `${review.securityName ?? "未确认标的"}${review.securitySymbol ? ` · ${review.securitySymbol}` : ""}`;
  return (
    <article className="ai-report-wrap">
      <div className="ai-report-heading">
        <div><p className="section-kicker">{review.model}</p><h3>{title}</h3></div>
        <span>仅供信息整理与观察，不构成交易建议。</span>
      </div>
      {review.report?.coreDrivers?.length ? <ResearchV2Cards review={review} /> : <div className="ai-report-table-scroll"><table className="ai-report-table"><thead><tr><th>分析维度</th><th>AI复盘结果</th></tr></thead><tbody>{reportLabels.map(([label, key]) => <tr key={key}><th scope="row">{label}</th><td>{review.report?.[key] ?? "暂无数据"}</td></tr>)}</tbody></table></div>}
      <details className="ai-audit-details"><summary>查看 FACTS / INFERENCES / RISKS 审计依据</summary><div className="ai-audit-grid"><section><strong>FACTS</strong><ul>{review.facts.map((item) => <li key={item}>{item}</li>)}</ul></section><section><strong>INFERENCES</strong><ul>{review.inferences.map((item) => <li key={item}>{item}</li>)}</ul></section><section><strong>RISKS</strong><ul>{review.risks.map((item) => <li key={item}>{item}</li>)}</ul></section></div></details>
    </article>
  );
}

export function ReviewPage() {
  const [reviewDate, setReviewDate] = useState(chinaToday);
  const [reviews, setReviews] = useState<AiReview[]>([]);
  const [providers, setProviders] = useState<AiProviderConfig[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [state, setState] = useState<AiGenerationState>("idle");
  const [message, setMessage] = useState<string | null>(null);
  const [progress, setProgress] = useState<string | null>(null);

  const load = useCallback(async (date: string) => {
    setIsLoading(true);
    try {
      const [stored, configuredProviders] = await Promise.all([
        loadAiReviewsForDate(date).catch(() => []),
        loadAiProviderConfigs(),
      ]);
      const followedSecurityReviews = stored.filter((review) => review.securityId !== null);
      setReviews(followedSecurityReviews);
      setProviders(configuredProviders);
      setState(followedSecurityReviews.length > 0 ? "success" : "idle");
      setMessage(null);
    } catch (error) {
      setMessage(safeErrorMessage(error));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { void load(reviewDate); }, [load, reviewDate]);

  async function generate() {
    setState("generating");
    setMessage(null);
    setProgress("正在准备研究数据：检查关注标的行情、资讯、事件与历史走势…");
    try {
      const generated = await generateAiReviews(reviewDate);
      setReviews(generated.filter((review) => review.securityId !== null));
      setState("success");
      setProgress(null);
      setMessage(`已生成 ${generated.length} 只关注标的的 AI复盘`);
    } catch (error) {
      setState("failed");
      setProgress(null);
      setMessage(safeErrorMessage(error));
    }
  }

  const selectedProvider = providers.find((provider) => provider.isCurrent);

  return (
    <section className="page review-page" aria-labelledby="review-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">AI research center</p>
          <h1 id="review-title">AI复盘</h1>
          <p>围绕每只关注标的的行情、公告、资讯、事件与市场环境生成可追溯分析。</p>
        </div>
        <div className="review-actions"><label>复盘日期<input aria-label="复盘日期" onChange={(event) => setReviewDate(event.target.value)} type="date" value={reviewDate} /></label><button className="primary-button review-generate-button" disabled={state === "generating" || !selectedProvider} onClick={() => void generate()} type="button">{state === "generating" ? "正在生成…" : "生成AI复盘"}</button></div>
      </header>

      {message ? <p className="notice" role="status">{message}</p> : null}
      {progress ? <p className="notice" role="status">{progress}</p> : null}
      {!isLoading ? <div className="ai-active-provider" aria-label="当前AI模型"><span>当前模型</span><strong>{selectedProvider ? selectedProvider.displayName : "暂无已启用模型"}</strong>{selectedProvider ? <small>{selectedProvider.model}</small> : null}</div> : null}
      {!selectedProvider && !isLoading ? <div className="settings-card ai-empty-state">请先在设置中配置并开启一个 AI Provider。</div> : null}
      {isLoading ? <p className="table-state review-state">正在读取已保存的 AI复盘…</p> : null}
      {!isLoading && selectedProvider && reviews.length === 0 ? <div className="settings-card ai-empty-state">尚未生成当日 AI复盘。点击“生成AI复盘”后，系统会自动准备可用的研究数据。</div> : null}
      {!isLoading && reviews.length > 0 ? <section className="ai-review-list" aria-label="AI复盘报告列表">{reviews.map((review) => <ReportTable key={review.id} review={review} />)}</section> : null}
    </section>
  );
}
