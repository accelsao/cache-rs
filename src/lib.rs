macro_rules! vec_no_clone {
    ( $val:expr; $n:expr ) => {{
        let result: Vec<_> = std::iter::repeat_with(|| $val).take($n).collect();
        result
    }};
}

mod cache;
mod store;

use anyhow::Result;
use bytes::Bytes;

trait Cache {
    fn get(key: Bytes) -> Result<Bytes>;
    fn set(key: Bytes, value: Bytes) -> Result<()>;
}

#[macro_use]
macro_rules! vec_no_clone {
    ( $val:expr; $n:expr ) => {{
        let result: Vec<_> = std::iter::repeat_with(|| $val).take($n).collect();
        result
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
