use pgx::pg_sys::panic::CaughtError;
use pgx::PgTryBuilder;
use pgx::{pg_sys::Datum, PgOid, SpiClient, SpiTupleTable};
use std::ops::{Deref, DerefMut};
use std::panic::{RefUnwindSafe, UnwindSafe};

use crate::subtxn::*;

/// Read-only commands for SPI interface
pub trait CheckedCommands {
    type Result<A>;

    /// Execute a read-only command, returning an error if one occurred.
    fn checked_select(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CaughtError>;
}

/// Mutable commands for SPI interface
pub trait CheckedMutCommands {
    type Result<A>;

    /// Execute a mutable command, returning an error if one occurred.
    fn checked_update(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CaughtError>;
}

impl<Parent: Deref<Target = SpiClient> + UnwindSafe + RefUnwindSafe> CheckedCommands
    for SubTransaction<Parent, false>
{
    type Result<A> = (A, SubTransaction<Parent, false>);

    fn checked_select(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CaughtError> {
        PgTryBuilder::new(move || Ok((self.select(query, limit, args), self)))
            .catch_others(|e| Err(e))
            .execute()
    }
}

impl<Parent: Deref<Target = SpiClient> + UnwindSafe + RefUnwindSafe> CheckedCommands
    for SubTransaction<Parent, true>
{
    type Result<A> = (A, SubTransaction<Parent, true>);

    fn checked_select(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CaughtError> {
        self.rollback_on_drop()
            .checked_select(query, limit, args)
            .map(|(res, xact)| (res, xact.commit_on_drop()))
    }
}

impl<Parent: DerefMut<Target = SpiClient> + UnwindSafe + RefUnwindSafe> CheckedMutCommands
    for SubTransaction<Parent, false>
{
    type Result<A> = (A, SubTransaction<Parent, false>);

    fn checked_update(
        mut self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CaughtError> {
        PgTryBuilder::new(move || Ok((self.update(query, limit, args), self)))
            .catch_others(|e| Err(e))
            .execute()
    }
}

impl<Parent: DerefMut<Target = SpiClient> + UnwindSafe + RefUnwindSafe> CheckedMutCommands
    for SubTransaction<Parent, true>
{
    type Result<A> = (A, SubTransaction<Parent, true>);

    fn checked_update(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CaughtError> {
        self.rollback_on_drop()
            .checked_update(query, limit, args)
            .map(|(res, xact)| (res, xact.commit_on_drop()))
    }
}

impl CheckedCommands for SpiClient {
    type Result<A> = (A, Self);

    fn checked_select(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CaughtError> {
        RetainedSpiClient(self.clone())
            .sub_transaction(|xact| xact.checked_select(query, limit, args))
            .map(|(table, xact)| (table, *xact.commit()))
    }
}

impl<'a> CheckedCommands for &'a SpiClient {
    type Result<A> = A;

    fn checked_select(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CaughtError> {
        RetainedSpiClient(self.clone())
            .sub_transaction(|xact| xact.checked_select(query, limit, args))
            .map(|(table, _xact): (_, SubTransaction<_, true>)| table)
    }
}

impl CheckedMutCommands for SpiClient {
    type Result<A> = (A, Self);

    fn checked_update(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CaughtError> {
        RetainedSpiClient(self.clone())
            .sub_transaction(|xact| xact.checked_update(query, limit, args))
            .map(|(table, xact)| (table, *xact.commit()))
    }
}

impl<'a> CheckedMutCommands for &'a mut SpiClient {
    type Result<A> = A;

    fn checked_update(
        self,
        query: &str,
        limit: Option<i64>,
        args: Option<Vec<(PgOid, Option<Datum>)>>,
    ) -> Result<Self::Result<SpiTupleTable>, CaughtError> {
        RetainedSpiClient(self.clone())
            .sub_transaction(|xact| xact.checked_update(query, limit, args))
            .map(|(table, _xact): (_, SubTransaction<_, true>)| table)
    }
}
