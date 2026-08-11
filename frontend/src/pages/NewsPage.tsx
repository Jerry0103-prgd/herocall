import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

import { loadMarketIntelligence, type IntelligenceItem, type SecurityIntelligence } from "../services/intelligence";

const credibilityLabel: Record<IntelligenceItem["credibilityLevel"], string> = {
  A: "A级 · 官方确认", B: "B级 · 权威媒体", C: "C级 · 行业信息", D: "D级 · 社区观点", E: "E级 · 未经证实",
};

const heatLabel: Record<SecurityIntelligence["discussionHeat"], string> = {
  NO_DATA: "暂无趋势判断", LOW: "低", NORMAL: "正常", HIGH: "高", SURGING: "显著升温",
};

function formatBeijingTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const parts = new Intl.DateTimeFormat("en-CA", { timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).formatToParts(date);
  const part = (type: Intl.DateTimeFormatPartTypes) => parts.find((item) => item.type === type)?.value ?? "";
  return `${part("year")}-${part("month")}-${part("day")} ${part("hour")}:${part("minute")}`;
}

function IntelligenceItemCard({ item, onOpen }: { item: IntelligenceItem; onOpen: (url: string) => void }) {
  return <article className={`intelligence-item intelligence-item--${item.credibilityLevel.toLowerCase()}`}>
    <div className="intelligence-item-heading"><div><span className="intelligence-type">{item.sourceType}</span><span className="credibility-badge">{credibilityLabel[item.credibilityLevel]}</span><h3>{item.title}</h3></div>{item.credibilityLevel === "E" ? <strong className="unverified">未经证实</strong> : null}</div>
    <p>{item.summary}</p>
    <footer><span>{item.source}</span><time>{formatBeijingTime(item.publishedAt)}</time>{item.sourceUrl ? <button onClick={() => onOpen(item.sourceUrl!)} type="button">查看原文 ›</button> : null}</footer>
  </article>;
}

export function NewsPage() {
  const [securities, setSecurities] = useState<SecurityIntelligence[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    try {
      const view = await loadMarketIntelligence();
      setSecurities(view.securities);
      setMessage(view.partialUnavailableSources.length ? `部分来源暂不可用：${view.partialUnavailableSources.join("；")}` : null);
    } catch {
      setMessage("本地市场情报服务暂不可用");
    } finally { setIsLoading(false); }
  }, []);
  useEffect(() => { void refresh(); }, [refresh]);

  async function openOriginal(url: string) {
    try { await openUrl(url); } catch { setMessage("无法打开原文链接，请稍后重试。"); }
  }

  return <section className="page intelligence-page" aria-labelledby="intelligence-title">
    <header className="page-header"><div><p className="eyebrow">Market intelligence</p><h1 id="intelligence-title">市场情报</h1><p>围绕我的关注聚合来源、主题和可信度；社区观点与传闻不会被当作客观事实。</p></div><button className="secondary-button" onClick={() => void refresh()} type="button">刷新本地情报</button></header>
    {message ? <p className="notice" role="status">{message}</p> : null}
    {isLoading ? <p className="table-state news-state">正在整理关注标的的市场情报…</p> : null}
    {!isLoading && securities.length === 0 ? <p className="table-state news-state">当前暂无值得关注的新情报</p> : null}
    {!isLoading ? <div className="intelligence-list">{securities.map((security) => <article className="intelligence-card" key={security.securityId}>
      <header><div><p className="section-kicker">{security.securitySymbol}</p><h2>{security.securityName}</h2></div><div className="intelligence-signals"><span className={`heat-tag heat-tag--${security.discussionHeat.toLowerCase()}`}>热度：{heatLabel[security.discussionHeat]}</span><span className="sentiment-tag">舆情：{security.sentiment}</span></div></header>
      <section><h3>今日焦点</h3>{security.topics.length ? <ol className="intelligence-topics">{security.topics.map((topic) => <li key={topic.title}><strong>{topic.title}</strong><small>{topic.sourceCount > 1 ? `${topic.sourceCount} 个来源正在讨论` : "单一来源"} · {topic.credibilityLevels.map((level) => `${level}级`).join(" / ")}</small></li>)}</ol> : <p className="muted">暂无可聚类主题</p>}</section>
      <section><h3>重要情报</h3>{security.importantItems.length ? <div className="intelligence-items">{security.importantItems.map((item) => <IntelligenceItemCard item={item} key={item.id} onOpen={openOriginal} />)}</div> : <p className="muted">当前暂无值得关注的新情报</p>}</section>
      {security.communityOpinions.length ? <section className="community-summary"><h3>社区观点</h3><p>社区观点，不代表事实</p><ul>{security.communityOpinions.map((opinion) => <li key={opinion}>{opinion}</li>)}</ul></section> : null}
      {security.rumors.length ? <section className="rumor-summary"><h3>市场传闻</h3><p>以下内容未经证实，须以官方公告或权威媒体为准。</p>{security.rumors.map((item) => <IntelligenceItemCard item={item} key={item.id} onOpen={openOriginal} />)}</section> : null}
      <section className="intelligence-summary"><h3>情报摘要</h3><p>{security.summary}</p></section>
    </article>)}</div> : null}
  </section>;
}
