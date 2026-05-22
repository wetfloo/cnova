pub trait ResultOkFrom<T1, T2, E1> {
	fn ok_from(val: Result<T1, E1>) -> Self;
}

impl<T1, T2, E1> ResultOkFrom<T1, T2, E1> for Result<T2, E1>
where
	T2: From<T1>,
{
	fn ok_from(val: Result<T1, E1>) -> Self {
		val.map(|v| v.into())
	}
}

pub trait ResultOkInto<T1, T2, E1> {
	fn ok_into(self) -> Result<T2, E1>;
}

impl<T1, T2, E1> ResultOkInto<T1, T2, E1> for Result<T1, E1>
where
	T2: From<T1>,
{
	fn ok_into(self) -> Result<T2, E1> {
		Result::ok_from(self)
	}
}

pub trait ResultErrFrom<T1, E1, E2> {
	fn err_from(val: Result<T1, E1>) -> Self;
}

impl<T1, E1, E2> ResultErrFrom<T1, E1, E2> for Result<T1, E2>
where
	E2: From<E1>,
{
	fn err_from(val: Result<T1, E1>) -> Self
	where
		E2: From<E1>,
	{
		val.map_err(|err| err.into())
	}
}

pub trait ResultErrInto<T1, E1, E2> {
	fn err_into(self) -> Result<T1, E2>;
}

impl<T1, E1, E2> ResultErrInto<T1, E1, E2> for Result<T1, E1>
where
	E2: From<E1>,
{
	fn err_into(self) -> Result<T1, E2> {
		Result::err_from(self)
	}
}

pub trait ResultBothFrom<T1, T2, E1, E2> {
	fn both_from(val: Result<T1, E1>) -> Self;
}

impl<T1, T2, E1, E2> ResultBothFrom<T1, T2, E1, E2> for Result<T2, E2>
where
	T2: From<T1>,
	E2: From<E1>,
{
	fn both_from(val: Result<T1, E1>) -> Self {
		val.map(|v| v.into())
			.map_err(|e| e.into())
	}
}

pub trait ResultBothInto<T1, T2, E1, E2> {
	fn both_into(self) -> Result<T2, E2>;
}

impl<T1, T2, E1, E2> ResultBothInto<T1, T2, E1, E2> for Result<T1, E1>
where
	T2: From<T1>,
	E2: From<E1>,
{
	fn both_into(self) -> Result<T2, E2> {
		Result::both_from(self)
	}
}
