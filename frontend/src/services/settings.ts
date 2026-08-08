import { invoke } from "@tauri-apps/api/core";

export type SettingsStatus = {
  tushareStatus: "已配置" | "未配置";
  databaseStatus: string;
  marketConnectionStatus: string;
  lastSyncAt: string | null;
};

export type CashAccount = {
  id: number;
  name: string;
  currency: "CNY";
  amount: string;
};

export type CreateCashAccountInput = {
  currency: "CNY";
  amount: string;
};

export type BackupResult = {
  fileName: string;
};

export type TushareStatus = {
  status: "已配置" | "未配置";
};

export function loadSettingsStatus(): Promise<SettingsStatus> {
  return invoke<SettingsStatus>("get_settings_status");
}

// These commands intentionally return only a configuration state. The token is transient input
// and never read back into the frontend after it is stored in the system credential manager.
export function loadTushareStatus(): Promise<TushareStatus> {
  return invoke<TushareStatus>("get_tushare_status");
}

export function saveTushareToken(token: string): Promise<TushareStatus> {
  return invoke<TushareStatus>("save_tushare_token", { token });
}

export function removeTushareToken(): Promise<TushareStatus> {
  return invoke<TushareStatus>("remove_tushare_token");
}

export function loadCashAccounts(): Promise<CashAccount[]> {
  return invoke<CashAccount[]>("get_cash_accounts");
}

export function createCashAccount(input: CreateCashAccountInput): Promise<CashAccount> {
  return invoke<CashAccount>("create_cash_account", { input });
}

export function createDatabaseBackup(): Promise<BackupResult> {
  return invoke<BackupResult>("create_database_backup");
}
