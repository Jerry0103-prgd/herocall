import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

import { loadMarketRadar, type RadarEvent } from "../services/intelligence";

const impactLabel: Record<RadarEvent["potentialImpact"], string> = {
  POSITIVE: "偏积极", NEUTRAL: "偏中性", NEGATIVE: "偏负面", UNCERTAIN: "不确定",
};

function formatBeijingTime(value: string) {
  const date = new Date(value); if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("zh-CN", { timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(date).replaceAll("/", "-");
}

function RadarColumn({ title, events, onOpen }: { title: string; events: RadarEvent[]; onOpen: (url: string) => void }) {
  return <section className="radar-period"><h2>{title}</h2>{events.length === 0 ? <p>暂无已识别的重要事件</p> : <div className="radar-events">{events.map((event) => <article className="radar-event" key={event.id}>
    <time>{formatBeijingTime(event.eventTime)}</time><div><span className="event-type">{event.eventType}</span><h3>{event.title}</h3><p>{event.relatedSecurity ?? "未确认关联标的"}</p></div>
    <footer><span>来源：{event.source}</span><span className="credibility-badge">{event.credibilityLevel}级</span><span className="impact-tag">潜在影响：{impactLabel[event.potentialImpact]}</span>{event.sourceUrl ? <button onClick={() => onOpen(event.sourceUrl!)} type="button">原文 ›</button> : null}</footer>
  </article>)}</div>}</section>;
}

export function EventCalendarPage() {
  const [radar, setRadar] = useState<{ next24Hours: RadarEvent[]; next3Days: RadarEvent[]; next7Days: RadarEvent[] } | null>(null);
  const [isLoading, setIsLoading] = useState(true); const [message, setMessage] = useState<string | null>(null);
  const refresh = useCallback(async () => { setIsLoading(true); try { setRadar(await loadMarketRadar()); setMessage(null); } catch { setMessage("本地市场雷达服务暂不可用"); } finally { setIsLoading(false); } }, []);
  useEffect(() => { void refresh(); }, [refresh]);
  async function openOriginal(url: string) { try { await openUrl(url); } catch { setMessage("无法打开原文链接，请稍后重试。"); } }
  return <section className="page radar-page" aria-labelledby="radar-title"><header className="page-header"><div><p className="eyebrow">Market radar</p><h1 id="radar-title">市场雷达</h1><p>聚焦未来可能影响我的关注标的、且带来源与确认状态的重要节点；潜在影响不代表确定结果。</p></div><button className="secondary-button" onClick={() => void refresh()} type="button">刷新本地雷达</button></header>
    {message ? <p className="notice" role="status">{message}</p> : null}
    {isLoading ? <p className="table-state event-state">正在整理未来市场事件…</p> : null}
    {!isLoading && radar ? <div className="radar-grid"><RadarColumn events={radar.next24Hours} onOpen={openOriginal} title="未来24小时" /><RadarColumn events={radar.next3Days} onOpen={openOriginal} title="未来3天" /><RadarColumn events={radar.next7Days} onOpen={openOriginal} title="未来7天" /></div> : null}
  </section>;
}
