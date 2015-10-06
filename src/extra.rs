//! A few useful utilities.

use super::{Conduit, produce};

/// Create a source that outputs no values.
pub fn source_null<'a, I, O>() -> Conduit<'a, I, O> {
    ().into()
}

/// Create a source from the values in the iterable.
pub fn source_iter<'a, I, T: IntoIterator>(iterable: T) -> Conduit<'a, I, T::Item>
    where T::Item: Clone + 'a {
    let mut conduit = source_null();
    for x in iterable {
        conduit = conduit.and_then(move |_| produce(x.clone()));
    }
    conduit
}
