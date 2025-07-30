/// Compares the given values and prints a pretty diff with the given message if
/// they are not equal.
#[macro_export]
macro_rules! compare {
    ($v1:expr, $v2:expr, $($arg:tt)*) => {
        if $v1 != $v2 {
            warn!("{} differs:\n{}", format!($($arg)*), pretty_assertions::Comparison::new(&$v1, &$v2))
        }
    };
}
