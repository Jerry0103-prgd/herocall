import type { MarketIndexQuote } from "../services/dashboard";

type MarketIndexCardProps = {
  quote: MarketIndexQuote;
};

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

function formatTurnover(value: string | null) {
  // Adapter records the supplier-declared unit. Do not infer or relabel it in the UI.
  return value || "暂无数据";
}

export function MarketIndexCard({ quote }: MarketIndexCardProps) {
  const tone = changeTone(quote.changePercent);
  return (
    <article className="index-card">
      <div className="index-heading">
        <div>
          <h3>{quote.name}</h3>
          <p>{quote.symbol}</p>
        </div>
      </div>
      <div className="index-current-value">
        <strong>{quote.currentPrice ?? "暂无数据"}</strong>
        <b className={`index-change index-change--${tone}`}>{formatChangePercent(quote.changePercent)}</b>
      </div>
      <div className="index-intraday-grid">
        <span>今开<b>{quote.openPrice ?? "暂无数据"}</b></span>
        <span>最高<b>{quote.highPrice ?? "暂无数据"}</b></span>
        <span>最低<b>{quote.lowPrice ?? "暂无数据"}</b></span>
        <span>成交额<b>{formatTurnover(quote.turnoverAmount)}</b></span>
      </div>
      <p className="index-updated-at">更新时间：{formatBeijingTime(quote.updatedAt)}</p>
    </article>
  );
}
