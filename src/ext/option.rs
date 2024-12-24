use std::fmt;

pub trait OptionExt {
    type T;

    fn trace(&self) -> OptionTrace<Self::T>;
}

pub struct OptionTrace<'a, T>(&'a Option<T>);

impl<T> fmt::Display for OptionTrace<'_, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Some(v) => write!(f, "\"{}\"", v),
            None => f.write_str("None"),
        }
    }
}

impl<'a, T> From<&'a Option<T>> for OptionTrace<'a, T>
where
    T: fmt::Display,
{
    fn from(value: &'a Option<T>) -> Self {
        Self(value)
    }
}

impl<T> OptionExt for Option<T>
where
    T: fmt::Display,
{
    type T = T;

    fn trace(&self) -> OptionTrace<'_, Self::T> {
        self.into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
