import { useEffect, useState } from "react";

import type { CreateHoldingInput, PortfolioHolding, SecurityType } from "../services/portfolio";

type PortfolioHoldingFormProps = {
  holding: PortfolioHolding | null;
  isSaving: boolean;
  onCancel: () => void;
  onSubmit: (input: CreateHoldingInput | { name: string; quantity: string; averageCost: string }) => void;
};

const initialForm: CreateHoldingInput = {
  symbol: "",
  name: "",
  market: "SSE",
  securityType: "STOCK",
  quantity: "",
  averageCost: "",
};

export function PortfolioHoldingForm({ holding, isSaving, onCancel, onSubmit }: PortfolioHoldingFormProps) {
  const [form, setForm] = useState<CreateHoldingInput>(initialForm);

  useEffect(() => {
    setForm(holding ? {
      symbol: holding.symbol,
      name: holding.name,
      market: holding.market as CreateHoldingInput["market"],
      securityType: holding.securityType,
      quantity: holding.quantity,
      averageCost: holding.averageCost,
    } : initialForm);
  }, [holding]);

  const isEditing = holding !== null;

  function updateField<K extends keyof CreateHoldingInput>(key: K, value: CreateHoldingInput[K]) {
    setForm((current) => ({ ...current, [key]: value }));
  }

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (isEditing) {
      onSubmit({ name: form.name, quantity: form.quantity, averageCost: form.averageCost });
      return;
    }
    onSubmit(form);
  }

  return (
    <form className="holding-form" onSubmit={submit}>
      <div className="form-heading">
        <div><p className="section-kicker">本地持仓账本</p><h2>{isEditing ? "修改持仓" : "新增持仓"}</h2></div>
        <button className="text-button" onClick={onCancel} type="button">关闭</button>
      </div>

      <div className="form-grid">
        <label>证券代码<input disabled={isEditing} maxLength={6} onChange={(event) => updateField("symbol", event.target.value)} required value={form.symbol} /></label>
        <label>证券名称<input onChange={(event) => updateField("name", event.target.value)} required value={form.name} /></label>
        <label>市场<select disabled={isEditing} onChange={(event) => updateField("market", event.target.value as CreateHoldingInput["market"])} value={form.market}><option value="SSE">SSE（上交所）</option><option value="SZSE">SZSE（深交所）</option></select></label>
        <label>证券类型<select disabled={isEditing} onChange={(event) => updateField("securityType", event.target.value as SecurityType)} value={form.securityType}><option value="STOCK">股票</option><option value="ETF">ETF</option></select></label>
        <label>持仓数量<input inputMode="numeric" onChange={(event) => updateField("quantity", event.target.value)} required value={form.quantity} /></label>
        <label>成本价<input inputMode="decimal" onChange={(event) => updateField("averageCost", event.target.value)} required value={form.averageCost} /></label>
      </div>
      <p className="form-note">成本金额、可卖数量及行情相关指标均由本地 Rust 服务计算；未确认行情不会补齐价格。</p>
      <div className="form-actions">
        <button className="secondary-button" onClick={onCancel} type="button">取消</button>
        <button className="primary-button" disabled={isSaving} type="submit">{isSaving ? "保存中…" : "保存持仓"}</button>
      </div>
    </form>
  );
}
