import type { MarketIndexQuote } from "../services/dashboard";

type MarketIndexCardProps = {
  quote: MarketIndexQuote;
};

function displayStatus(status: MarketIndexQuote["status"]) {
  if (status === "REALTIME") return "实时";
  if (status === "DELAYED") return "延迟";
  if (status === "CLOSED") return "已收盘";
  return "暂无数据";
}

function changeTone(value: string | null) {
  const numericValue = Number.parseFloat(value?.replace("%", "") ?? "");
  if (!Number.isFinite(numericValue) || numericValue === 0) return "flat";
  return numericValue > 0 ? "up" : "down";
}

function formatChangePercent(value: string | null) {
  if (!value) return "暂无数据";
  const normalized = value.trim();
  return normalized.endsWith("%") ? normalized : `${normalized}%`;
}

function formatBeijingTime(value: string | null) {
  if (!value) return "暂无数据";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit",
    hour: "2-digit", minute: "2-digit", hourCycle: "h23",
  }).formatToParts(date);
  const part = (type: Intl.DateTimeFormatPartTypes) => parts.find((item) => item.type === type)?.value ?? "";
  return `${part("year")}-${part("month")}-${part("day")} ${part("hour")}:${part("minute")}`;
}

export function MarketIndexCard({ quote }: MarketIndexCardProps) {
  const tone = changeTone(quote.changePercent);
  return (
    <article className={`index-card index-card--${tone}`}>
      <div className="index-heading">
        <div>
          <h3>{quote.name}</h3>
          <p>{quote.symbol}</p>
        </div>
        <span className={`status-badge status-badge--${quote.status.toLowerCase()}`}>
          {displayStatus(quote.status)}
        </span>
      </div>
      <div className="index-values">
        <strong>{quote.currentPrice ?? "暂无数据"}</strong>
        <span className="index-change">{formatChangePercent(quote.changePercent)}</span>
      </div>
      <dl className="quote-meta">
        <div><dt>来源</dt><dd>{quote.source ?? "暂无数据"}</dd></div>
        <div><dt>更新时间</dt><dd>{formatBeijingTime(quote.updatedAt)}</dd></div>
        <div><dt>数据状态</dt><dd>{displayStatus(quote.status)}</dd></div>
      </dl>
    </article>
  );
}
