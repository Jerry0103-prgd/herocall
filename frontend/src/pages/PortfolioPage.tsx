import { useCallback, useEffect, useState } from "react";

import {
  loadPortfolioHoldings,
  type PortfolioHolding,
} from "../services/portfolio";

export function PortfolioPage() {
  const [holdings, setHoldings] = useState<PortfolioHolding[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    try {
      setHoldings(await loadPortfolioHoldings());
      setMessage(null);
    } catch {
      setMessage("本地持仓服务暂不可用");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  return (
    <section className="page portfolio-page" aria-labelledby="portfolio-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">Watchlist</p>
          <h1 id="portfolio-title">我的关注</h1>
          <p>以证券代码和名称记录当前关注标的；不展示账户资产、盈亏或交易指标。</p>
        </div>
      </header>

      {message ? <p className="notice" role="status">{message}</p> : null}

      <section className="portfolio-table-card" aria-label="关注标的列表">
        {isLoading ? <p className="table-state">正在读取本地关注标的…</p> : null}
        {!isLoading && holdings.length === 0 ? <p className="table-state">暂无关注标的</p> : null}
        {!isLoading && holdings.length > 0 ? (
          <div className="watchlist-list">
            {holdings.map((holding) => (
              <article className="watchlist-item" key={holding.holdingId}>
                <span className="code-cell">{holding.symbol}</span>
                <strong>{holding.name}</strong>
              </article>
            ))}
          </div>
        ) : null}
      </section>
    </section>
  );
}
