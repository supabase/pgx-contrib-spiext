use pgx::prelude::*;

pgx::pg_module_magic!();

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgx::pg_sys::submodules::panic::CaughtError;
    use pgx::prelude::*;
    use pgx::SpiClient;
    use pgx_contrib_spiext::*;

    #[pg_test]
    fn test_sub_txn() {
        use subtxn::*;
        Spi::execute(|mut c| {
            c.update("CREATE TABLE a (v INTEGER)", None, None);
            let c = c.sub_transaction(|mut xact| {
                xact.update("INSERT INTO a VALUES (0)", None, None);
                assert_eq!(
                    0,
                    xact.select("SELECT v FROM a", Some(1), None)
                        .first()
                        .get_datum::<i32>(1)
                        .unwrap()
                );
                let xact = xact.sub_transaction(|mut xact| {
                    xact.update("INSERT INTO a VALUES (1)", None, None);
                    assert_eq!(
                        2,
                        xact.select("SELECT COUNT(*) FROM a", Some(1), None)
                            .first()
                            .get_datum::<i32>(1)
                            .unwrap()
                    );
                    xact.rollback()
                });
                xact.rollback()
            });
            assert_eq!(
                0,
                c.select("SELECT COUNT(*) FROM a", Some(1), None)
                    .first()
                    .get_datum::<i32>(1)
                    .unwrap()
            );
        })
    }

    #[pg_test]
    fn test_subtxn_checked_execution_smoketest() {
        use checked::*;
        use subtxn::*;
        Spi::execute(|mut c| {
            c.update("CREATE TABLE a (v INTEGER)", None, None);
            let (_, c) = c
                .sub_transaction(|xact| xact.checked_update("INSERT INTO a VALUES (0)", None, None))
                .unwrap();
            drop(c);
            // The above transaction will be committed

            // We use SpiClient here because `c` was consumed. It's not the best way to
            // handle this, but we needed to simulate dropping the sub-transaction
            assert_eq!(
                1,
                SpiClient
                    .select("SELECT COUNT(*) FROM a", Some(1), None)
                    .first()
                    .get_datum::<i32>(1)
                    .unwrap()
            );
            let c = SpiClient.sub_transaction(|mut xact| {
                xact.update("INSERT INTO a VALUES (0)", None, None);
                xact.rollback()
            });
            // The above transaction will be rolled back (as explicitly requested)
            assert_eq!(
                1,
                c.select("SELECT COUNT(*) FROM a", Some(1), None)
                    .first()
                    .get_datum::<i32>(1)
                    .unwrap()
            );
        });
    }

    #[pg_test]
    fn test_catch_checked_select() {
        use checked::*;
        Spi::execute(|c| {
            let _ = (&c).checked_select("SELECT 1", None, None).unwrap();
            let (_, c) = c.checked_select("SELECT 1", None, None).unwrap();
            let result = c.checked_select("SLECT 1", None, None);
            assert!(matches!(
                result,
                Err(CaughtError::PostgresError(error)) if error.message() == "syntax error at or near \"SLECT\""
            ));
        });
    }

    #[pg_test]
    fn test_catch_checked_update() {
        use checked::*;
        Spi::execute(|mut c| {
            let txid = unsafe { pg_sys::GetCurrentSubTransactionId() };
            let _ = (&mut c)
                .checked_update("CREATE TABLE x ()", None, None)
                .unwrap();
            // Ensure we're no longer in the a sub-transaction created by `checked_update`
            let txid_ = unsafe { pg_sys::GetCurrentSubTransactionId() };
            assert!(txid == txid_);
            assert!((&c)
                .checked_select("SELECT count(*) FROM x", None, None)
                .is_ok());
            let (_, c) = c.checked_update("CREATE TABLE a ()", None, None).unwrap();
            let result = c.checked_update("CREAT TABLE x()", None, None);
            assert!(matches!(
                result,
                Err(CaughtError::PostgresError(error)) if error.message() == "syntax error at or near \"CREAT\""
            ));
        });
    }

    #[pg_test]
    fn test_catch_checked_select_txn() {
        use checked::*;
        use subtxn::*;
        Spi::execute(|c| {
            c.sub_transaction(|xact| {
                let (_, xact) = xact.checked_select("SELECT 1", None, None).unwrap();
                let result = xact.checked_select("SLECT 1", None, None);
                assert!(matches!(
                    result,
                    Err(CaughtError::PostgresError(error)) if error.message() == "syntax error at or near \"SLECT\""
                ));
            });
        });
    }

    #[pg_test]
    fn test_catch_checked_update_txn() {
        use checked::*;
        use subtxn::*;
        Spi::execute(|c| {
            c.sub_transaction(|xact| {
                let (_, xact) = xact
                    .checked_update("CREATE TABLE a ()", None, None)
                    .unwrap();
                let result = xact.checked_update("INSER INTO a VALUES ()", None, None);
                assert!(matches!(
                    result,
                    Err(CaughtError::PostgresError(error)) if error.message() == "syntax error at or near \"INSER\""
                ));
            });
        });
    }

    #[pg_test]
    fn test_multi_ops_in_subtransaction_returning_data() {
        use checked::*;
        use subtxn::*;

        let spi_result: Result<i32, CaughtError> = Spi::connect(|c| {
            //Create subtransaction
            let sub_txn_result: Result<i32, CaughtError> = c.sub_transaction(|xact| {
                // Operation 1, with early return if it fails
                let (_, xact) = xact.checked_update("CREATE TABLE a (id int)", None, None)?;
                // Operation 2, with early return if it fails
                let (_, xact) = xact.checked_update("INSERT INTO a VALUES (1)", None, None)?;
                // Operation 3, with early return if it fails
                let (val, _) =
                    xact.checked_update("INSERT INTO a VALUES (2) returning id", None, None)?;
                // Get a value from the query
                let out_val: i32 = val.first().get_datum::<i32>(1).unwrap();
                // Return that value from the closure
                Ok(out_val)
            });

            // Spi::connect requires a Result<Option<_>, SpiError>
            // and unwraps the outer result type, panic-ing if it finds an SpiError.
            // to return our own result, we must wrap it in an Result<Option<T>>
            Ok(Some(sub_txn_result))
        })
        .unwrap();

        assert!(matches!(spi_result, Ok(2)));

        Spi::execute(|c| {
            let count = c
                .select("select count(*) from a", None, None)
                .first()
                .get_datum::<i32>(1)
                .unwrap();
            assert_eq!(count, 2);
        })
    }

    #[pg_test]
    fn test_xxx() {
        use checked::*;
        use subtxn::*;

        Spi::execute(|mut c| {
            c.update("CREATE TABLE abc (v INTEGER)", None, None);
        });

        let spi_result: Result<i32, String> = Spi::connect(|c| {
            //Create subtransaction
            let sub_txn_result: Result<i32, String> = c.sub_transaction(|xact| {
                // Postgres operation 1 (may fail)
                let (val, xact) = xact
                    .checked_update("insert into abc(id) values 1", None, None)
                    .map_err(|_| "insert 1 failed".to_string())?;

                // Rust operation, may fail
                let n_inserted = val.first().get_datum::<i32>(1).unwrap();
                if n_inserted != 1 {
                    Err("too many rows inserted".to_string()).map_err(|e| {
                        xact.rollback();
                        e
                    })?;
                }

                // Postgres operation 2
                // [rustc E0382] [E] use of moved value: `xact`
                let (val, _) = xact
                    .checked_update("insert into abc(id) values 2", None, None)
                    .map_err(|_| "insert 2 failed".to_string())?;

                let return_val: i32 = val.first().get_datum::<i32>(1).unwrap();
                // Return that value from the closure
                Ok(return_val)
            });

            Ok(Some(sub_txn_result))
        })
        .unwrap();

        assert!(matches!(spi_result, Ok(2)));

        Spi::execute(|c| {
            let count = c
                .select("select count(*) from a", None, None)
                .first()
                .get_datum::<i32>(1)
                .unwrap();
            assert_eq!(count, 2);
        })
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
