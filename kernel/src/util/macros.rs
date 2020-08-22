macro_rules! unwrap_or_return {
    ($e:expr) => {
        match $e {
            Some(value) => value,
            None => return,
        }
    };
}
