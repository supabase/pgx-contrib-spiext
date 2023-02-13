use pgx::prelude::*;

pgx::pg_module_magic!();

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgx::prelude::*;
    use pgx::spi::SpiClient;
    use pgx_contrib_spiext::prelude::*;
    use pgx_contrib_spiext::subtxn::CommitOnDrop;

    #[pg_test]
    fn test_subxact_smoketest() {
        Spi::connect(|mut c| {
            c.update("CREATE TABLE a (v INTEGER)", None, None).unwrap();
            let c = c.sub_transaction(|mut xact| {
                xact.update("INSERT INTO a VALUES (0)", None, None).unwrap();
                assert_eq!(
                    0,
                    xact.select("SELECT v FROM a", Some(1), None)
                        .unwrap()
                        .first()
                        .get::<i64>(1)
                        .unwrap()
                        .unwrap()
                );
                let xact = xact.sub_transaction(|mut xact| {
                    xact.update("INSERT INTO a VALUES (1)", None, None).unwrap();
                    assert_eq!(
                        2,
                        xact.select("SELECT COUNT(*) FROM a", Some(1), None)
                            .unwrap()
                            .first()
                            .get::<i64>(1)
                            .unwrap()
                            .unwrap()
                    );
                    xact.rollback()
                });
                xact.rollback()
            });
            assert_eq!(
                0,
                c.select("SELECT COUNT(*) FROM a", Some(1), None)
                    .unwrap()
                    .first()
                    .get::<i64>(1)
                    .unwrap()
                    .unwrap()
            );
        })
    }

    #[pg_test]
    fn test_commit_on_drop() {
        Spi::connect(|mut c| {
            c.update("CREATE TABLE a (v INTEGER)", None, None).unwrap();
            // The type below is explicit to ensure it's commit on drop by default
            c.sub_transaction(|mut xact: SubTransaction<SpiClient, CommitOnDrop>| {
                xact.update("INSERT INTO a VALUES (0)", None, None).unwrap();
                // Dropped explicitly for illustration purposes
                drop(xact);
            });
            // Create a new client to check the state
            Spi::connect(|c| {
                // The above insert should have been committed
                assert_eq!(
                    1,
                    c.select("SELECT COUNT(*) FROM a", Some(1), None)
                        .unwrap()
                        .first()
                        .get::<i64>(1)
                        .unwrap()
                        .unwrap()
                );
            });
        })
    }

    #[pg_test]
    fn test_rollback_on_drop() {
        Spi::connect(|mut c| {
            c.update("CREATE TABLE a (v INTEGER)", None, None).unwrap();
            // The type below is explicit to ensure it's commit on drop by default
            c.sub_transaction(|mut xact: SubTransaction<SpiClient, CommitOnDrop>| {
                xact.update("INSERT INTO a VALUES (0)", None, None).unwrap();
                let xact = xact.rollback_on_drop();
                // Dropped explicitly for illustration purposes
                drop(xact);
            });
            // Create a new client to check the state
            Spi::connect(|c| {
                // The above insert should NOT have been committed
                assert_eq!(
                    0,
                    c.select("SELECT COUNT(*) FROM a", Some(1), None)
                        .unwrap()
                        .first()
                        .get::<i64>(1)
                        .unwrap()
                        .unwrap()
                );
            });
        })
    }

    #[pg_test]
    fn test_checked_select() {
        Spi::connect(|c| {
            c.sub_transaction(|xact| {
                // Ensure xact is passed through
                let (_, xact) = xact.checked_select("SELECT 1", None, None).unwrap();
                let result = xact.checked_select("SLECT", None, None);
                assert!(matches!(result, Err(CheckedError::CaughtError(CaughtError::PostgresError(error))) if error.message() == "syntax error at or near \"SLECT\""));
            });
            Ok::<_, spi::Error>(())
        }).unwrap();
    }

    #[pg_test]
    fn test_checked_update() {
        Spi::connect(|c| {
            c.sub_transaction(|xact| {
                // Ensure xact is passed through
                let (_, xact) = xact.checked_update("CREATE TABLE q ()", None, None).unwrap();
                let result = xact.checked_update("DLETE", None, None);
                assert!(matches!(result, Err(CheckedError::CaughtError(CaughtError::PostgresError(error))) if error.message() == "syntax error at or near \"DLETE\""));
            });
            Ok::<_, spi::Error>(())
        }).unwrap();
    }
}

#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
