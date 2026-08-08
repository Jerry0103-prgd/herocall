import { useState, type FormEvent } from "react";

type CashAccountFormProps = {
  isSaving: boolean;
  onCancel: () => void;
  onSubmit: (input: { currency: "CNY"; amount: string }) => Promise<void>;
};

export function CashAccountForm({ isSaving, onCancel, onSubmit }: CashAccountFormProps) {
  const [amount, setAmount] = useState("");

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await onSubmit({ currency: "CNY", amount });
  }

  return (
    <form className="cash-account-form" onSubmit={(event) => void submit(event)}>
      <div className="form-heading">
        <div><p className="eyebrow">Cash account</p><h2>新增人民币现金账户</h2></div>
        <button className="text-button" onClick={onCancel} type="button">取消</button>
      </div>
      <div className="form-grid cash-form-grid">
        <label>币种<input aria-label="币种" readOnly value="CNY" /></label>
        <label>金额<input aria-label="金额" autoComplete="off" inputMode="decimal" onChange={(event) => setAmount(event.target.value)} placeholder="例如：100000.00" required value={amount} /></label>
      </div>
      <p className="form-note">金额以本地记账口径保存为可用于买入和可取现金；不连接券商账户。</p>
      <div className="form-actions"><button className="primary-button" disabled={isSaving} type="submit">{isSaving ? "正在保存…" : "保存现金账户"}</button></div>
    </form>
  );
}
