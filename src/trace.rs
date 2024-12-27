use core::fmt;

/// Allows the implementor to present itself nicely in some user-facing scenario. This is almost
/// like [`fmt::Display`], except that you don't need the type to be [`fmt::Display`], which is useful for
/// types like [`Option`] and [`Result`]
pub struct Trace<T>(T);

impl<T, E> fmt::Display for Trace<&Result<T, E>>
where
    T: fmt::Display,
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Ok(v) => write!(f, "\"{}\"", v),
            Err(e) => write!(f, "Err(\"{}\")", e),
        }
    }
}

impl<T> fmt::Display for Trace<&Option<T>>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(v) => write!(f, "\"{}\"", v),
            None => f.write_str("None"),
        }
    }
}

/// See [`Trace`] documentation
pub trait TraceExt {
    // Seems like the compiler can't see that this method is used for macros
    // and generates warnings for it, so that's why it has to be allowed
    #[allow(unused)]
    fn trace(&self) -> Trace<&Self>;
}

impl<T, E> TraceExt for Result<T, E> {
    fn trace(&self) -> Trace<&Self> {
        Trace(self)
    }
}

impl<T> TraceExt for Option<T> {
    fn trace(&self) -> Trace<&Self> {
        Trace(self)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_ok_trace() {
        let res: Result<_, i32> = Ok(69);
        assert_eq!("\"69\"", res.trace().to_string());
    }

    #[test]
    fn test_err_trace() {
        let res: Result<i32, _> = Err(42);
        assert_eq!("Err(\"42\")", res.trace().to_string());
    }

    #[test]
    fn test_some_trace() {
        assert_eq!("\"69\"", Some(69).trace().to_string());
    }

    #[test]
    fn test_none_trace() {
        let opt: Option<i32> = None;
        assert_eq!("None", opt.trace().to_string());
    }
}
