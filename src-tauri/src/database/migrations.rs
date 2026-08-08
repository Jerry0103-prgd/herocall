use rusqlite::{Connection, OptionalExtension};

use super::service::{DatabaseError, DatabaseResult};

struct Migration {
    version: &'static str,
    checksum: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "001",
        checksum: "database-core-001-v1",
        sql: include_str!("../../migrations/001_database_core.sql"),
    },
    Migration {
        version: "002",
        checksum: "financial-domain-model-002-v1",
        sql: include_str!("../../migrations/002_financial_domain_model.sql"),
    },
];

pub(super) fn apply(connection: &mut Connection) -> DatabaseResult<()> {
    connection.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            checksum TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );
        ",
    )?;

    for migration in MIGRATIONS {
        let applied_checksum: Option<String> = connection
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version = ?1",
                [migration.version],
                |row| row.get(0),
            )
            .optional()?;

        match applied_checksum {
            Some(checksum) if checksum == migration.checksum => {}
            Some(_) => {
                return Err(DatabaseError::MigrationChecksum {
                    version: migration.version,
                });
            }
            None => {
                let transaction = connection.transaction()?;
                transaction.execute_batch(migration.sql)?;
                transaction.execute(
                    "INSERT INTO schema_migrations (version, checksum) VALUES (?1, ?2)",
                    [migration.version, migration.checksum],
                )?;
                transaction.commit()?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
pub(super) fn apply_001_for_upgrade_test(connection: &mut Connection) -> DatabaseResult<()> {
    connection.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            checksum TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );
        ",
    )?;
    connection.execute_batch(MIGRATIONS[0].sql)?;
    connection.execute(
        "INSERT INTO schema_migrations (version, checksum) VALUES (?1, ?2)",
        [MIGRATIONS[0].version, MIGRATIONS[0].checksum],
    )?;
    Ok(())
}
