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

export function loadSettingsStatus(): Promise<SettingsStatus> {
  return invoke<SettingsStatus>("get_settings_status");
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
