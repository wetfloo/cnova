mod discard;
mod inspect;

use inspect::{InspectError, InspectOk};

use crate::iter::discard::{DiscardError, DiscardOk};

pub trait IterExt: Iterator {
	/// Allows you to inspect any [Result::Err]'s contents without modifying the iterator.
	fn inspect_err<T, E, F>(self, inspect: F) -> InspectError<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E);

	fn inspect_ok<T, E, F>(self, inspect: F) -> InspectOk<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&T);

	/// Drops any [Result::Ok] passing along only [Result::Err] inner values.
	fn discard_ok<T, E>(self) -> DiscardOk<Self>
	where
		Self: Iterator<Item = Result<T, E>> + Sized;

	/// Drops any [Result::Err] passing along only [Result::Ok] inner values.
	fn discard_err<T, E>(self) -> DiscardError<Self>
	where
		Self: Iterator<Item = Result<T, E>> + Sized;
}

impl<I> IterExt for I
where
	I: Iterator,
{
	fn inspect_ok<T, E, F>(self, inspect: F) -> InspectOk<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&T),
	{
		InspectOk::new(self, inspect)
	}

	fn inspect_err<T, E, F>(self, inspect: F) -> InspectError<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E),
	{
		InspectError::new(self, inspect)
	}

	fn discard_ok<T, E>(self) -> DiscardOk<Self>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		DiscardOk::new(self)
	}

	fn discard_err<T, E>(self) -> DiscardError<Self>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		DiscardError::new(self)
	}
}
