use pgx::prelude::*;

pgx::pg_module_magic!();

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgx::prelude::*;
    use pgx_contrib_spiext::error::*;
    use pgx_contrib_spiext::*;
    use std::panic::catch_unwind;

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
    fn test_catch_pg_error() {
        use catch_error::catch_error;
        use subtxn::*;
        Spi::execute(|c| {
            let result = c.sub_transaction(|xact| {
                catch_error(xact, |xact| (xact.select("SLECT 1", None, None), xact))
            });
            assert!(matches!(
                result.unwrap_err(),
                Error::PG(PostgresError{ message: Some(message), ..}) if message == "syntax error at or near \"SLECT\""
            ));
        });
    }

    #[pg_test]
    fn test_into_postgres_error_propagates_rust_error() {
        use catch_error::catch_error;
        use std::any::Any;
        use subtxn::*;
        #[allow(unused_variables)]
        Spi::execute(|c| {
            let result: Result<_, Box<dyn Any + Send>> = catch_unwind(|| {
                let _ = c.sub_transaction(|xact| {
                    catch_error(xact, |xact| {
                        panic!("error");
                        #[allow(unreachable_code)]
                        ((), xact)
                    })
                    .map_err(Error::into_postgres_error)
                });
            });
            assert!(matches!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&s) if s == "error"
            ));
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
                Err(PostgresError{ message: Some(message), ..}) if message == "syntax error at or near \"SLECT\""
            ));
        });
    }

    #[pg_test]
    fn test_catch_checked_update() {
        use checked::*;
        Spi::execute(|mut c| {
            let _ = (&mut c)
                .checked_update("CREATE TABLE x ()", None, None)
                .unwrap();
            let (_, c) = c.checked_update("CREATE TABLE a ()", None, None).unwrap();
            let result = c.checked_update("CREAT TABLE x()", None, None);
            assert!(matches!(
                result,
                Err(PostgresError{ message: Some(message), ..}) if message == "syntax error at or near \"CREAT\""
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
                    Err(PostgresError{ message: Some(message), ..}) if message == "syntax error at or near \"SLECT\""
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
                    Err(PostgresError{ message: Some(message), ..}) if message == "syntax error at or near \"INSER\""
                ));
            });
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