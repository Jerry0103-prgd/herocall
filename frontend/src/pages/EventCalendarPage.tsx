import { useCallback, useEffect, useState } from "react";

import { loadCalendarEvents, type CalendarEvent, type EventStatus } from "../services/events";

const statusLabels: Record<"ALL" | EventStatus, string> = {
  ALL: "全部状态",
  CONFIRMED: "已确认",
  UNCONFIRMED: "未确认",
  ARCHIVED: "已归档",
};

const typeLabels: Record<CalendarEvent["eventType"], string> = {
  EARNINGS: "财报",
  DIVIDEND: "分红",
  EX_DIVIDEND: "除权除息",
  SHAREHOLDER_MEETING: "股东大会",
  MACRO_DATA: "宏观数据",
  FED_MEETING: "美联储会议",
};

export function EventCalendarPage() {
  const [status, setStatus] = useState<"ALL" | EventStatus>("ALL");
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async (nextStatus: "ALL" | EventStatus) => {
    setIsLoading(true);
    try {
      setEvents(await loadCalendarEvents(nextStatus === "ALL" ? undefined : nextStatus));
      setMessage(null);
    } catch {
      setMessage("本地事件服务暂不可用");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(status); }, [refresh, status]);

  return (
    <section className="page event-calendar-page" aria-labelledby="event-calendar-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">Investment events</p>
          <h1 id="event-calendar-title">事件日历</h1>
          <p>仅展示已保存且可追溯的事件；持仓相关事件优先，日期按原始带时区时间排序。</p>
        </div>
        <label className="event-filter">状态<select aria-label="事件状态" onChange={(event) => setStatus(event.target.value as "ALL" | EventStatus)} value={status}>{Object.entries(statusLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>
      </header>

      {message ? <p className="notice" role="status">{message}</p> : null}
      {isLoading ? <p className="table-state event-state">正在读取本地事件…</p> : null}
      {!isLoading && events.length === 0 ? <p className="table-state event-state">暂无事件</p> : null}
      {!isLoading && events.length > 0 ? <div className="event-list">{events.map((calendarEvent) => (
        <article className={`event-card ${calendarEvent.holdingRelated ? "is-holding-related" : ""}`} key={calendarEvent.id}>
          <time dateTime={calendarEvent.eventTime}><strong>{calendarEvent.eventTime}</strong><span>{calendarEvent.timezone}</span></time>
          <div className="event-main"><div className="event-title-line"><span className="event-type">{typeLabels[calendarEvent.eventType]}</span><h2>{calendarEvent.title}</h2></div><p>{calendarEvent.holdingRelated ? `持仓关联：${calendarEvent.relatedSecurity ?? "未确认"}` : calendarEvent.relatedSecurity ?? "非持仓关联事件"}</p></div>
          <div className="event-meta"><span>来源：{calendarEvent.source}</span><span className={`event-status event-status--${calendarEvent.status.toLowerCase()}`}>{statusLabels[calendarEvent.status]}</span>{calendarEvent.sourceUrl ? <a href={calendarEvent.sourceUrl} rel="noreferrer" target="_blank">原文</a> : null}</div>
        </article>
      ))}</div> : null}
    </section>
  );
}
