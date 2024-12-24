use std::fmt;

pub trait ResultExt {
    type T;
    type E;

    fn trace(&self) -> ResultTrace<Self::T, Self::E>;
}

pub struct ResultTrace<'a, T, E>(&'a Result<T, E>);

impl<T, E> fmt::Display for ResultTrace<'_, T, E>
where
    T: fmt::Display,
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Ok(v) => write!(f, "\"{}\"", v),
            Err(e) => write!(f, "Err({})", e),
        }
    }
}

impl<'a, T, E> From<&'a Result<T, E>> for ResultTrace<'a, T, E>
where
    T: fmt::Display,
    E: fmt::Display,
{
    fn from(value: &'a Result<T, E>) -> Self {
        Self(value)
    }
}

impl<T, E> ResultExt for Result<T, E>
where
    T: fmt::Display,
    E: fmt::Display,
{
    type T = T;
    type E = E;

    fn trace(&self) -> ResultTrace<'_, Self::T, Self::E> {
        self.into()
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
        assert_eq!("Err(42)", res.trace().to_string());
    }
}
