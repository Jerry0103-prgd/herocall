import { useCallback, useEffect, useState } from "react";

import { CashAccountForm } from "../components/CashAccountForm";
import {
  createCashAccount,
  createDatabaseBackup,
  loadCashAccounts,
  loadSettingsStatus,
  loadTushareStatus,
  removeTushareToken,
  saveTushareToken,
  type CashAccount,
  type SettingsStatus,
} from "../services/settings";

function valueOrUnavailable(value: string | null | undefined) {
  return value ?? "暂无数据";
}

export function SettingsPage() {
  const [status, setStatus] = useState<SettingsStatus | null>(null);
  const [cashAccounts, setCashAccounts] = useState<CashAccount[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isBackingUp, setIsBackingUp] = useState(false);
  const [isSavingToken, setIsSavingToken] = useState(false);
  const [isRemovingToken, setIsRemovingToken] = useState(false);
  const [isCashFormOpen, setIsCashFormOpen] = useState(false);
  const [tushareToken, setTushareToken] = useState("");
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    try {
      const [nextStatus, nextAccounts, tokenStatus] = await Promise.all([loadSettingsStatus(), loadCashAccounts(), loadTushareStatus()]);
      setStatus({ ...nextStatus, tushareStatus: tokenStatus.status });
      setCashAccounts(nextAccounts);
      setMessage(null);
    } catch {
      setMessage("本地设置服务暂不可用");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  async function saveCashAccount(input: { currency: "CNY"; amount: string }) {
    setIsSaving(true);
    try {
      await createCashAccount(input);
      setIsCashFormOpen(false);
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "现金账户保存失败");
    } finally {
      setIsSaving(false);
    }
  }

  async function backup() {
    setIsBackingUp(true);
    try {
      const result = await createDatabaseBackup();
      setMessage(`备份已创建：${result.fileName}`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "数据库备份失败");
    } finally {
      setIsBackingUp(false);
    }
  }

  async function saveToken() {
    if (!tushareToken.trim()) {
      setMessage("请输入 Tushare Token");
      return;
    }
    setIsSavingToken(true);
    try {
      const tokenStatus = await saveTushareToken(tushareToken);
      setTushareToken("");
      setStatus((current) => current ? { ...current, tushareStatus: tokenStatus.status } : current);
      setMessage("Tushare Token 已安全保存到系统钥匙串");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Tushare Token 保存失败");
    } finally {
      setIsSavingToken(false);
    }
  }

  async function removeToken() {
    setIsRemovingToken(true);
    try {
      const tokenStatus = await removeTushareToken();
      setTushareToken("");
      setStatus((current) => current ? { ...current, tushareStatus: tokenStatus.status } : current);
      setMessage("Tushare Token 已从系统钥匙串删除");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Tushare Token 删除失败");
    } finally {
      setIsRemovingToken(false);
    }
  }

  return (
    <section className="page settings-page" aria-labelledby="settings-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">System configuration</p>
          <h1 id="settings-title">设置</h1>
          <p>本地配置、现金账户与数据库备份；不保存券商权限或交易指令。</p>
        </div>
      </header>

      {message ? <p className="notice" role="status">{message}</p> : null}

      <section className="settings-section" aria-labelledby="market-config-title">
        <div className="section-heading"><div><p className="section-kicker">Market source</p><h2 id="market-config-title">行情数据源配置</h2></div></div>
        <div className="settings-card settings-config-card">
          <div><strong>Tushare</strong><p>Token 只保存到 macOS 系统钥匙串，不会写入 SQLite、源码、日志或再次回传到界面。</p></div>
          <span className={`settings-status ${status?.tushareStatus === "已配置" ? "is-configured" : ""}`}>{status?.tushareStatus ?? "未确认"}</span>
        </div>
        <form className="settings-card tushare-token-form" onSubmit={(event) => { event.preventDefault(); void saveToken(); }}>
          <label htmlFor="tushare-token">Tushare Token</label>
          <div className="tushare-token-actions">
            <input autoComplete="off" disabled={isSavingToken || isRemovingToken} id="tushare-token" onChange={(event) => setTushareToken(event.target.value)} placeholder="输入后仅保存到系统钥匙串" type="password" value={tushareToken} />
            <button className="primary-button" disabled={isSavingToken || isRemovingToken} type="submit">{isSavingToken ? "正在保存…" : "保存 Token"}</button>
            <button className="secondary-button" disabled={isSavingToken || isRemovingToken || status?.tushareStatus !== "已配置"} onClick={() => void removeToken()} type="button">{isRemovingToken ? "正在删除…" : "删除 Token"}</button>
          </div>
        </form>
      </section>

      <section className="settings-section" aria-labelledby="cash-title">
        <div className="section-heading"><div><p className="section-kicker">Cash</p><h2 id="cash-title">现金管理</h2></div><button className="primary-button" onClick={() => setIsCashFormOpen(true)} type="button">新增现金账户</button></div>
        {isCashFormOpen ? <CashAccountForm isSaving={isSaving} onCancel={() => setIsCashFormOpen(false)} onSubmit={saveCashAccount} /> : null}
        <div className="settings-card cash-accounts-card">
          {isLoading ? <p className="table-state">正在读取本地现金账户…</p> : null}
          {!isLoading && cashAccounts.length === 0 ? <p className="table-state">暂无现金账户</p> : null}
          {!isLoading && cashAccounts.length > 0 ? <div className="cash-account-list">{cashAccounts.map((account) => <div className="cash-account-row" key={account.id}><span>{account.name}</span><span>{account.currency}</span><strong>{account.amount}</strong></div>)}</div> : null}
        </div>
      </section>

      <section className="settings-section" aria-labelledby="system-status-title">
        <div className="section-heading"><div><p className="section-kicker">Health</p><h2 id="system-status-title">系统状态</h2></div></div>
        <div className="system-status-grid">
          <div className="settings-card"><span>数据库状态</span><strong>{status?.databaseStatus ?? "未确认"}</strong></div>
          <div className="settings-card"><span>行情连接状态</span><strong>{status?.marketConnectionStatus ?? "未确认"}</strong></div>
          <div className="settings-card"><span>最后同步时间</span><strong>{valueOrUnavailable(status?.lastSyncAt)}</strong></div>
        </div>
      </section>

      <section className="settings-section" aria-labelledby="backup-title">
        <div className="section-heading"><div><p className="section-kicker">Backup</p><h2 id="backup-title">本地备份</h2></div></div>
        <div className="settings-card backup-card"><div><strong>SQLite 数据库备份</strong><p>备份文件保存在 Documents/AStock-AI-Workbench/backups，不覆盖已有备份。</p></div><button className="secondary-button" disabled={isBackingUp} onClick={() => void backup()} type="button">{isBackingUp ? "正在备份…" : "立即备份"}</button></div>
      </section>
    </section>
  );
}
