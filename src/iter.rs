use std::iter::FusedIterator;
use std::mem;

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

struct InspectSpecialCase<N, F> {
	inner_iter: N,
	/// Function that will be called on every [Iterator::next] call.
	/// Should only be called if deemed appropriate.
	f: F,
}

trait InspectSpecialCaseFn<T> {
	fn call(&mut self, val: &T);
}

impl<N, F> Iterator for InspectSpecialCase<N, F>
where
	N: Iterator,
	F: InspectSpecialCaseFn<N::Item>,
{
	type Item = N::Item;

	fn next(&mut self) -> Option<Self::Item> {
		self.inner_iter
			.next()
			.inspect(|val| self.f.call(val))
	}
}

impl<T, E, F> InspectSpecialCaseFn<Result<T, E>> for InspectSpecialCaseFnError<F>
where
	F: FnMut(&E),
{
	fn call(&mut self, val: &Result<T, E>) {
		if let Err(e) = val.as_ref() {
			self.0(e)
		}
	}
}

struct InspectSpecialCaseFnOk<F>(F);

impl<T, E, F> InspectSpecialCaseFn<Result<T, E>> for InspectSpecialCaseFnOk<F>
where
	F: FnMut(&T),
{
	fn call(&mut self, val: &Result<T, E>) {
		if let Ok(v) = val.as_ref() {
			self.0(v)
		}
	}
}

type InspectOk<N, F> = InspectSpecialCase<N, InspectSpecialCaseFnOk<F>>;
type InspectError<N, F> = InspectSpecialCase<N, InspectSpecialCaseFnError<F>>;

struct InspectSpecialCaseFnError<F>(F);

impl<I> IterExt for I
where
	I: Iterator,
{
	fn inspect_err<T, E, F>(self, inspect: F) -> InspectError<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E),
	{
		InspectError {
			inner_iter: self,
			f: InspectSpecialCaseFnError(inspect),
		}
	}

	fn inspect_ok<T, E, F>(self, inspect: F) -> InspectOk<Self, F>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&T),
	{
		InspectOk {
			inner_iter: self,
			f: InspectSpecialCaseFnOk(inspect),
		}
	}

	fn discard_err<T, E>(self) -> DiscardError<impl Iterator<Item = T>>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		new_discard_err(self)
	}
}

impl<N, T, E, F> DoubleEndedIterator for InspectSpecialCase<N, F>
where
	N: DoubleEndedIterator<Item = Result<T, E>>,
	F: InspectSpecialCaseFn<N::Item>,
{
	fn next_back(&mut self) -> Option<Self::Item> {
		self.inner_iter
			.next_back()
			.inspect(|val| self.f.call(val))
	}
}

impl<N, T, E, F> ExactSizeIterator for InspectSpecialCase<N, F>
where
	N: ExactSizeIterator<Item = Result<T, E>>,
	F: InspectSpecialCaseFn<N::Item>,
{
	fn len(&self) -> usize {
		self.inner_iter.len()
	}
}

impl<N, T, E, F> FusedIterator for InspectError<N, F>
where
	N: FusedIterator<Item = Result<T, E>>,
	F: FnMut(&E),
{
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
