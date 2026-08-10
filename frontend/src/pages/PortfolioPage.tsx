import { useCallback, useEffect, useState } from "react";

import { WatchlistForm } from "../components/WatchlistForm";
import {
  createWatchlistItem,
  removeFollowedSecurityCompletely,
  loadPortfolioHoldings,
  type CreateWatchlistInput,
  type PortfolioHolding,
} from "../services/portfolio";

export function PortfolioPage() {
  const [holdings, setHoldings] = useState<PortfolioHolding[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [removingSecurityId, setRemovingSecurityId] = useState<number | null>(null);
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [pendingRemoval, setPendingRemoval] = useState<PortfolioHolding | null>(null);
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

  async function save(input: CreateWatchlistInput) {
    setIsSaving(true);
    try {
      await createWatchlistItem(input);
      setIsFormOpen(false);
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "关注标的保存失败");
    } finally {
      setIsSaving(false);
    }
  }

  async function remove(holding: PortfolioHolding) {
    setRemovingSecurityId(holding.securityId);
    setMessage(null);
    // Remove from the rendered list immediately. The persisted list is read back below and is
    // authoritative; an error restores it instead of silently pretending deletion succeeded.
    setHoldings((current) => current.filter((item) => item.securityId !== holding.securityId));
    try {
      await removeFollowedSecurityCompletely(holding.holdingId, holding.securityId);
      const persisted = await loadPortfolioHoldings();
      if (persisted.some((item) => item.securityId === holding.securityId)) {
        throw new Error("取消关注未确认，请重新打开页面后重试");
      }
      setHoldings(persisted);
      setMessage(`已彻底删除：${holding.name}`);
    } catch (error) {
      try {
        setHoldings(await loadPortfolioHoldings());
      } catch {
        // Retain the optimistic list only when the local service is unavailable; the error below
        // makes that state explicit rather than showing a stale success state.
      }
      setMessage(error instanceof Error && error.message ? error.message : typeof error === "string" ? error : "关注标的删除失败");
    } finally {
      setRemovingSecurityId(null);
      setPendingRemoval(null);
    }
  }

  return (
    <section className="page portfolio-page" aria-labelledby="portfolio-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">Watchlist</p>
          <h1 id="portfolio-title">我的关注</h1>
          <p>以证券代码和名称记录当前关注标的；不展示账户资产、盈亏或交易指标。</p>
        </div>
        <button className="primary-button" onClick={() => { setIsFormOpen(true); setMessage(null); }} type="button">新增关注</button>
      </header>

      {message ? <p className="notice" role="status">{message}</p> : null}
      {isFormOpen ? <WatchlistForm isSaving={isSaving} onCancel={() => setIsFormOpen(false)} onSubmit={save} /> : null}
      {pendingRemoval ? <section aria-describedby="remove-follow-description" aria-labelledby="remove-follow-title" className="confirmation-dialog" role="dialog">
        <h2 id="remove-follow-title">确认彻底删除关注标的</h2>
        <p id="remove-follow-description">取消关注后，将同时删除该标的在 Hero Call 中保存的行情、资讯、事件及 AI 复盘数据，且无法恢复。是否继续？</p>
        <div className="form-actions"><button className="secondary-button" disabled={removingSecurityId !== null} onClick={() => setPendingRemoval(null)} type="button">取消</button><button className="danger-button" disabled={removingSecurityId !== null} onClick={() => void remove(pendingRemoval)} type="button">{removingSecurityId !== null ? "正在删除…" : "确认删除"}</button></div>
      </section> : null}

      <section className="portfolio-table-card" aria-label="关注标的列表">
        {isLoading ? <p className="table-state">正在读取本地关注标的…</p> : null}
        {!isLoading && holdings.length === 0 ? <p className="table-state">暂无关注标的</p> : null}
        {!isLoading && holdings.length > 0 ? (
          <div className="watchlist-list">
            {holdings.map((holding) => (
              <article className="watchlist-item" key={holding.holdingId}>
                <span className="code-cell">{holding.symbol}</span>
                <strong>{holding.name}</strong>
                <button className="watchlist-remove-button" disabled={removingSecurityId !== null} onClick={() => setPendingRemoval(holding)} type="button">取消关注</button>
              </article>
            ))}
          </div>
        ) : null}
      </section>
    </section>
  );
}
