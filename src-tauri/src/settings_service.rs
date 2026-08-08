//! Application service for local system configuration.
//!
//! Secrets are intentionally outside the SQLite model: Tushare is configured through the system
//! credential store and this module returns a safe status, never the token.

use std::{error::Error, fmt, str::FromStr};

use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::database::service::{CashAccount, DatabaseError, DatabaseService, NewCashAccount};
use crate::secure_storage::{get_tushare_status, SecureStorageError};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsStatusView {
    pub tushare_status: String,
    pub database_status: String,
    pub market_connection_status: String,
    pub last_sync_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CashAccountView {
    pub id: i64,
    pub name: String,
    pub currency: String,
    pub amount: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCashAccountInput {
    pub currency: String,
    pub amount: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupView {
    pub file_name: String,
}

#[derive(Debug)]
pub enum SettingsError {
    Database(DatabaseError),
    SecureStorage(SecureStorageError),
    Validation(&'static str),
    AppPath(String),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::SecureStorage(error) => write!(formatter, "secure storage error: {error}"),
            Self::Validation(message) => formatter.write_str(message),
            Self::AppPath(message) => write!(formatter, "application path error: {message}"),
        }
    }
}

impl Error for SettingsError {}

impl From<DatabaseError> for SettingsError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<SecureStorageError> for SettingsError {
    fn from(error: SecureStorageError) -> Self {
        Self::SecureStorage(error)
    }
}

pub struct SettingsService;

impl SettingsService {
    /// Reports only whether protected credential configuration exists. The token must not be
    /// persisted, logged, returned through a command, or rendered by the frontend.
    pub fn load_status(database: &DatabaseService) -> Result<SettingsStatusView, SettingsError> {
        let tushare_status = get_tushare_status()?.status;
        Self::load_status_with_tushare_status(database, tushare_status)
    }

    fn load_status_with_tushare_status(
        database: &DatabaseService,
        tushare_status: String,
    ) -> Result<SettingsStatusView, SettingsError> {
        let latest_source = database.latest_market_source_status()?;
        let (market_connection_status, last_sync_at) = match latest_source {
            Some(source) if source.status == "ACTIVE" => ("已连接".into(), source.last_success_at),
            Some(source) if source.status == "NO_DATA" => ("暂无数据".into(), None),
            Some(_) => ("未确认".into(), None),
            None => ("未确认".into(), None),
        };

        Ok(SettingsStatusView {
            tushare_status,
            database_status: "正常".into(),
            market_connection_status,
            last_sync_at,
        })
    }

    pub fn list_cash_accounts(
        database: &DatabaseService,
    ) -> Result<Vec<CashAccountView>, SettingsError> {
        Ok(database
            .list_cash_accounts()?
            .into_iter()
            .map(Self::cash_account_view)
            .collect())
    }

    pub fn create_cny_cash_account(
        database: &DatabaseService,
        input: CreateCashAccountInput,
    ) -> Result<CashAccountView, SettingsError> {
        if input.currency != "CNY" {
            return Err(SettingsError::Validation("V1.0 仅支持人民币现金账户"));
        }
        let amount = Decimal::from_str(input.amount.trim())
            .map_err(|_| SettingsError::Validation("现金金额格式无效"))?;
        if amount.is_sign_negative() {
            return Err(SettingsError::Validation("现金金额不能小于 0"));
        }
        let amount = amount.to_string();
        let account = database.create_cash_account(NewCashAccount {
            name: database.next_cny_cash_account_name()?,
            currency: "CNY".into(),
            available_to_buy: amount.clone(),
            withdrawable_cash: amount,
            pending_settlement: "0".into(),
        })?;
        Ok(Self::cash_account_view(account))
    }

    pub fn create_backup(
        app: &tauri::AppHandle,
        database: &DatabaseService,
    ) -> Result<BackupView, SettingsError> {
        let documents_dir = app
            .path()
            .document_dir()
            .map_err(|error| SettingsError::AppPath(error.to_string()))?;
        let backup_directory = documents_dir.join("AStock-AI-Workbench").join("backups");
        let file_name = format!(
            "astock-ai-workbench-{}.sqlite3",
            Utc::now().format("%Y%m%dT%H%M%S%.3fZ")
        );
        let destination = backup_directory.join(&file_name);
        database.backup_to(&destination)?;
        Ok(BackupView { file_name })
    }

    fn cash_account_view(account: CashAccount) -> CashAccountView {
        CashAccountView {
            id: account.id,
            name: account.name,
            currency: account.currency,
            amount: account.available_to_buy,
        }
    }

    #[cfg(test)]
    fn backup_path_for_test() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "astock-ai-workbench-settings-{}-{}.sqlite3",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_reports_only_safe_configuration_state_and_saves_cny_cash() {
        let database = DatabaseService::open_in_memory().expect("create database");
        let configured =
            SettingsService::load_status_with_tushare_status(&database, "已配置".into())
                .expect("load configured status");
        assert_eq!(configured.tushare_status, "已配置");
        assert_eq!(configured.market_connection_status, "未确认");

        let account = SettingsService::create_cny_cash_account(
            &database,
            CreateCashAccountInput {
                currency: "CNY".into(),
                amount: "1200.50".into(),
            },
        )
        .expect("create cash account");
        assert_eq!(account.currency, "CNY");
        assert_eq!(account.amount, "1200.50");

        let accounts = SettingsService::list_cash_accounts(&database).expect("list accounts");
        assert_eq!(accounts, vec![account]);
    }

    #[test]
    fn sqlite_backup_is_created_without_overwriting_the_source() {
        let database = DatabaseService::open_in_memory().expect("create database");
        let destination = SettingsService::backup_path_for_test();
        database.backup_to(&destination).expect("create backup");
        assert!(destination.is_file());
        let copied_database = DatabaseService::open(&destination).expect("open backup");
        assert!(SettingsService::list_cash_accounts(&copied_database)
            .expect("read backup")
            .is_empty());
        std::fs::remove_file(destination).expect("remove test backup");
    }
}
