import { useEffect, useState } from "react";

import {
  searchWatchlistSecurities,
  type CreateWatchlistInput,
  type SecurityLookup,
} from "../services/portfolio";

type WatchlistFormProps = {
  isSaving: boolean;
  onCancel: () => void;
  onSubmit: (input: CreateWatchlistInput) => void;
};

export function WatchlistForm({ isSaving, onCancel, onSubmit }: WatchlistFormProps) {
  const [symbol, setSymbol] = useState("");
  const [name, setName] = useState("");
  const [candidates, setCandidates] = useState<SecurityLookup[]>([]);
  const [selected, setSelected] = useState<SecurityLookup | null>(null);
  const [lookupMessage, setLookupMessage] = useState<string | null>(null);

  useEffect(() => {
    const normalizedSymbol = symbol.trim();
    const query = normalizedSymbol.length >= 2 ? normalizedSymbol : name.trim();
    if (!query) {
      setCandidates([]);
      setLookupMessage(null);
      return;
    }
    let active = true;
    void searchWatchlistSecurities(query).then((items) => {
      if (!active) return;
      setCandidates(items);
      const exactCode = normalizedSymbol.length === 6
        ? items.find((item) => item.symbol === normalizedSymbol)
        : undefined;
      if (exactCode) {
        setSelected(exactCode);
        setName(exactCode.name);
        setLookupMessage(null);
      } else if (normalizedSymbol.length === 6) {
        setLookupMessage("本地暂无可验证的证券基础信息，不能保存为关注标的。");
      } else {
        setLookupMessage(items.length === 0 ? "未找到本地已验证的匹配证券。" : "请选择一个证券以确认代码、名称和交易所。");
      }
    }).catch(() => {
      if (active) {
        setCandidates([]);
        setLookupMessage("证券基础信息暂不可用，请稍后重试。");
      }
    });
    return () => { active = false; };
  }, [name, symbol]);

  function selectCandidate(candidate: SecurityLookup) {
    setSelected(candidate);
    setSymbol(candidate.symbol);
    setName(candidate.name);
    setCandidates([]);
    setLookupMessage(null);
  }

  function changeSymbol(value: string) {
    setSelected(null);
    setSymbol(value.replace(/[^0-9]/g, ""));
    setName("");
  }

  function changeName(value: string) {
    setSelected(null);
    setName(value);
    setSymbol("");
  }

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (selected) onSubmit({ securityId: selected.securityId });
  }

  return (
    <form className="holding-form watchlist-form" onSubmit={submit}>
      <div className="form-heading">
        <div><p className="section-kicker">关注标的</p><h2>加入关注列表</h2></div>
        <button className="text-button" onClick={onCancel} type="button">关闭</button>
      </div>
      <p className="watchlist-form-intro">输入证券代码或名称，从本地已验证的证券基础信息中确认后加入关注；不记录数量、成本、价格或交易流水。</p>
      <div className="form-grid watchlist-form-grid">
        <label>证券代码<input inputMode="numeric" maxLength={6} onChange={(event) => changeSymbol(event.target.value)} placeholder="例如 300209" value={symbol} /></label>
        <label>证券名称<input onChange={(event) => changeName(event.target.value)} placeholder="例如 行云科技" value={name} /></label>
      </div>
      {selected ? <p className="watchlist-selection">已确认：{selected.symbol} {selected.name} · {selected.exchange}</p> : null}
      {!selected && candidates.length > 0 ? <div className="watchlist-candidates" role="listbox" aria-label="证券候选">
        {candidates.map((candidate) => <button key={candidate.securityId} onClick={() => selectCandidate(candidate)} role="option" type="button"><strong>{candidate.symbol} {candidate.name}</strong><span>{candidate.exchange}</span></button>)}
      </div> : null}
      {lookupMessage ? <p className="watchlist-lookup-message" role="status">{lookupMessage}</p> : null}
      <div className="form-actions">
        <button className="secondary-button" onClick={onCancel} type="button">取消</button>
        <button className="primary-button" disabled={isSaving || !selected} type="submit">{isSaving ? "正在加入…" : "加入关注列表"}</button>
      </div>
    </form>
  );
}
