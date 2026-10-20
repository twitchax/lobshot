//! The small vocabulary every crate in the workspace shares: the `anyhow` error aliases.

/// The error type for application glue.
pub type Err = anyhow::Error;
/// A result carrying [`Err`].
pub type Res<T> = anyhow::Result<T, Err>;
/// A result with nothing to return.
pub type Void = Res<()>;
