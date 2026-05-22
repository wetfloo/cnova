use std::iter::FusedIterator;
use std::mem;

struct InspectSpecialCase<N, F> {
	inner_iter: N,
	/// Function that will be called on every [Iterator::next] call.
	f: F,
}

trait InspectSpecialCaseFn<T> {
	fn call(&mut self, val: &T);
}

impl<'a, N, F> Iterator for InspectSpecialCase<N, F>
where
	N: Iterator,
	F: InspectSpecialCaseFn<N::Item>,
{
	type Item = N::Item;

	fn next(&mut self) -> Option<Self::Item> {
		self.inner_iter.next().inspect(|val| {
			self.f.call(val);
		})
	}
}

pub trait IterExt: Iterator {
	/// Allows you to inspect any [Result::Err]'s contents without modifying the iterator.
	fn inspect_err<T, E, F>(self, inspect: F) -> InspectError<impl Iterator<Item = Self::Item>>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E);

	/// Drops any [Result::Err] passing along only [Result::Ok] inner values.
	fn discard_err<T, E>(self) -> DiscardError<impl Iterator<Item = T>>
	where
		Self: Iterator<Item = Result<T, E>> + Sized;
}

pub struct InspectError<N> {
	inner_iter: N,
}

fn new_inspect_err<I, F, T, E>(
	iter: I,
	mut inspect: F,
) -> InspectError<impl Iterator<Item = I::Item>>
where
	I: Iterator<Item = Result<T, E>>,
	F: FnMut(&E),
{
	InspectError {
		inner_iter: iter.inspect(move |res| {
			if let Err(e) = res {
				inspect(e)
			}
		}),
	}
}

impl<N> Iterator for InspectError<N>
where
	N: Iterator,
{
	type Item = N::Item;

	fn next(&mut self) -> Option<Self::Item> {
		self.inner_iter.next()
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner_iter.size_hint()
	}
}

impl<I> IterExt for I
where
	I: Iterator,
{
	fn inspect_err<T, E, F>(self, inspect: F) -> InspectError<impl Iterator<Item = Self::Item>>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E),
	{
		new_inspect_err(self, inspect)
	}

	fn discard_err<T, E>(self) -> DiscardError<impl Iterator<Item = T>>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		new_discard_err(self)
	}
}

impl<N> DoubleEndedIterator for InspectError<N>
where
	N: DoubleEndedIterator,
{
	fn next_back(&mut self) -> Option<Self::Item> {
		self.inner_iter.next_back()
	}
}

impl<N> ExactSizeIterator for InspectError<N>
where
	N: ExactSizeIterator,
{
	fn len(&self) -> usize {
		self.inner_iter.len()
	}
}

impl<N> FusedIterator for InspectError<N> where N: FusedIterator {}

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
