#![allow(dead_code)] // Phase 2-A defines the service API before later application commands consume it.

use std::{error::Error, fmt, fs, path::Path};

use rusqlite::{params, Connection, Row};
use tauri::Manager;

use super::migrations;

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
    pub instrument_type: String,
    pub industry: Option<String>,
    pub concepts_json: String,
    pub trading_rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSecurity {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub instrument_type: String,
    pub industry: Option<String>,
    pub concepts_json: String,
    pub trading_rule: String,
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
    pub position_source: String,
    pub as_of_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingUpdate {
    pub quantity: i64,
    pub available_quantity: i64,
    pub average_cost: String,
    pub as_of_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub id: i64,
    pub cash_account_id: i64,
    pub security_id: i64,
    pub side: String,
    pub record_source: String,
    pub trade_date: String,
    pub quantity: i64,
    pub price: String,
    pub commission: String,
    pub stamp_duty: String,
    pub transfer_fee: String,
    pub other_fee: String,
    pub external_reference: Option<String>,
    pub import_batch_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTransaction {
    pub cash_account_id: i64,
    pub security_id: i64,
    pub side: String,
    pub record_source: String,
    pub trade_date: String,
    pub quantity: i64,
    pub price: String,
    pub commission: String,
    pub stamp_duty: String,
    pub transfer_fee: String,
    pub other_fee: String,
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
            INSERT INTO securities (symbol, name, market, instrument_type, industry, concepts_json, trading_rule)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                input.symbol,
                input.name,
                input.market,
                input.instrument_type,
                input.industry,
                input.concepts_json,
                input.trading_rule,
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
            INSERT INTO holdings (cash_account_id, security_id, quantity, available_quantity, average_cost, position_source, as_of_date)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                input.cash_account_id,
                input.security_id,
                input.quantity,
                input.available_quantity,
                input.average_cost,
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
                cash_account_id, security_id, side, record_source, trade_date, quantity, price,
                commission, stamp_duty, transfer_fee, other_fee, external_reference, import_batch_id, note
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ",
            params![
                input.cash_account_id,
                input.security_id,
                input.side,
                input.record_source,
                input.trade_date,
                input.quantity,
                input.price,
                input.commission,
                input.stamp_duty,
                input.transfer_fee,
                input.other_fee,
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
            SELECT id, cash_account_id, security_id, quantity, available_quantity, average_cost, position_source, as_of_date
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
            SET quantity = ?1, available_quantity = ?2, average_cost = ?3, as_of_date = ?4,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?5
            ",
            params![
                input.quantity,
                input.available_quantity,
                input.average_cost,
                input.as_of_date,
                id,
            ],
        )?;
        self.get_holding(id)
    }

    pub fn delete_transaction(&self, id: i64) -> DatabaseResult<usize> {
        self.connection
            .execute("DELETE FROM transactions WHERE id = ?1", [id])
            .map_err(Into::into)
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

    fn get_security(&self, id: i64) -> DatabaseResult<Security> {
        self.connection
            .query_row(
                "
            SELECT id, symbol, name, market, instrument_type, industry, concepts_json, trading_rule
            FROM securities WHERE id = ?1
            ",
                [id],
                |row| {
                    Ok(Security {
                        id: row.get(0)?,
                        symbol: row.get(1)?,
                        name: row.get(2)?,
                        market: row.get(3)?,
                        instrument_type: row.get(4)?,
                        industry: row.get(5)?,
                        concepts_json: row.get(6)?,
                        trading_rule: row.get(7)?,
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
            SELECT id, cash_account_id, security_id, side, record_source, trade_date, quantity, price,
                   commission, stamp_duty, transfer_fee, other_fee, external_reference, import_batch_id, note
            FROM transactions WHERE id = ?1
            ",
            [id],
            |row| {
                Ok(Transaction {
                    id: row.get(0)?,
                    cash_account_id: row.get(1)?,
                    security_id: row.get(2)?,
                    side: row.get(3)?,
                    record_source: row.get(4)?,
                    trade_date: row.get(5)?,
                    quantity: row.get(6)?,
                    price: row.get(7)?,
                    commission: row.get(8)?,
                    stamp_duty: row.get(9)?,
                    transfer_fee: row.get(10)?,
                    other_fee: row.get(11)?,
                    external_reference: row.get(12)?,
                    import_batch_id: row.get(13)?,
                    note: row.get(14)?,
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
            position_source: row.get(6)?,
            as_of_date: row.get(7)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_security() -> NewSecurity {
        NewSecurity {
            symbol: "600519".into(),
            name: "贵州茅台".into(),
            market: "SSE".into(),
            instrument_type: "STOCK".into(),
            industry: Some("白酒".into()),
            concepts_json: "[\"消费\"]".into(),
            trading_rule: "T_PLUS_ONE".into(),
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

        assert_eq!(migration_count, 1);
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
                position_source: "INITIAL_POSITION".into(),
                as_of_date: Some("2026-08-08".into()),
            })
            .expect("create holding");

        let transaction = database
            .create_transaction(NewTransaction {
                cash_account_id: cash_account.id,
                security_id: security.id,
                side: "BUY".into(),
                record_source: "MANUAL".into(),
                trade_date: "2026-08-08".into(),
                quantity: 100,
                price: "1500.00".into(),
                commission: "5.00".into(),
                stamp_duty: "0".into(),
                transfer_fee: "0".into(),
                other_fee: "0".into(),
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
                    as_of_date: Some("2026-08-09".into()),
                },
            )
            .expect("update holding");
        assert_eq!(updated.quantity, 200);
        assert_eq!(updated.average_cost, "1550.00");

        assert_eq!(
            database
                .delete_transaction(transaction.id)
                .expect("delete transaction"),
            1
        );
        assert_eq!(
            database.delete_holding(holding.id).expect("delete holding"),
            1
        );
        assert_eq!(
            database
                .delete_security(security.id)
                .expect("delete security"),
            1
        );
    }
}
