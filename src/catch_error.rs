//! # Error-catching
//!
//! Enables catching Rust and (most importantly) Postgres-originating errors.
//!
//! This functionality is not accessible to end-users without opting in with the `pub_catch_error`
//! feature gate. The reason for this is that it use may be brittle as we're dealing with low-level
//! stuff. If it will eventually be proven to be safe, this restriction may be removed.
use crate::subtxn::SubTransaction;
use pgx::pg_sys::panic::CaughtError;
use pgx::PgTryBuilder;
use std::panic::{RefUnwindSafe, UnwindSafe};

impl<Parent> SubTransaction<Parent> {
    /// Internal hack to keep a reference to the transaction that goes
    /// into the closure passed to `catch_error` in case if we need to roll it back.
    fn internal_clone(&mut self) -> SubTransaction<()> {
        // Don't drop the original subtxn (equates to committing it) it while it's being used so that
        // we can roll it back. Very important!
        self.drop = false;
        SubTransaction {
            memory_context: self.memory_context,
            resource_owner: self.resource_owner,
            drop: false,
            parent: Some(()),
        }
    }
}

/// Run a closure within a sub-transaction. Rolls the sub-transaction back if any panic occurs
/// and returns the captured error.
///
/// At this moment, this function is internal to `pgx-contrib-spiext`, unless the `pub_catch_error`
/// feature is enabled. This is done to potential safety risks use of this function may bring.
/// If it will eventually be proven to be safe, this restriction may be removed.
pub fn catch_error<Try, R, Parent>(
    mut subtxn: SubTransaction<Parent>,
    try_func: Try,
) -> Result<(R, SubTransaction<Parent>), CaughtError>
where
    Parent: UnwindSafe + RefUnwindSafe,
    Try: FnOnce(SubTransaction<Parent>) -> (R, SubTransaction<Parent>) + UnwindSafe + RefUnwindSafe,
{
    let original_drop_flag = subtxn.drop;
    // This is an internal reference to the transaction that we use to roll the transaction
    // back if a panic occurs.
    let subtxn_ = subtxn.internal_clone();

    let result = PgTryBuilder::new(move || {
        let (result, mut xact) = try_func(subtxn);
        // Restore original transaction's `drop` flag
        xact.drop = original_drop_flag;

        Ok((result, xact))
    })
    .catch_others(|e| Err(e))
    .execute();

    if result.is_err() {
        subtxn_.rollback();
    }

    result
}
