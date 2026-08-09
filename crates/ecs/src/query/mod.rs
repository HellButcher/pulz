//! Query system for iterating entities with matching component sets.
//!
//! A [`Query<D, F>`](exec::Query) fetches component data `D` from all archetypes that satisfy
//! filter `F`. Implement [`QueryData`] and [`QueryFilter`] to create custom data and filter types.

mod data;
mod exec;
mod fetch;
mod filter;
mod state;

pub use data::{QueryData, QueryFetch, QueryFetchState, QueryFilter, QueryItem};
pub use exec::Query;
pub use filter::{With, Without};
