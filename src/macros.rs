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
