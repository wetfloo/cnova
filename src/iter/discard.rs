use std::iter::FusedIterator;

pub type DiscardOk<N> = DiscardSpecialCase<N, DiscardSpecialCaseFnOk>;
pub type DiscardError<N> = DiscardSpecialCase<N, DiscardSpecialCaseFnError>;

pub struct DiscardSpecialCase<N, F> {
	inner_iter: N,
	f: F,
}

impl<N> DiscardSpecialCase<N, DiscardSpecialCaseFnOk> {
	pub(crate) fn new<T, E>(inner_iter: N) -> Self
	where
		N: Iterator<Item = Result<T, E>>,
	{
		Self {
			inner_iter,
			f: DiscardSpecialCaseFnOk,
		}
	}
}

impl<N> DiscardSpecialCase<N, DiscardSpecialCaseFnError> {
	pub(crate) fn new<T, E>(inner_iter: N) -> Self
	where
		N: Iterator<Item = Result<T, E>>,
	{
		Self {
			inner_iter,
			f: DiscardSpecialCaseFnError,
		}
	}
}

impl<N, F> Iterator for DiscardSpecialCase<N, F>
where
	N: Iterator,
	F: DiscardSpecialCaseFn<N::Item>,
{
	type Item = F::Out;

	fn next(&mut self) -> Option<Self::Item> {
		self.inner_iter
			.next()
			.and_then(|val| self.f.call(val))
	}
}

impl<N, T, E, F> DoubleEndedIterator for DiscardSpecialCase<N, F>
where
	N: DoubleEndedIterator<Item = Result<T, E>>,
	F: DiscardSpecialCaseFn<N::Item>,
{
	fn next_back(&mut self) -> Option<Self::Item> {
		self.inner_iter
			.next_back()
			.and_then(|val| self.f.call(val))
	}
}

impl<N, T, E, F> ExactSizeIterator for DiscardSpecialCase<N, F>
where
	N: ExactSizeIterator<Item = Result<T, E>>,
	F: DiscardSpecialCaseFn<N::Item>,
{
	fn len(&self) -> usize {
		self.inner_iter.len()
	}
}

impl<N, T, E> FusedIterator for DiscardError<N> where N: FusedIterator<Item = Result<T, E>> {}

pub trait DiscardSpecialCaseFn<T> {
	type Out;

	fn call(&mut self, val: T) -> Option<Self::Out>;
}

pub struct DiscardSpecialCaseFnOk;

impl<T, E> DiscardSpecialCaseFn<Result<T, E>> for DiscardSpecialCaseFnOk {
	type Out = E;

	fn call(&mut self, val: Result<T, E>) -> Option<Self::Out> {
		val.err()
	}
}

pub struct DiscardSpecialCaseFnError;

impl<T, E> DiscardSpecialCaseFn<Result<T, E>> for DiscardSpecialCaseFnError {
	type Out = T;

	fn call(&mut self, val: Result<T, E>) -> Option<Self::Out> {
		val.ok()
	}
}
