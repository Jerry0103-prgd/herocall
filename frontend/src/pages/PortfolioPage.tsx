import { useCallback, useEffect, useState } from "react";

import { PortfolioHoldingForm } from "../components/PortfolioHoldingForm";
import {
  createPortfolioHolding,
  deletePortfolioHolding,
  loadPortfolioHoldings,
  updatePortfolioHolding,
  type CreateHoldingInput,
  type PortfolioHolding,
} from "../services/portfolio";

function valueOrUnavailable(value: string | null) {
  return value ?? "暂无数据";
}

export function PortfolioPage() {
  const [holdings, setHoldings] = useState<PortfolioHolding[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [editingHolding, setEditingHolding] = useState<PortfolioHolding | null>(null);
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

  function openCreate() {
    setEditingHolding(null);
    setIsFormOpen(true);
    setMessage(null);
  }

  function openEdit(holding: PortfolioHolding) {
    setEditingHolding(holding);
    setIsFormOpen(true);
    setMessage(null);
  }

  async function save(input: CreateHoldingInput | { name: string; quantity: string; averageCost: string }) {
    setIsSaving(true);
    try {
      if (editingHolding) {
        await updatePortfolioHolding({ holdingId: editingHolding.holdingId, ...input });
      } else {
        await createPortfolioHolding(input as CreateHoldingInput);
      }
      setIsFormOpen(false);
      setEditingHolding(null);
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "持仓保存失败");
    } finally {
      setIsSaving(false);
    }
  }

  async function remove(holdingId: number) {
    try {
      await deletePortfolioHolding(holdingId);
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "持仓删除失败");
    }
  }

  return (
    <section className="page portfolio-page" aria-labelledby="portfolio-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">Portfolio ledger</p>
          <h1 id="portfolio-title">我的持仓</h1>
          <p>本地维护 A 股股票与 ETF 持仓；不连接券商，也不提供交易能力。</p>
        </div>
        <button className="primary-button" onClick={openCreate} type="button">新增持仓</button>
      </header>

      {message ? <p className="notice" role="status">{message}</p> : null}
      {isFormOpen ? <PortfolioHoldingForm holding={editingHolding} isSaving={isSaving} onCancel={() => setIsFormOpen(false)} onSubmit={save} /> : null}

      <section className="portfolio-table-card" aria-label="持仓列表">
        {isLoading ? <p className="table-state">正在读取本地持仓…</p> : null}
        {!isLoading && holdings.length === 0 ? <p className="table-state">暂无持仓</p> : null}
        {!isLoading && holdings.length > 0 ? (
          <div className="table-scroll">
            <table>
              <thead><tr><th>证券名称</th><th>证券代码</th><th>市场</th><th>证券类型</th><th>持仓数量</th><th>可卖数量</th><th>成本价</th><th>当前价格</th><th>持仓市值</th><th>今日盈亏</th><th>总盈亏</th><th>涨跌幅</th><th>交易状态</th><th>操作</th></tr></thead>
              <tbody>
                {holdings.map((holding) => (
                  <tr key={holding.holdingId}>
                    <td>{holding.name}</td><td className="code-cell">{holding.symbol}</td><td>{holding.market}</td><td>{holding.securityType}</td>
                    <td>{holding.quantity}</td><td>{holding.availableQuantity ?? "暂无数据"}</td><td>{holding.averageCost}</td>
                    <td>{valueOrUnavailable(holding.currentPrice)}</td><td>{valueOrUnavailable(holding.marketValue)}</td><td>{valueOrUnavailable(holding.dailyPnl)}</td><td>{valueOrUnavailable(holding.totalPnl)}</td><td>{valueOrUnavailable(holding.changePercent)}</td><td>{holding.transactionStatus}</td>
                    <td><div className="row-actions"><button onClick={() => openEdit(holding)} type="button">修改</button><button className="danger-button" onClick={() => void remove(holding.holdingId)} type="button">删除</button></div></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : null}
      </section>
    </section>
  );
}
