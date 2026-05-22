mod discard;
mod inspect;

use crate::iter::discard::{DiscardError, DiscardOk};
use inspect::{InspectError, InspectOk};

pub trait IterExt: Iterator {
	/// Allows you to inspect any [Result::Ok]'s contents without modifying the iterator.
	fn inspect_ok<T, E, F>(self, inspect: F) -> InspectOk<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&T),
	{
		InspectOk::new(self, inspect)
	}

	/// Allows you to inspect any [Result::Err]'s contents without modifying the iterator.
	fn inspect_err<T, E, F>(self, inspect: F) -> InspectError<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E),
	{
		InspectError::new(self, inspect)
	}

	/// Drops any [Result::Ok] passing along only [Result::Err] inner values.
	///
	/// ```
	/// # use cnova::iter::IterExt;
	/// let results = vec![
	///	    Ok(1),
	///	    Err(2),
	///	    Ok(3),
	///	    Ok(4),
	///	    Err(5),
	///	    Err(6),
	/// ];
	///
	/// let filtered: Vec<_> = results.into_iter().discard_ok().collect();
	/// assert_eq!(vec![2, 5, 6], filtered);
	/// ```
	fn discard_ok<T, E>(self) -> DiscardOk<Self>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		DiscardOk::new(self)
	}

	/// Drops any [Result::Err] passing along only [Result::Ok] inner values.
	///
	/// ```
	/// # use cnova::iter::IterExt;
	/// let results = vec![
	///	    Ok(1),
	///	    Err(2),
	///	    Ok(3),
	///	    Ok(4),
	///	    Err(5),
	///	    Err(6),
	/// ];
	///
	/// let filtered: Vec<_> = results.into_iter().discard_err().collect();
	/// assert_eq!(vec![1, 3, 4], filtered);
	/// ```
	fn discard_err<T, E>(self) -> DiscardError<Self>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		DiscardError::new(self)
	}
}

impl<I> IterExt for I
where
	I: Iterator,
{
}
