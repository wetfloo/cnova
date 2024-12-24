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
