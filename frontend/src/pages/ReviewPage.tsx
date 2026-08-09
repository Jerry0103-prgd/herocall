import { useCallback, useEffect, useState } from "react";

import {
  generateAiReviews,
  loadAiProviderConfigs,
  loadAiReviewsForDate,
  type AiReview,
  type AiProviderConfig,
} from "../services/ai";

const reportLabels = [
  ["当前个股情况", "stockStatus"],
  ["当前市场环境影响", "marketAnalysis"],
  ["所属板块分析", "sectorAnalysis"],
  ["消息面分析", "newsAnalysis"],
  ["技术面分析", "technicalAnalysis"],
  ["策略参考", "strategyReference"],
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

function ReportTable({ review }: { review: AiReview }) {
  const title = `${review.securityName ?? "未确认标的"}${review.securitySymbol ? ` · ${review.securitySymbol}` : ""}`;
  return (
    <article className="ai-report-wrap">
      <div className="ai-report-heading">
        <div><p className="section-kicker">{review.model}</p><h3>{title}</h3></div>
        <span>仅供信息整理与观察，不构成交易建议。</span>
      </div>
      <div className="ai-report-table-scroll"><table className="ai-report-table"><thead><tr><th>分析维度</th><th>AI复盘结果</th></tr></thead><tbody>{reportLabels.map(([label, key]) => <tr key={key}><th scope="row">{label}</th><td>{review.report?.[key] ?? "暂无数据"}</td></tr>)}</tbody></table></div>
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
    try {
      const generated = await generateAiReviews(reviewDate);
      setReviews(generated.filter((review) => review.securityId !== null));
      setState("success");
      setMessage(`已生成 ${generated.length} 只关注标的的 AI复盘`);
    } catch (error) {
      setState("failed");
      setMessage(safeErrorMessage(error));
    }
  }

  const selectedProvider = providers.find((provider) => provider.enabled && provider.configured);

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
      {!isLoading ? <div className="ai-active-provider" aria-label="当前AI模型"><span>当前模型</span><strong>{selectedProvider ? selectedProvider.displayName : "暂无已启用模型"}</strong>{selectedProvider ? <small>{selectedProvider.model}</small> : null}</div> : null}
      {!selectedProvider && !isLoading ? <div className="settings-card ai-empty-state">请先在设置中配置并开启一个 AI Provider。</div> : null}
      {isLoading ? <p className="table-state review-state">正在读取已保存的 AI复盘…</p> : null}
      {!isLoading && selectedProvider && reviews.length === 0 ? <div className="settings-card ai-empty-state">尚未生成当日 AI复盘。请先更新今日市场快照，再点击“生成AI复盘”。</div> : null}
      {!isLoading && reviews.length > 0 ? <section className="ai-review-list" aria-label="AI复盘报告列表">{reviews.map((review) => <ReportTable key={review.id} review={review} />)}</section> : null}
    </section>
  );
}
