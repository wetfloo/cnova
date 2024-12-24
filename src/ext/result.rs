use std::fmt;

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

impl<'a, T, E> From<&'a Result<T, E>> for ResultTrace<'a, T, E> {
    fn from(value: &'a Result<T, E>) -> Self {
        Self(value)
    }
}

pub trait ResultExt {
    type T;
    type E;

    fn trace(&self) -> ResultTrace<Self::T, Self::E>;
}

impl<T, E> ResultExt for Result<T, E> {
    type T = T;
    type E = E;

    fn trace(&self) -> ResultTrace<'_, Self::T, Self::E> {
        self.into()
    }
}
