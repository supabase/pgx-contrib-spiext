use pgx::prelude::*;

pgx::pg_module_magic!();

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgx::pg_sys::submodules::panic::CaughtError;
    use pgx::prelude::*;
    use pgx_contrib_spiext::*;

    #[pg_test]
    fn test_sub_txn() {
        use subtxn::*;
        Spi::execute(|mut c| {
            c.update("CREATE TABLE a (v INTEGER)", None, None);
            let c = c
                .sub_transaction(|mut xact| {
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
                })
                .unwrap();
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
                .unwrap()
                .unwrap();
            drop(c);
            // The above transaction will be committed
        });

        Spi::execute(|c| {
            assert_eq!(
                1,
                c.select("SELECT COUNT(*) FROM a", Some(1), None)
                    .first()
                    .get_datum::<i32>(1)
                    .unwrap()
            );
            let c = c
                .sub_transaction(|mut xact| {
                    xact.update("INSERT INTO a VALUES (0)", None, None);
                    xact.rollback()
                })
                .unwrap();
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
            }).unwrap();
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
            }).unwrap();
        });
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
