// plctag-rs
//
// a rust wrapper of libplctag, with rust style APIs and useful extensions.
// Copyright: 2020-2021, Joylei <leingliu@gmail.com>
// License: MIT

#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

extern crate plctag_core;
extern crate tokio;
#[macro_use]
extern crate log;
#[macro_use]
extern crate async_trait;

mod cell;
mod entry;
mod op;
mod pool;

pub use entry::TagEntry;
pub use op::AsyncTag;

use plctag_core::{RawTag, Status};
use std::{fmt, sync::Arc};
use tokio::task::{self, JoinError};

/// Tag instance will be put into pool for reuse.
///
/// # Note
/// - Tag instances will not drop if the [`PoolEntry`] or [`Pool`] is still on the stack
///
pub type Pool = pool::Pool<RawTag>;
/// Tag Instance in the pool
pub type PoolEntry = pool::Entry<RawTag>;
/// Tag exclusive reference
pub type TagRef<'a> = private::TagRef<'a, RawTag>;
/// result for [`plctag-async`]
pub type Result<T> = std::result::Result<T, Error>;

/// errors for [`plctag-async`]
#[derive(Debug)]
pub enum Error {
    /// plc tag error
    TagError(Status),
    /// tokio task join error
    JoinError(tokio::task::JoinError),
    /// other error
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::TagError(_) => None,
            Error::JoinError(e) => Some(e),
            Error::Other(e) => Some(e.as_ref()),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TagError(e) => write!(f, "TagError - {}", e),
            Error::JoinError(e) => write!(f, "{}", e),
            Error::Other(e) => write!(f, "{}", e),
        }
    }
}

impl From<Status> for Error {
    fn from(s: Status) -> Self {
        Error::TagError(s)
    }
}

impl From<JoinError> for Error {
    fn from(e: JoinError) -> Self {
        Error::JoinError(e)
    }
}

mod private {
    /// exclusive tag ref to ensure thread and operations safety
    pub struct TagRef<'a, T> {
        pub(crate) tag: &'a T,
        #[allow(dead_code)]
        pub(crate) lock: tokio::sync::MutexGuard<'a, ()>,
    }

    impl<T> AsRef<T> for TagRef<'_, T> {
        #[inline(always)]
        fn as_ref(&self) -> &T {
            &self.tag
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_entry() -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let path = "make=system&family=library&name=debug&debug=4";
            let entry = TagEntry::create(path).await?;
            let tag = entry.get().await?;

            let level: i32 = tag.read_value(0).await?;
            assert_eq!(level, 4);

            tag.write_value(0, 1).await?;
            let level: i32 = tag.read_value(0).await?;
            assert_eq!(level, 1);
            Ok(())
        })
    }

    #[test]
    fn test_pool() -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let pool = Pool::new();
            let path = "make=system&family=library&name=debug&debug=4";

            //retrieve 1st
            {
                let entry = pool.entry(path).await?;
                let tag = entry.get().await?;

                let level: i32 = tag.read_value(0).await?;
                assert_eq!(level, 4);

                tag.write_value(0, &1_i32).await?;
                let level: i32 = tag.read_value(0).await?;
                assert_eq!(level, 1);
            }

            //retrieve 2nd
            {
                let entry = pool.entry(path).await?;
                let tag = entry.get().await?;

                let level: i32 = tag.read_value(0).await?;
                assert_eq!(level, 1);
            }
            Ok(())
        })
    }
}
