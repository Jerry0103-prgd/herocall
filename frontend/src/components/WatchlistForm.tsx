import { useState } from "react";

import type { CreateWatchlistInput } from "../services/portfolio";

type WatchlistFormProps = {
  isSaving: boolean;
  onCancel: () => void;
  onSubmit: (input: CreateWatchlistInput) => void;
};

export function WatchlistForm({ isSaving, onCancel, onSubmit }: WatchlistFormProps) {
  const [symbol, setSymbol] = useState("");
  const [name, setName] = useState("");

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSubmit({ symbol, name });
  }

  return (
    <form className="holding-form watchlist-form" onSubmit={submit}>
      <div className="form-heading">
        <div><p className="section-kicker">关注标的</p><h2>加入关注列表</h2></div>
        <button className="text-button" onClick={onCancel} type="button">关闭</button>
      </div>
      <p className="watchlist-form-intro">仅保存证券代码和名称，不记录任何持仓数量、成本、价格或交易流水。</p>
      <div className="form-grid watchlist-form-grid">
        <label>证券代码<input inputMode="numeric" maxLength={6} onChange={(event) => setSymbol(event.target.value)} pattern="[0-9]{6}" placeholder="例如 300209" required value={symbol} /></label>
        <label>证券名称<input onChange={(event) => setName(event.target.value)} placeholder="例如 行云科技" required value={name} /></label>
      </div>
      <div className="form-actions">
        <button className="secondary-button" onClick={onCancel} type="button">取消</button>
        <button className="primary-button" disabled={isSaving} type="submit">{isSaving ? "正在加入…" : "加入关注列表"}</button>
      </div>
    </form>
  );
}
