use std::iter::FusedIterator;

pub(crate) trait IterExt: Iterator {
	fn inspect_err<T, E, F>(self, inspect: F) -> InspectError<impl Iterator<Item = Self::Item>>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E);

	fn discard_err<T, E>(self) -> impl Iterator<Item = T>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
	{
		self.filter_map(|res| res.ok())
	}
}

pub(crate) struct InspectError<N> {
	inner_iter: N,
}

impl<N> InspectError<N> {
	fn new<I, F>(iter: I, f: F) -> InspectError<impl Iterator<Item = I::Item>>
	where
		I: Iterator,
		F: FnMut(&I::Item),
	{
		InspectError {
			inner_iter: iter.inspect(f),
		}
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
	fn inspect_err<T, E, F>(self, mut inspect: F) -> InspectError<impl Iterator<Item = Self::Item>>
	where
		Self: Iterator<Item = Result<T, E>> + Sized,
		F: FnMut(&E),
	{
		InspectError {
			inner_iter: self.inspect(move |res| {
				if let Err(err) = res {
					inspect(err)
				}
			}),
		}
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

pub(crate) struct DiscardError<N> {
	inner_iter: N,
}

impl<N> DiscardError<N> {
	fn new<I, T, E>(iter: I) -> DiscardError<impl Iterator<Item = T>>
	where
		I: Iterator<Item = Result<T, E>>,
	{
		DiscardError {
			inner_iter: iter.filter_map(|res| res.ok()),
		}
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
