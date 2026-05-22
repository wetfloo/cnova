mod discard;
mod inspect;

use crate::iter::discard::{DiscardError, DiscardOk};
use inspect::{InspectError, InspectOk};

pub trait IterExt: Iterator {
	/// Allows you to inspect any [Ok] contents without modifying the iterator.
	fn inspect_ok<T, E, F>(self, inspect: F) -> InspectOk<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&T),
	{
		InspectOk::new(self, inspect)
	}

	/// Allows you to inspect any [Err] contents without modifying the iterator.
	fn inspect_err<T, E, F>(self, inspect: F) -> InspectError<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E),
	{
		InspectError::new(self, inspect)
	}

	/// Drops any [Ok], passing along only [Err] contents.
	///
	/// ```
	/// # use cnova::iter::IterExt;
	/// let results = vec![
	///     Ok(1),
	///     Err(2),
	///     Ok(3),
	///     Ok(4),
	///     Err(5),
	///     Err(6),
	/// ];
	///
	/// let mut filtered_iter = results
	///     .into_iter()
	///     .discard_ok();
	///
	/// assert_eq!(Some(2), filtered_iter.next());
	/// assert_eq!(Some(5), filtered_iter.next());
	/// assert_eq!(Some(6), filtered_iter.next());
	/// assert_eq!(None, filtered_iter.next());
	/// ```
	fn discard_ok<T, E>(self) -> DiscardOk<Self>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		DiscardOk::new(self)
	}

	/// Drops any [Err], passing along only [Ok] contents.
	///
	/// ```
	/// # use cnova::iter::IterExt;
	/// let results = vec![
	///     Ok(1),
	///     Err(2),
	///     Ok(3),
	///     Ok(4),
	///     Err(5),
	///     Err(6),
	/// ];
	///
	/// let mut filtered_iter = results
	///     .into_iter()
	///     .discard_err();
	///
	/// assert_eq!(Some(1), filtered_iter.next());
	/// assert_eq!(Some(3), filtered_iter.next());
	/// assert_eq!(Some(4), filtered_iter.next());
	/// assert_eq!(None, filtered_iter.next());
	/// ```
	fn discard_err<T, E>(self) -> DiscardError<Self>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		DiscardError::new(self)
	}
}

impl<I> IterExt for I where I: Iterator {}
