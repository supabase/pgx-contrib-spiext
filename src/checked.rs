pub use pgx::pg_sys::panic::CaughtError;
use pgx::PgTryBuilder;
use pgx::{
    pg_sys::Datum,
    spi::{self, SpiClient, SpiTupleTable},
    PgOid,
};
use std::ops::{Deref, DerefMut};
use std::panic::{RefUnwindSafe, UnwindSafe};

use crate::subtxn::*;

#[derive(thiserror::Error, Debug)]
pub enum CheckedError {
    #[error("caught error: {0:?}")]
    // TODO: CaughtError currently doesn't implement `Error` and thus can't be made `#[from]`
    CaughtError(CaughtError),
    #[error("SPI error: {0:?}")]
    SpiError(#[from] spi::Error),
}

/// Commands for SPI interface
pub trait CheckedCommands<'a>: Deref<Target = SpiClient<'a>> {
    type Result<A>
    where
        A: 'a;

    /// Execute a read-only command, returning an error if one occurred.
    fn checked_select(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CheckedError>;
}

/// Mutable commands for SPI interface
pub trait CheckedMutCommands<'a>: DerefMut<Target = SpiClient<'a>> {
    type Result<A>
    where
        A: 'a;

    /// Execute a mutable command, returning an error if one occurred.
    fn checked_update(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CheckedError>;
}

impl<'a, Parent: SubTransactionExt + RefUnwindSafe + UnwindSafe> CheckedCommands<'a>
    for SubTransaction<Parent, RollbackOnDrop>
where
    SubTransaction<Parent, RollbackOnDrop>: Deref<Target = SpiClient<'a>>,
{
    type Result<A: 'a> = (A, Self);

    fn checked_select(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CheckedError> {
        PgTryBuilder::new(move || {
            self.select(query, limit, args)
                .map(|res| (res, self))
                .map_err(CheckedError::SpiError)
        })
        .catch_others(|e| Err(CheckedError::CaughtError(e)))
        .execute()
    }
}

impl<'a, Parent: SubTransactionExt + RefUnwindSafe + UnwindSafe> CheckedCommands<'a>
    for SubTransaction<Parent, CommitOnDrop>
where
    SubTransaction<Parent, CommitOnDrop>: Deref<Target = SpiClient<'a>>,
    SubTransaction<Parent, RollbackOnDrop>: Deref<Target = SpiClient<'a>>,
{
    type Result<A: 'a> = (A, Self);

    fn checked_select(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CheckedError> {
        self.rollback_on_drop()
            .checked_select(query, limit, args)
            .map(|(v, s)| (v, s.commit_on_drop()))
    }
}

impl<'a, Parent: SubTransactionExt + RefUnwindSafe + UnwindSafe> CheckedMutCommands<'a>
    for SubTransaction<Parent, RollbackOnDrop>
where
    SubTransaction<Parent, RollbackOnDrop>: DerefMut<Target = SpiClient<'a>>,
{
    type Result<A: 'a> = (A, Self);

    fn checked_update(
        mut self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CheckedError> {
        PgTryBuilder::new(move || {
            self.update(query, limit, args)
                .map(|res| (res, self))
                .map_err(CheckedError::SpiError)
        })
        .catch_others(|e| Err(CheckedError::CaughtError(e)))
        .execute()
    }
}

impl<'a, Parent: SubTransactionExt + RefUnwindSafe + UnwindSafe> CheckedMutCommands<'a>
    for SubTransaction<Parent, CommitOnDrop>
where
    SubTransaction<Parent, CommitOnDrop>: DerefMut<Target = SpiClient<'a>>,
    SubTransaction<Parent, RollbackOnDrop>: DerefMut<Target = SpiClient<'a>>,
{
    type Result<A: 'a> = (A, Self);

    fn checked_update(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CheckedError> {
        self.rollback_on_drop()
            .checked_update(query, limit, args)
            .map(|(v, s)| (v, s.commit_on_drop()))
    }
}
