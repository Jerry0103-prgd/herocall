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
    Migration {
        version: "003",
        checksum: "market-data-003-v1",
        sql: include_str!("../../migrations/003_market_data.sql"),
    },
    Migration {
        version: "004",
        checksum: "news-004-v1",
        sql: include_str!("../../migrations/004_news.sql"),
    },
    Migration {
        version: "005",
        checksum: "daily-reviews-005-v1",
        sql: include_str!("../../migrations/005_daily_reviews.sql"),
    },
    Migration {
        version: "006",
        checksum: "ai-reviews-006-v1",
        sql: include_str!("../../migrations/006_ai_reviews.sql"),
    },
    Migration {
        version: "007",
        checksum: "events-007-v1",
        sql: include_str!("../../migrations/007_events.sql"),
    },
    Migration {
        version: "008",
        checksum: "app-settings-008-v1",
        sql: include_str!("../../migrations/008_app_settings.sql"),
    },
    Migration {
        version: "009",
        checksum: "manual-market-snapshots-009-v1",
        sql: include_str!("../../migrations/009_manual_market_snapshots.sql"),
    },
    Migration {
        version: "010",
        checksum: "ai-core-010-v1",
        sql: include_str!("../../migrations/010_ai_core.sql"),
    },
    Migration {
        version: "011",
        checksum: "market-index-change-percent-011-v1",
        sql: include_str!("../../migrations/011_market_index_change_percent.sql"),
    },
    Migration {
        version: "012",
        checksum: "disclosure-ingestion-012-v1",
        sql: include_str!("../../migrations/012_disclosure_ingestion.sql"),
    },
    Migration {
        version: "013",
        checksum: "watchlist-items-013-v1",
        sql: include_str!("../../migrations/013_watchlist_items.sql"),
    },
    Migration {
        version: "014",
        checksum: "ai-research-report-014-v1",
        sql: include_str!("../../migrations/014_ai_research_report.sql"),
    },
    Migration {
        version: "015",
        checksum: "ai-provider-settings-015-v1",
        sql: include_str!("../../migrations/015_ai_provider_settings.sql"),
    },
    Migration {
        version: "016",
        checksum: "ai-review-security-016-v1",
        sql: include_str!("../../migrations/016_ai_review_security.sql"),
    },
    Migration {
        version: "017",
        checksum: "security-data-ownership-017-v1",
        sql: include_str!("../../migrations/017_security_data_ownership.sql"),
    },
    Migration {
        version: "018",
        checksum: "tencent-tokenhub-provider-018-v1",
        sql: include_str!("../../migrations/018_tencent_tokenhub_provider.sql"),
    },
    Migration {
        version: "019",
        checksum: "index-intraday-fields-019-v1",
        sql: include_str!("../../migrations/019_index_intraday_fields.sql"),
    },
    Migration {
        version: "020",
        checksum: "ai-provider-model-configuration-020-v1",
        sql: include_str!("../../migrations/020_ai_provider_model_configuration.sql"),
    },
];

pub(super) fn apply(connection: &mut Connection) -> DatabaseResult<()> {
    apply_migrations(connection, MIGRATIONS)
}

fn apply_migrations(connection: &mut Connection, migrations: &[Migration]) -> DatabaseResult<()> {
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

    for migration in migrations {
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
    apply_migrations(connection, &MIGRATIONS[..1])
}

#[cfg(test)]
pub(super) fn apply_010_for_upgrade_test(connection: &mut Connection) -> DatabaseResult<()> {
    apply_migrations(connection, &MIGRATIONS[..10])
}

#[cfg(test)]
pub(super) fn apply_011_for_upgrade_test(connection: &mut Connection) -> DatabaseResult<()> {
    apply_migrations(connection, &MIGRATIONS[..11])
}

#[cfg(test)]
pub(super) fn apply_016_for_upgrade_test(connection: &mut Connection) -> DatabaseResult<()> {
    apply_migrations(connection, &MIGRATIONS[..16])
}

#[cfg(test)]
pub(super) fn apply_017_for_upgrade_test(connection: &mut Connection) -> DatabaseResult<()> {
    apply_migrations(connection, &MIGRATIONS[..17])
}

#[cfg(test)]
pub(super) fn apply_018_for_upgrade_test(connection: &mut Connection) -> DatabaseResult<()> {
    apply_migrations(connection, &MIGRATIONS[..18])
}
