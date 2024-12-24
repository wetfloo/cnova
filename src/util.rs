use core::fmt;

// TODO (tracing): delete this
pub trait TraceLog {
    fn trace_log(&self);
}

// TODO (tracing): delete this
pub trait TraceErr {
    fn trace_err(self) -> Self;
}

pub trait ResultExt {
    type T;
    type E;

    fn trace(&self) -> ResultTrace<Self::T, Self::E>;
}

pub trait OptionExt {
    type Value;

    fn trace(&self) -> OptionTrace<Self::Value>;
}

// TODO (tracing): delete this
impl<T, E> TraceErr for Result<T, E>
where
    E: TraceLog,
{
    fn trace_err(self) -> Self {
        self.inspect_err(|e| e.trace_log())
    }
}

pub struct ResultTrace<'a, T, E>(&'a Result<T, E>);

impl<T, E> fmt::Display for ResultTrace<'_, T, E>
where
    T: fmt::Display,
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Ok(v) => v.fmt(f),
            Err(e) => write!(f, "Err({})", e),
        }
    }
}

impl<'a, T, E> From<&'a Result<T, E>> for ResultTrace<'a, T, E> {
    fn from(value: &'a Result<T, E>) -> Self {
        Self(value)
    }
}

impl<T, E> ResultExt for Result<T, E> {
    type T = T;
    type E = E;

    fn trace(&self) -> ResultTrace<'_, Self::T, Self::E> {
        self.into()
    }
}

pub struct OptionTrace<'a, T>(&'a Option<T>);

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
