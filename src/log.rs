#[macro_export]
macro_rules! log {
    ($fmt:expr $(, $arg:expr)*) => {
        println!($fmt $(, $arg)*)
    }
}
