//! Search, read and analyse the Hebrew text of the Dead Sea Scrolls.
//!
//! The library holds everything that needs no operating-system services (the corpus, the
//! statistics, the table output), so that it could also be compiled for the web.

pub mod bible;
pub mod builder;
pub mod corpus;
pub mod hebrew;
pub mod query;
pub mod stats;
pub mod table;
pub mod tf;
