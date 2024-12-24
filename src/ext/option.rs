use std::fmt;

pub struct OptionTrace<'a, T>(&'a Option<T>);

pub trait OptionExt {
    type Value;

    fn trace(&self) -> OptionTrace<Self::Value>;
}

impl<T> fmt::Display for OptionTrace<'_, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Some(v) => v.fmt(f),
            None => f.write_str("None"),
        }
    }
}

impl<'a, T> From<&'a Option<T>> for OptionTrace<'a, T> {
    fn from(value: &'a Option<T>) -> Self {
        Self(value)
    }
}

impl<T> OptionExt for Option<T> {
    type Value = T;

    fn trace(&self) -> OptionTrace<'_, Self::Value> {
        self.into()
    }
}
