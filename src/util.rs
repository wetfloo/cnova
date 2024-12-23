use core::fmt;

// TODO (tracing): delete this
pub trait TraceLog {
    fn trace_log(&self);
}

// TODO (tracing): delete this
pub trait TraceErr {
    fn trace_err(self) -> Self;
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
