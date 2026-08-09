import { useCallback, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

import { AboutHeroCall } from "../components/AboutHeroCall";
import {
  loadAiProviderConfigs,
  removeAiProviderApiKey,
  saveAiProviderApiKey,
  setAiProviderEnabled,
  type AiProviderConfig,
} from "../services/ai";
import {
  createDatabaseBackup,
  loadSettingsStatus,
  loadTushareStatus,
  removeTushareToken,
  saveTushareToken,
  type SettingsStatus,
} from "../services/settings";

function formatBeijingTime(value: string | null | undefined) {
  if (!value) return "暂无数据";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "未确认";
  const parts = new Intl.DateTimeFormat("en-CA", { timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).formatToParts(date);
  const part = (type: Intl.DateTimeFormatPartTypes) => parts.find((item) => item.type === type)?.value ?? "";
  return `${part("year")}-${part("month")}-${part("day")} ${part("hour")}:${part("minute")}`;
}

export function SettingsPage() {
  const [status, setStatus] = useState<SettingsStatus | null>(null);
  const [isBackingUp, setIsBackingUp] = useState(false);
  const [isSavingToken, setIsSavingToken] = useState(false);
  const [isRemovingToken, setIsRemovingToken] = useState(false);
  const [tushareToken, setTushareToken] = useState("");
  const [aiProviders, setAiProviders] = useState<AiProviderConfig[]>([]);
  const [providerKeys, setProviderKeys] = useState<Record<string, string>>({});
  const [activeProvider, setActiveProvider] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [nextStatus, tokenStatus, providers] = await Promise.all([loadSettingsStatus(), loadTushareStatus(), loadAiProviderConfigs()]);
      setStatus({ ...nextStatus, tushareStatus: tokenStatus.status });
      setAiProviders(providers);
      setMessage(null);
    } catch {
      setMessage("本地设置服务暂不可用");
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  useEffect(() => {
    let isMounted = true;
    void getVersion().then(
      (version) => { if (isMounted) setAppVersion(version); },
      () => { if (isMounted) setAppVersion(null); },
    );
    return () => { isMounted = false; };
  }, []);

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

  async function saveProvider(provider: AiProviderConfig) {
    const key = providerKeys[provider.provider]?.trim();
    if (!key) { setMessage(`请输入 ${provider.displayName} API Key`); return; }
    setActiveProvider(provider.provider);
    try {
      await saveAiProviderApiKey(provider.provider, key);
      setProviderKeys((current) => ({ ...current, [provider.provider]: "" }));
      setAiProviders(await loadAiProviderConfigs());
      setMessage(`${provider.displayName} API Key 已安全保存到系统钥匙串`);
    } catch (error) { setMessage(error instanceof Error ? error.message : `${provider.displayName} API Key 保存失败`); }
    finally { setActiveProvider(null); }
  }

  async function removeProvider(provider: AiProviderConfig) {
    setActiveProvider(provider.provider);
    try {
      await removeAiProviderApiKey(provider.provider);
      setAiProviders(await setAiProviderEnabled(provider.provider, false));
      setMessage(`${provider.displayName} API Key 已从系统钥匙串删除并关闭`);
    } catch (error) { setMessage(error instanceof Error ? error.message : `${provider.displayName} API Key 删除失败`); }
    finally { setActiveProvider(null); }
  }

  async function toggleProvider(provider: AiProviderConfig) {
    setActiveProvider(provider.provider);
    try {
      setAiProviders(await setAiProviderEnabled(provider.provider, !provider.enabled));
      setMessage(`${provider.displayName} 已${provider.enabled ? "关闭" : "开启"}`);
    } catch (error) { setMessage(error instanceof Error ? error.message : "AI Provider 状态更新失败"); }
    finally { setActiveProvider(null); }
  }

  return (
    <section className="page settings-page" aria-labelledby="settings-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">System configuration</p>
          <h1 id="settings-title">设置</h1>
          <p>本地配置、数据源状态与数据库备份；不保存券商权限或交易指令。</p>
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

      <section className="settings-section" aria-labelledby="ai-config-title">
        <div className="section-heading"><div><p className="section-kicker">AI provider</p><h2 id="ai-config-title">AI Provider 配置</h2></div><span>仅调用一个已开启 Provider，按优先级选择。</span></div>
        <div className="ai-provider-list">{aiProviders.map((provider) => <article className="settings-card ai-provider-card" key={provider.provider}>
          <div className="ai-provider-header"><div><strong>{provider.displayName}</strong><p>模型：{provider.model}</p></div><div className="ai-provider-statuses"><span className={`settings-status ${provider.configured ? "is-configured" : ""}`}>{provider.configured ? "已配置" : "未配置"}</span><button className="secondary-button" disabled={activeProvider === provider.provider || (!provider.enabled && !provider.configured)} onClick={() => void toggleProvider(provider)} type="button">{provider.enabled ? "关闭" : "开启"}</button></div></div>
          <form className="tushare-token-form ai-provider-key-form" onSubmit={(event) => { event.preventDefault(); void saveProvider(provider); }}><label htmlFor={`${provider.provider}-api-key`}>{provider.displayName} API Key</label><div className="tushare-token-actions"><input autoComplete="off" disabled={activeProvider === provider.provider} id={`${provider.provider}-api-key`} onChange={(event) => setProviderKeys((current) => ({ ...current, [provider.provider]: event.target.value }))} placeholder="输入后仅保存到系统钥匙串" type="password" value={providerKeys[provider.provider] ?? ""} /><button className="primary-button" disabled={activeProvider === provider.provider} type="submit">{activeProvider === provider.provider ? "处理中…" : "保存 Key"}</button><button className="secondary-button" disabled={activeProvider === provider.provider || !provider.configured} onClick={() => void removeProvider(provider)} type="button">删除 Key</button></div></form>
        </article>)}</div>
      </section>

      <section className="settings-section" aria-labelledby="system-status-title">
        <div className="section-heading"><div><p className="section-kicker">Health</p><h2 id="system-status-title">系统状态</h2></div></div>
        <div className="system-status-grid">
          <div className="settings-card"><span>数据库状态</span><strong>{status?.databaseStatus ?? "未确认"}</strong></div>
          <div className="settings-card"><span>行情连接状态</span><strong>{status?.marketConnectionStatus ?? "未确认"}</strong></div>
          <div className="settings-card system-sync-card"><span>最后同步时间</span><strong>{formatBeijingTime(status?.lastSyncAt)}</strong></div>
        </div>
      </section>

      <section className="settings-section" aria-labelledby="backup-title">
        <div className="section-heading"><div><p className="section-kicker">Backup</p><h2 id="backup-title">本地备份</h2></div></div>
        <div className="settings-card backup-card"><div><strong>SQLite 数据库备份</strong><p>备份文件保存在本机 Documents 下的应用备份目录，不覆盖已有备份。</p></div><button className="secondary-button" disabled={isBackingUp} onClick={() => void backup()} type="button">{isBackingUp ? "正在备份…" : "立即备份"}</button></div>
      </section>

      <AboutHeroCall version={appVersion} />
    </section>
  );
}
