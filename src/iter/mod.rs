mod inspect;

use inspect::{InspectError, InspectOk};
use std::iter::FusedIterator;

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

	/// Drops any [Result::Err] passing along only [Result::Ok] inner values.
	fn discard_err<T, E>(self) -> DiscardError<impl Iterator<Item = T>>
	where
		Self: Iterator<Item = Result<T, E>> + Sized;
}

impl<I> IterExt for I
where
	I: Iterator,
{
	fn inspect_err<T, E, F>(self, inspect: F) -> InspectError<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E),
	{
		InspectError::new(self, inspect)
	}

	fn inspect_ok<T, E, F>(self, inspect: F) -> InspectOk<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&T),
	{
		InspectOk::new(self, inspect)
	}

	fn discard_err<T, E>(self) -> DiscardError<impl Iterator<Item = T>>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		new_discard_err(self)
	}
}

pub struct DiscardError<N> {
	inner_iter: N,
}

fn new_discard_err<I, T, E>(iter: I) -> DiscardError<impl Iterator<Item = T>>
where
	I: Iterator<Item = Result<T, E>>,
{
	DiscardError {
		inner_iter: iter.filter_map(|res| res.ok()),
	}
}

impl<N> Iterator for DiscardError<N>
where
	N: Iterator,
{
	type Item = N::Item;

	fn next(&mut self) -> Option<Self::Item> {
		self.inner_iter.next()
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		// It can be empty if Results are all `Err`,
		// but it can also be full of `Ok`s.
		(0, self.inner_iter.size_hint().1)
	}
}

impl<N> DoubleEndedIterator for DiscardError<N>
where
	N: DoubleEndedIterator,
{
	fn next_back(&mut self) -> Option<Self::Item> {
		self.inner_iter.next_back()
	}
}

impl<N> FusedIterator for DiscardError<N> where N: FusedIterator {}
