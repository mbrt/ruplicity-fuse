#![macro_use]

macro_rules! try_or_log(
    ($e:expr) => (
        match $e {
            Ok(v) => v,
            Err(e) => {
                error!("{}", e);
                return;
            }
        }
    )
);

/// Helper macro for unwrapping an Option if possible, continuing the loop
/// if the value is None.
macro_rules! unwrap_opt_or_continue(
    ($e:expr) => (
        match $e {
            Some(v) => v,
            _ => { continue; }
        }
    )
);
