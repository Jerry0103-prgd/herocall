import { useEffect, useState } from "react";

import { completeInitialization } from "../services/initialization";
import { loadSettingsStatus, type SettingsStatus } from "../services/settings";

type FirstRunWizardProps = {
  onCompleted: () => void;
};

const steps = ["数据源状态", "完成初始化"];

export function FirstRunWizard({ onCompleted }: FirstRunWizardProps) {
  const [step, setStep] = useState(0);
  const [settingsStatus, setSettingsStatus] = useState<SettingsStatus | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (step !== 0) return;
    void loadSettingsStatus()
      .then(setSettingsStatus)
      .catch(() => setMessage("数据源状态暂不可用，请在设置页稍后确认。"));
  }, [step]);

  function nextStep() {
    setMessage(null);
    setStep((current) => Math.min(current + 1, steps.length - 1));
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
        <h1 id="first-run-title">欢迎使用 Hero Call</h1>
        <p className="first-run-intro">此向导仅确认数据源状态。Hero Call 用于研究、阅读与复盘，不连接券商，也不提供下单能力。</p>

        <ol className="first-run-progress" aria-label="初始化步骤">
          {steps.map((label, index) => <li className={index === step ? "is-current" : index < step ? "is-complete" : ""} key={label}>{label}</li>)}
        </ol>

        {message ? <p className="notice" role="status">{message}</p> : null}

        {step === 0 ? <section className="first-run-step"><h2>数据源状态</h2><p>此步骤只展示系统凭据库中的安全配置状态。不会显示、保存到 SQLite 或写入任何 Token。</p><div className="first-run-source-status"><span>Tushare</span><strong className={settingsStatus?.tushareStatus === "已配置" ? "is-configured" : ""}>{settingsStatus?.tushareStatus ?? "未确认"}</strong></div><div className="form-actions"><button className="primary-button" onClick={nextStep} type="button">继续</button></div></section> : null}

        {step === 1 ? <section className="first-run-step"><h2>完成初始化</h2><p>完成后不会再次自动展示此向导。你仍可随时在设置页查看数据源状态，在关注标的页面查看已保存的标的。</p><div className="form-actions"><button className="primary-button" disabled={isSaving} onClick={() => void finish()} type="button">{isSaving ? "正在完成…" : "完成初始化"}</button></div></section> : null}
      </section>
    </div>
  );
}
