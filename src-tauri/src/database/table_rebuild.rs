//! A referenced-table rebuild must not run DROP TABLE with FK actions enabled:
//! SQLite could cascade-delete retained cards/history before the replacement is
//! renamed. Only the migration runner uses this scope; normal writes keep FKs on.

use rusqlite::Connection;

pub(super) fn run<T>(
    connection: &mut Connection,
    operation: impl FnOnce(&mut Connection) -> Result<T, String>,
) -> Result<T, String> {
    if !connection.is_autocommit() {
        return Err("A table-rebuild migration requires its own transaction.".to_string());
    }
    if !foreign_keys_enabled(connection)? {
        return Err("Foreign-key enforcement must be enabled before a migration.".to_string());
    }
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .map_err(|error| format!("Could not prepare the table-rebuild migration: {error}"))?;
    let mut guard = RestoreForeignKeys {
        connection,
        restored: false,
    };
    if foreign_keys_enabled(guard.connection)? {
        return Err("Could not suspend FK actions for the table-rebuild migration.".to_string());
    }
    let result = operation(guard.connection);
    let restoration = guard.restore();
    match (result, restoration) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(restoration)) => Err(format!("{error} {restoration}")),
    }
}

fn foreign_keys_enabled(connection: &Connection) -> Result<bool, String> {
    connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, bool>(0))
        .map_err(|error| format!("Could not inspect foreign-key enforcement: {error}"))
}

struct RestoreForeignKeys<'a> {
    connection: &'a mut Connection,
    restored: bool,
}

impl RestoreForeignKeys<'_> {
    fn restore(&mut self) -> Result<(), String> {
        // A successful callback must have committed its owned transaction. If
        // it accidentally leaves one open, roll it back rather than letting a
        // no-op PRAGMA make an uncommitted migration appear successful.
        let unfinished = !self.connection.is_autocommit();
        if unfinished {
            self.connection
                .execute_batch("ROLLBACK;")
                .map_err(|error| format!("Could not roll back the migration: {error}"))?;
        }
        self.connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|error| format!("Could not restore foreign-key enforcement: {error}"))?;
        if !foreign_keys_enabled(self.connection)? {
            return Err("Foreign-key enforcement was not restored after migration.".to_string());
        }
        self.restored = true;
        if unfinished {
            return Err("The table-rebuild migration left an unfinished transaction.".to_string());
        }
        Ok(())
    }
}

impl Drop for RestoreForeignKeys<'_> {
    fn drop(&mut self) {
        if !self.restored {
            // Covers early returns and unwinding. Normal Result paths call
            // restore explicitly so a restoration error reaches the opener.
            let _ = self.restore();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::TransactionBehavior;

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE parent (id INTEGER PRIMARY KEY);
                 CREATE TABLE child (
                     id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parent ON DELETE CASCADE
                 );
                 INSERT INTO parent VALUES (1);
                 INSERT INTO child VALUES (2, 1);",
            )
            .unwrap();
        connection
    }

    #[test]
    fn referenced_parent_rebuild_preserves_children_and_restores_enforcement() {
        let mut connection = fixture();
        run(&mut connection, |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|error| error.to_string())?;
            transaction
                .execute_batch(
                    "CREATE TABLE parent_next (id INTEGER PRIMARY KEY, label TEXT);
                     INSERT INTO parent_next (id) SELECT id FROM parent;
                     DROP TABLE parent;
                     ALTER TABLE parent_next RENAME TO parent;",
                )
                .map_err(|error| error.to_string())?;
            super::super::verify_integrity(&transaction)?;
            transaction.commit().map_err(|error| error.to_string())
        })
        .unwrap();
        let count: i64 = connection
            .query_row("SELECT count(*) FROM child", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert!(foreign_keys_enabled(&connection).unwrap());
        assert!(connection
            .execute("INSERT INTO child VALUES (3, 99)", [])
            .is_err());
    }

    #[test]
    fn failed_rebuild_rolls_back_ddl_data_and_connection_settings() {
        let mut connection = fixture();
        let result = run(&mut connection, |connection| {
            let transaction = connection
                .transaction()
                .map_err(|error| error.to_string())?;
            transaction
                .execute_batch("DROP TABLE parent; CREATE TABLE parent (id INTEGER PRIMARY KEY);")
                .map_err(|error| error.to_string())?;
            super::super::verify_integrity(&transaction)?;
            transaction.commit().map_err(|error| error.to_string())
        });
        assert!(result.is_err());
        assert!(foreign_keys_enabled(&connection).unwrap());
        let count: i64 = connection
            .query_row("SELECT count(*) FROM parent", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        super::super::verify_integrity(&connection).unwrap();
    }

    #[test]
    fn a_callback_cannot_report_success_with_an_open_transaction() {
        let mut connection = fixture();
        assert!(run(&mut connection, |connection| {
            connection
                .execute_batch("BEGIN; DELETE FROM parent;")
                .map_err(|error| error.to_string())
        })
        .is_err());
        assert!(connection.is_autocommit());
        assert!(foreign_keys_enabled(&connection).unwrap());
        let count: i64 = connection
            .query_row("SELECT count(*) FROM parent", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn nested_or_unprotected_migration_is_rejected_without_running_callback() {
        let mut connection = fixture();
        connection.execute_batch("BEGIN;").unwrap();
        assert!(run(&mut connection, |_| -> Result<(), String> {
            panic!("a nested callback must not run")
        })
        .is_err());
        assert!(!connection.is_autocommit());
        connection
            .execute_batch("ROLLBACK; PRAGMA foreign_keys = OFF;")
            .unwrap();
        assert!(run(&mut connection, |_| -> Result<(), String> {
            panic!("an unprotected callback must not run")
        })
        .is_err());
    }
}
