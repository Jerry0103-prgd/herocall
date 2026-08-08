import { useEffect, useState } from "react";

import { createPortfolioHolding, type CreateHoldingInput } from "../services/portfolio";
import { completeInitialization } from "../services/initialization";
import { createCashAccount, loadSettingsStatus, type SettingsStatus } from "../services/settings";

type FirstRunWizardProps = {
  onCompleted: () => void;
};

const steps = ["设置人民币现金", "添加初始持仓", "数据源状态", "完成初始化"];

const initialHolding: CreateHoldingInput = {
  symbol: "",
  name: "",
  market: "SSE",
  securityType: "STOCK",
  quantity: "",
  averageCost: "",
};

export function FirstRunWizard({ onCompleted }: FirstRunWizardProps) {
  const [step, setStep] = useState(0);
  const [cashAmount, setCashAmount] = useState("");
  const [holding, setHolding] = useState<CreateHoldingInput>(initialHolding);
  const [settingsStatus, setSettingsStatus] = useState<SettingsStatus | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (step !== 2) return;
    void loadSettingsStatus()
      .then(setSettingsStatus)
      .catch(() => setMessage("数据源状态暂不可用，请在设置页稍后确认。"));
  }, [step]);

  function nextStep() {
    setMessage(null);
    setStep((current) => Math.min(current + 1, steps.length - 1));
  }

  async function saveCashAndContinue() {
    if (!cashAmount.trim()) {
      nextStep();
      return;
    }
    setIsSaving(true);
    try {
      await createCashAccount({ currency: "CNY", amount: cashAmount });
      nextStep();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "现金账户保存失败");
    } finally {
      setIsSaving(false);
    }
  }

  async function saveHoldingAndContinue() {
    const fields = [holding.symbol, holding.name, holding.quantity, holding.averageCost];
    if (fields.every((value) => !value.trim())) {
      nextStep();
      return;
    }
    if (fields.some((value) => !value.trim())) {
      setMessage("请补全初始持仓信息，或跳过此步后再在“我的持仓”中维护。");
      return;
    }
    setIsSaving(true);
    try {
      await createPortfolioHolding(holding);
      nextStep();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "初始持仓保存失败");
    } finally {
      setIsSaving(false);
    }
  }

  async function finish() {
    setIsSaving(true);
    try {
      await completeInitialization();
      onCompleted();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "初始化状态保存失败");
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="first-run-overlay" role="dialog" aria-modal="true" aria-labelledby="first-run-title">
      <section className="first-run-wizard">
        <p className="eyebrow">First-run setup</p>
        <h1 id="first-run-title">欢迎使用 AStock AI Workbench</h1>
        <p className="first-run-intro">可选设置均可跳过，稍后可在“设置”或“我的持仓”中继续维护。本应用不连接券商，也不提供下单能力。</p>

        <ol className="first-run-progress" aria-label="初始化步骤">
          {steps.map((label, index) => <li className={index === step ? "is-current" : index < step ? "is-complete" : ""} key={label}>{label}</li>)}
        </ol>

        {message ? <p className="notice" role="status">{message}</p> : null}

        {step === 0 ? <section className="first-run-step"><h2>设置人民币现金</h2><p>可选。保存为本地 CNY 现金账户，不涉及券商余额或交易权限。</p><label>金额（CNY）<input inputMode="decimal" onChange={(event) => setCashAmount(event.target.value)} placeholder="例如 10000.00" value={cashAmount} /></label><div className="form-actions"><button className="secondary-button" disabled={isSaving} onClick={nextStep} type="button">跳过此步</button><button className="primary-button" disabled={isSaving} onClick={() => void saveCashAndContinue()} type="button">{isSaving ? "正在保存…" : "保存并继续"}</button></div></section> : null}

        {step === 1 ? <section className="first-run-step"><h2>添加初始持仓</h2><p>可选。仅保存用户录入的股票或 ETF 持仓；不会请求或生成行情数据。</p><div className="form-grid"><label>证券代码<input onChange={(event) => setHolding({ ...holding, symbol: event.target.value })} placeholder="例如 600519" value={holding.symbol} /></label><label>证券名称<input onChange={(event) => setHolding({ ...holding, name: event.target.value })} placeholder="例如 贵州茅台" value={holding.name} /></label><label>市场<select onChange={(event) => setHolding({ ...holding, market: event.target.value as CreateHoldingInput["market"] })} value={holding.market}><option value="SSE">SSE</option><option value="SZSE">SZSE</option></select></label><label>证券类型<select onChange={(event) => setHolding({ ...holding, securityType: event.target.value as CreateHoldingInput["securityType"] })} value={holding.securityType}><option value="STOCK">股票</option><option value="ETF">ETF</option></select></label><label>持仓数量<input inputMode="numeric" onChange={(event) => setHolding({ ...holding, quantity: event.target.value })} placeholder="例如 100" value={holding.quantity} /></label><label>成本价<input inputMode="decimal" onChange={(event) => setHolding({ ...holding, averageCost: event.target.value })} placeholder="例如 1500.00" value={holding.averageCost} /></label></div><div className="form-actions"><button className="secondary-button" disabled={isSaving} onClick={nextStep} type="button">跳过此步</button><button className="primary-button" disabled={isSaving} onClick={() => void saveHoldingAndContinue()} type="button">{isSaving ? "正在保存…" : "保存并继续"}</button></div></section> : null}

        {step === 2 ? <section className="first-run-step"><h2>配置数据源状态</h2><p>此步骤只展示受保护运行时配置状态。不会显示、保存或写入任何 Token。</p><div className="first-run-source-status"><span>Tushare</span><strong className={settingsStatus?.tushareStatus === "已配置" ? "is-configured" : ""}>{settingsStatus?.tushareStatus ?? "未确认"}</strong></div><div className="form-actions"><button className="secondary-button" onClick={nextStep} type="button">跳过此步</button><button className="primary-button" onClick={nextStep} type="button">继续</button></div></section> : null}

        {step === 3 ? <section className="first-run-step"><h2>完成初始化</h2><p>完成后不会再次自动展示此向导。你仍可随时在设置页查看数据源状态、管理现金账户，并在我的持仓中维护持仓。</p><div className="form-actions"><button className="primary-button" disabled={isSaving} onClick={() => void finish()} type="button">{isSaving ? "正在完成…" : "完成初始化"}</button></div></section> : null}
      </section>
    </div>
  );
}
