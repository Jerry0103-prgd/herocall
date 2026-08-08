import type { MarketIndexQuote } from "../services/dashboard";

type MarketIndexCardProps = {
  quote: MarketIndexQuote;
};

function displayStatus(status: MarketIndexQuote["status"]) {
  return status === "NO_DATA" ? "暂无数据" : status;
}

export function MarketIndexCard({ quote }: MarketIndexCardProps) {
  return (
    <article className="index-card">
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
        <span>{quote.changePercent ?? "暂无数据"}</span>
      </div>
      <dl className="quote-meta">
        <div><dt>来源</dt><dd>{quote.source ?? "暂无数据"}</dd></div>
        <div><dt>更新时间</dt><dd>{quote.updatedAt ?? "暂无数据"}</dd></div>
      </dl>
    </article>
  );
}
