#![allow(dead_code)] // Phase 2-A defines the service API before later application commands consume it.

use std::{error::Error, fmt, fs, path::Path};

use rusqlite::{params, Connection, Row};
use tauri::Manager;

use super::migrations;
use crate::market_service::{MarketSnapshot, MarketSnapshotStore, MarketStatus, MarketStoreError};

pub type DatabaseResult<T> = Result<T, DatabaseError>;

#[derive(Debug)]
pub enum DatabaseError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    AppPath(String),
    MigrationChecksum { version: &'static str },
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "SQLite error: {error}"),
            Self::Io(error) => write!(formatter, "filesystem error: {error}"),
            Self::AppPath(error) => write!(formatter, "application data path error: {error}"),
            Self::MigrationChecksum { version } => {
                write!(
                    formatter,
                    "migration checksum mismatch for version {version}"
                )
            }
        }
    }
}

impl Error for DatabaseError {}

impl From<rusqlite::Error> for DatabaseError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<std::io::Error> for DatabaseError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Security {
    pub id: i64,
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: String,
    pub security_type: String,
    pub industry: Option<String>,
    pub concepts_json: String,
    pub trade_rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSecurity {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: String,
    pub security_type: String,
    pub industry: Option<String>,
    pub concepts_json: String,
    pub trade_rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashAccount {
    pub id: i64,
    pub name: String,
    pub currency: String,
    pub available_to_buy: String,
    pub withdrawable_cash: String,
    pub pending_settlement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCashAccount {
    pub name: String,
    pub currency: String,
    pub available_to_buy: String,
    pub withdrawable_cash: String,
    pub pending_settlement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holding {
    pub id: i64,
    pub cash_account_id: i64,
    pub security_id: i64,
    pub quantity: i64,
    pub available_quantity: i64,
    pub average_cost: String,
    pub cost_amount: String,
    pub position_source: String,
    pub as_of_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewHolding {
    pub cash_account_id: i64,
    pub security_id: i64,
    pub quantity: i64,
    pub available_quantity: i64,
    pub average_cost: String,
    pub cost_amount: String,
    pub position_source: String,
    pub as_of_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingUpdate {
    pub quantity: i64,
    pub available_quantity: i64,
    pub average_cost: String,
    pub cost_amount: String,
    pub as_of_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub id: i64,
    pub cash_account_id: i64,
    pub security_id: i64,
    pub side: String,
    pub status: String,
    pub record_source: String,
    pub trade_date: String,
    pub quantity: i64,
    pub price: String,
    pub commission: String,
    pub stamp_tax: String,
    pub transfer_fee: String,
    pub other_fee: String,
    pub minimum_commission: String,
    pub external_reference: Option<String>,
    pub import_batch_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTransaction {
    pub cash_account_id: i64,
    pub security_id: i64,
    pub side: String,
    pub status: String,
    pub record_source: String,
    pub trade_date: String,
    pub quantity: i64,
    pub price: String,
    pub commission: String,
    pub stamp_tax: String,
    pub transfer_fee: String,
    pub other_fee: String,
    pub minimum_commission: String,
    pub external_reference: Option<String>,
    pub import_batch_id: Option<String>,
    pub note: Option<String>,
}

/// SQLite access boundary for the Rust application layer. The UI has no direct database access.
pub struct DatabaseService {
    connection: Connection,
}

impl DatabaseService {
    pub fn open(path: impl AsRef<Path>) -> DatabaseResult<Self> {
        let mut connection = Connection::open(path)?;
        migrations::apply(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn open_in_memory() -> DatabaseResult<Self> {
        let mut connection = Connection::open_in_memory()?;
        migrations::apply(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn initialize_app_database(app: &tauri::AppHandle) -> DatabaseResult<()> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| DatabaseError::AppPath(error.to_string()))?;
        fs::create_dir_all(&app_data_dir)?;
        Self::open(app_data_dir.join("astock-ai-workbench.sqlite3"))?;
        Ok(())
    }

    pub fn create_security(&self, input: NewSecurity) -> DatabaseResult<Security> {
        self.connection.execute(
            "
            INSERT INTO securities (
                symbol, name, market, exchange, instrument_type, security_type,
                industry, concepts_json, trading_rule, trade_rule
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                input.symbol,
                input.name,
                input.market,
                input.exchange,
                input.security_type,
                input.security_type,
                input.industry,
                input.concepts_json,
                input.trade_rule,
                input.trade_rule,
            ],
        )?;
        self.get_security(self.connection.last_insert_rowid())
    }

    pub fn create_cash_account(&self, input: NewCashAccount) -> DatabaseResult<CashAccount> {
        self.connection.execute(
            "
            INSERT INTO cash_accounts (name, currency, available_to_buy, withdrawable_cash, pending_settlement)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                input.name,
                input.currency,
                input.available_to_buy,
                input.withdrawable_cash,
                input.pending_settlement,
            ],
        )?;
        self.get_cash_account(self.connection.last_insert_rowid())
    }

    pub fn create_holding(&self, input: NewHolding) -> DatabaseResult<Holding> {
        self.connection.execute(
            "
            INSERT INTO holdings (
                cash_account_id, security_id, quantity, available_quantity, average_cost,
                cost_amount, position_source, as_of_date
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                input.cash_account_id,
                input.security_id,
                input.quantity,
                input.available_quantity,
                input.average_cost,
                input.cost_amount,
                input.position_source,
                input.as_of_date,
            ],
        )?;
        self.get_holding(self.connection.last_insert_rowid())
    }

    /// Persists the user-entered ledger record only. It intentionally performs no cost,
    /// P&L, cash balance, or T+1 calculation in Phase 2-A.
    pub fn create_transaction(&self, input: NewTransaction) -> DatabaseResult<Transaction> {
        self.connection.execute(
            "
            INSERT INTO transactions (
                cash_account_id, security_id, side, status, record_source, trade_date, quantity, price,
                commission, stamp_duty, stamp_tax, transfer_fee, other_fee, minimum_commission,
                external_reference, import_batch_id, note
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ",
            params![
                input.cash_account_id,
                input.security_id,
                input.side,
                input.status,
                input.record_source,
                input.trade_date,
                input.quantity,
                input.price,
                input.commission,
                input.stamp_tax,
                input.stamp_tax,
                input.transfer_fee,
                input.other_fee,
                input.minimum_commission,
                input.external_reference,
                input.import_batch_id,
                input.note,
            ],
        )?;
        self.get_transaction(self.connection.last_insert_rowid())
    }

    pub fn get_holding(&self, id: i64) -> DatabaseResult<Holding> {
        self.connection.query_row(
            "
            SELECT id, cash_account_id, security_id, quantity, available_quantity, average_cost, cost_amount, position_source, as_of_date
            FROM holdings WHERE id = ?1
            ",
            [id],
            Self::map_holding,
        ).map_err(Into::into)
    }

    pub fn update_holding(&self, id: i64, input: HoldingUpdate) -> DatabaseResult<Holding> {
        self.connection.execute(
            "
            UPDATE holdings
            SET quantity = ?1, available_quantity = ?2, average_cost = ?3, cost_amount = ?4, as_of_date = ?5,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?6
            ",
            params![
                input.quantity,
                input.available_quantity,
                input.average_cost,
                input.cost_amount,
                input.as_of_date,
                id,
            ],
        )?;
        self.get_holding(id)
    }

    /// Preserves the immutable transaction history while making a record inactive for later rules.
    pub fn cancel_transaction(&self, id: i64) -> DatabaseResult<Transaction> {
        self.connection.execute(
            "
                UPDATE transactions
                SET status = 'CANCELLED', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE id = ?1
                ",
            [id],
        )?;
        self.get_transaction(id)
    }

    pub fn delete_holding(&self, id: i64) -> DatabaseResult<usize> {
        self.connection
            .execute("DELETE FROM holdings WHERE id = ?1", [id])
            .map_err(Into::into)
    }

    pub fn delete_security(&self, id: i64) -> DatabaseResult<usize> {
        self.connection
            .execute("DELETE FROM securities WHERE id = ?1", [id])
            .map_err(Into::into)
    }

    fn upsert_market_source(&self, snapshot: &MarketSnapshot) -> DatabaseResult<i64> {
        let (status, last_success_at, last_error) = if snapshot.delay_status == MarketStatus::NoData
        {
            (
                MarketStatus::NoData.as_str(),
                None,
                snapshot.unavailable_reason.as_deref(),
            )
        } else {
            ("ACTIVE", Some(snapshot.fetched_at.to_rfc3339()), None)
        };
        self.connection.execute(
            "
            INSERT INTO data_sources (
                name, source_type, priority, base_url, enabled, status, last_success_at, last_error
            ) VALUES (?1, 'MARKET', ?2, ?3, 1, ?4, ?5, ?6)
            ON CONFLICT(name) DO UPDATE SET
                source_type = excluded.source_type,
                priority = excluded.priority,
                base_url = excluded.base_url,
                status = excluded.status,
                last_success_at = excluded.last_success_at,
                last_error = excluded.last_error,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                &snapshot.source.name,
                snapshot.source.priority.as_i64(),
                &snapshot.source.base_url,
                status,
                last_success_at,
                last_error,
            ],
        )?;
        self.connection
            .query_row(
                "SELECT id FROM data_sources WHERE name = ?1",
                [&snapshot.source.name],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn get_security(&self, id: i64) -> DatabaseResult<Security> {
        self.connection
            .query_row(
                "
            SELECT id, symbol, name, market, exchange, security_type, industry, concepts_json, trade_rule
            FROM securities WHERE id = ?1
            ",
                [id],
                |row| {
                    Ok(Security {
                        id: row.get(0)?,
                        symbol: row.get(1)?,
                        name: row.get(2)?,
                        market: row.get(3)?,
                        exchange: row.get(4)?,
                        security_type: row.get(5)?,
                        industry: row.get(6)?,
                        concepts_json: row.get(7)?,
                        trade_rule: row.get(8)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    fn get_cash_account(&self, id: i64) -> DatabaseResult<CashAccount> {
        self.connection
            .query_row(
                "
            SELECT id, name, currency, available_to_buy, withdrawable_cash, pending_settlement
            FROM cash_accounts WHERE id = ?1
            ",
                [id],
                |row| {
                    Ok(CashAccount {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        currency: row.get(2)?,
                        available_to_buy: row.get(3)?,
                        withdrawable_cash: row.get(4)?,
                        pending_settlement: row.get(5)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    fn get_transaction(&self, id: i64) -> DatabaseResult<Transaction> {
        self.connection.query_row(
            "
            SELECT id, cash_account_id, security_id, side, status, record_source, trade_date, quantity, price,
                   commission, stamp_tax, transfer_fee, other_fee, minimum_commission,
                   external_reference, import_batch_id, note
            FROM transactions WHERE id = ?1
            ",
            [id],
            |row| {
                Ok(Transaction {
                    id: row.get(0)?,
                    cash_account_id: row.get(1)?,
                    security_id: row.get(2)?,
                    side: row.get(3)?,
                    status: row.get(4)?,
                    record_source: row.get(5)?,
                    trade_date: row.get(6)?,
                    quantity: row.get(7)?,
                    price: row.get(8)?,
                    commission: row.get(9)?,
                    stamp_tax: row.get(10)?,
                    transfer_fee: row.get(11)?,
                    other_fee: row.get(12)?,
                    minimum_commission: row.get(13)?,
                    external_reference: row.get(14)?,
                    import_batch_id: row.get(15)?,
                    note: row.get(16)?,
                })
            },
        ).map_err(Into::into)
    }

    fn map_holding(row: &Row<'_>) -> rusqlite::Result<Holding> {
        Ok(Holding {
            id: row.get(0)?,
            cash_account_id: row.get(1)?,
            security_id: row.get(2)?,
            quantity: row.get(3)?,
            available_quantity: row.get(4)?,
            average_cost: row.get(5)?,
            cost_amount: row.get(6)?,
            position_source: row.get(7)?,
            as_of_date: row.get(8)?,
        })
    }
}

impl MarketSnapshotStore for DatabaseService {
    fn save_market_snapshot(&self, snapshot: &MarketSnapshot) -> Result<(), MarketStoreError> {
        self.save_market_snapshot_inner(snapshot)
            .map_err(|error| MarketStoreError {
                message: error.to_string(),
            })
    }
}

impl DatabaseService {
    fn save_market_snapshot_inner(&self, snapshot: &MarketSnapshot) -> DatabaseResult<()> {
        let source_id = self.upsert_market_source(snapshot)?;

        // A NO_DATA result has no provider market timestamp. Persist its safe source status only;
        // inserting a fabricated timestamp into market_snapshots would violate traceability rules.
        let Some(market_timestamp) = snapshot.market_timestamp else {
            return Ok(());
        };

        self.connection.execute(
            "
            INSERT INTO market_snapshots (
                data_source_id, snapshot_kind, market_timestamp, fetched_at, delay_status
            ) VALUES (?1, 'FULL', ?2, ?3, ?4)
            ",
            params![
                source_id,
                market_timestamp.to_rfc3339(),
                snapshot.fetched_at.to_rfc3339(),
                snapshot.delay_status.as_str(),
            ],
        )?;
        let snapshot_id = self.connection.last_insert_rowid();

        for quote in &snapshot.quotes {
            self.connection.execute(
                "
                INSERT INTO market_quotes (
                    market_snapshot_id, security_id, data_source_id, current_price, change_pct,
                    market_timestamp, fetched_at, delay_status, symbol, security_name, market,
                    previous_close, price_change, volume, volume_unit, turnover_amount, turnover_unit, source
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
                )
                ON CONFLICT(security_id, data_source_id, market_timestamp) DO UPDATE SET
                    market_snapshot_id = excluded.market_snapshot_id,
                    current_price = excluded.current_price,
                    change_pct = excluded.change_pct,
                    fetched_at = excluded.fetched_at,
                    delay_status = excluded.delay_status,
                    symbol = excluded.symbol,
                    security_name = excluded.security_name,
                    market = excluded.market,
                    previous_close = excluded.previous_close,
                    price_change = excluded.price_change,
                    volume = excluded.volume,
                    volume_unit = excluded.volume_unit,
                    turnover_amount = excluded.turnover_amount,
                    turnover_unit = excluded.turnover_unit,
                    source = excluded.source
                ",
                params![
                    snapshot_id,
                    quote.security_id,
                    source_id,
                    quote.current_price.to_string(),
                    quote.change_percent.to_string(),
                    quote.market_timestamp.to_rfc3339(),
                    quote.fetched_at.to_rfc3339(),
                    quote.delay_status.as_str(),
                    &quote.symbol,
                    &quote.name,
                    &quote.market,
                    quote.previous_close.to_string(),
                    quote.price_change.to_string(),
                    quote.volume.to_string(),
                    &quote.volume_unit,
                    quote.turnover_amount.to_string(),
                    &quote.turnover_unit,
                    &quote.source,
                ],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_service::{
        DataSourcePriority, MarketDataSource, MarketQuote, MarketSnapshot, MarketStatus,
        SourceClass,
    };
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;

    fn decimal(value: &str) -> Decimal {
        value.parse().expect("valid decimal fixture")
    }

    fn new_security() -> NewSecurity {
        NewSecurity {
            symbol: "600519".into(),
            name: "贵州茅台".into(),
            market: "SSE".into(),
            exchange: "SSE".into(),
            security_type: "STOCK".into(),
            industry: Some("白酒".into()),
            concepts_json: "[\"消费\"]".into(),
            trade_rule: "T_PLUS_1".into(),
        }
    }

    fn new_cash_account() -> NewCashAccount {
        NewCashAccount {
            name: "主账户".into(),
            currency: "CNY".into(),
            available_to_buy: "100000.00".into(),
            withdrawable_cash: "100000.00".into(),
            pending_settlement: "0".into(),
        }
    }

    #[test]
    fn migrations_are_versioned_and_idempotent() {
        let mut connection = Connection::open_in_memory().expect("create in-memory database");
        migrations::apply(&mut connection).expect("first migration run");
        migrations::apply(&mut connection).expect("second migration run");

        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("read migration count");
        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                    'securities', 'holdings', 'transactions', 'cash_accounts',
                    'market_snapshots', 'market_quotes', 'data_sources'
                )",
                [],
                |row| row.get(0),
            )
            .expect("verify core tables");

        assert_eq!(migration_count, 3);
        assert_eq!(table_count, 7);
    }

    #[test]
    fn core_crud_persists_only_user_entered_records() {
        let database = DatabaseService::open_in_memory().expect("initialize database");

        let security = database
            .create_security(new_security())
            .expect("create security");
        assert_eq!(security.symbol, "600519");

        let cash_account = database
            .create_cash_account(new_cash_account())
            .expect("create cash account");

        let holding = database
            .create_holding(NewHolding {
                cash_account_id: cash_account.id,
                security_id: security.id,
                quantity: 100,
                available_quantity: 100,
                average_cost: "1500.00".into(),
                cost_amount: "150000.00".into(),
                position_source: "INITIAL_POSITION".into(),
                as_of_date: Some("2026-08-08".into()),
            })
            .expect("create holding");

        let transaction = database
            .create_transaction(NewTransaction {
                cash_account_id: cash_account.id,
                security_id: security.id,
                side: "BUY".into(),
                status: "CONFIRMED".into(),
                record_source: "MANUAL".into(),
                trade_date: "2026-08-08".into(),
                quantity: 100,
                price: "1500.00".into(),
                commission: "5.00".into(),
                stamp_tax: "0".into(),
                transfer_fee: "0".into(),
                other_fee: "0".into(),
                minimum_commission: "5.00".into(),
                external_reference: None,
                import_batch_id: None,
                note: Some("CRUD test record".into()),
            })
            .expect("create transaction");
        assert_eq!(transaction.quantity, 100);

        let fetched = database.get_holding(holding.id).expect("query holding");
        assert_eq!(fetched.average_cost, "1500.00");

        let updated = database
            .update_holding(
                holding.id,
                HoldingUpdate {
                    quantity: 200,
                    available_quantity: 100,
                    average_cost: "1550.00".into(),
                    cost_amount: "310000.00".into(),
                    as_of_date: Some("2026-08-09".into()),
                },
            )
            .expect("update holding");
        assert_eq!(updated.quantity, 200);
        assert_eq!(updated.average_cost, "1550.00");
        assert_eq!(updated.cost_amount, "310000.00");

        let cancelled = database
            .cancel_transaction(transaction.id)
            .expect("cancel transaction");
        assert_eq!(cancelled.status, "CANCELLED");
        assert_eq!(
            database.delete_holding(holding.id).expect("delete holding"),
            1
        );
    }

    #[test]
    fn migration_002_upgrades_001_without_losing_existing_records() {
        let mut connection = Connection::open_in_memory().expect("create legacy database");
        migrations::apply_001_for_upgrade_test(&mut connection).expect("apply 001 only");

        connection
            .execute(
                "
                INSERT INTO securities (symbol, name, market, instrument_type, concepts_json, trading_rule)
                VALUES ('510300', '沪深300ETF', 'SSE', 'ETF', '[]', 'T_PLUS_ZERO')
                ",
                [],
            )
            .expect("insert legacy security");
        let security_id = connection.last_insert_rowid();
        connection
            .execute("INSERT INTO cash_accounts (name) VALUES ('旧账户')", [])
            .expect("insert legacy account");
        let cash_account_id = connection.last_insert_rowid();
        connection
            .execute(
                "
                INSERT INTO holdings (cash_account_id, security_id, quantity, available_quantity, average_cost)
                VALUES (?1, ?2, 100, 100, '3.85')
                ",
                [cash_account_id, security_id],
            )
            .expect("insert legacy holding");
        connection
            .execute(
                "
                INSERT INTO transactions (
                    cash_account_id, security_id, side, trade_date, quantity, price, commission, stamp_duty
                ) VALUES (?1, ?2, 'BUY', '2026-08-08', 100, '3.85', '5.00', '0.10')
                ",
                [cash_account_id, security_id],
            )
            .expect("insert legacy transaction");

        migrations::apply(&mut connection).expect("upgrade database through 003");

        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("read migration count");
        let upgraded_security: (String, String, String) = connection
            .query_row(
                "SELECT exchange, security_type, trade_rule FROM securities WHERE id = ?1",
                [security_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read upgraded security");
        let upgraded_holding: (i64, String, String) = connection
            .query_row(
                "SELECT quantity, average_cost, cost_amount FROM holdings WHERE security_id = ?1",
                [security_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read upgraded holding");
        let upgraded_transaction: (String, String, String, String) = connection
            .query_row(
                "SELECT side, status, stamp_tax, minimum_commission FROM transactions WHERE security_id = ?1",
                [security_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read upgraded transaction");
        let corporate_actions_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'corporate_actions'",
                [],
                |row| row.get(0),
            )
            .expect("verify corporate action table");

        assert_eq!(migration_count, 3);
        assert_eq!(
            upgraded_security,
            ("SSE".into(), "ETF".into(), "T_PLUS_0".into())
        );
        assert_eq!(upgraded_holding, (100, "3.85".into(), "0".into()));
        assert_eq!(
            upgraded_transaction,
            ("BUY".into(), "CONFIRMED".into(), "0.10".into(), "0".into())
        );
        assert_eq!(corporate_actions_exists, 1);
    }

    #[test]
    fn market_snapshot_store_persists_normalized_quotes_and_no_data_source_state() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let security = database
            .create_security(new_security())
            .expect("create security for quote");
        let fetched_at = Utc.with_ymd_and_hms(2026, 8, 10, 2, 0, 0).unwrap();
        let market_timestamp = Utc.with_ymd_and_hms(2026, 8, 10, 1, 59, 0).unwrap();
        let source = MarketDataSource {
            name: "Recorded test market source".into(),
            base_url: "https://test.invalid/quotes".into(),
            priority: DataSourcePriority::PublicQuote,
            source_class: SourceClass::PublicQuote,
        };
        let snapshot = MarketSnapshot {
            source: source.clone(),
            market_timestamp: Some(market_timestamp),
            fetched_at,
            delay_status: MarketStatus::Delayed,
            quotes: vec![MarketQuote {
                security_id: security.id,
                symbol: security.symbol.clone(),
                name: security.name.clone(),
                market: security.market.clone(),
                current_price: decimal("1500.00"),
                previous_close: decimal("1490.00"),
                price_change: decimal("10.00"),
                change_percent: decimal("0.6711"),
                volume: decimal("100"),
                volume_unit: "LOTS".into(),
                turnover_amount: decimal("150000"),
                turnover_unit: "CNY".into(),
                market_timestamp,
                fetched_at,
                source: source.name.clone(),
                delay_status: MarketStatus::Delayed,
            }],
            unavailable_reason: None,
        };

        database
            .save_market_snapshot(&snapshot)
            .expect("persist traceable quote snapshot");
        let (symbol, previous_close, price_change, volume, source_name, delay_status):
            (String, String, String, String, String, String) = database
            .connection
            .query_row(
                "SELECT symbol, previous_close, price_change, volume, source, delay_status FROM market_quotes",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .expect("read persisted quote");
        assert_eq!(symbol, "600519");
        assert_eq!(previous_close, "1490.00");
        assert_eq!(price_change, "10.00");
        assert_eq!(volume, "100");
        assert_eq!(source_name, "Recorded test market source");
        assert_eq!(delay_status, "DELAYED");

        let no_data = MarketSnapshot {
            source,
            market_timestamp: None,
            fetched_at,
            delay_status: MarketStatus::NoData,
            quotes: Vec::new(),
            unavailable_reason: Some("recorded source failure".into()),
        };
        database
            .save_market_snapshot(&no_data)
            .expect("persist no-data source state without creating a fabricated timestamp");
        let (source_status, last_error): (String, Option<String>) = database
            .connection
            .query_row(
                "SELECT status, last_error FROM data_sources WHERE name = 'Recorded test market source'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read no-data source state");
        assert_eq!(source_status, "NO_DATA");
        assert_eq!(last_error.as_deref(), Some("recorded source failure"));
    }
}
