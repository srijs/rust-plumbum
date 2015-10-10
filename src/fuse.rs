/// Provides a convient syntax for fusing conduits.
///
/// # Example
///
/// ```
/// #[macro_use] extern crate plumbum;
/// use plumbum::*;
/// fn main() {
///     use std::iter::FromIterator;
///     use plumbum::{Conduit, Source, Sink};
///
///     let src = fuse!{
///         Source::from_iter(vec![42, 43]),
///         Conduit::transform(|x| 1 + x),
///         Conduit::transform(|x| 2 * x)
///     };
///     let sink = Sink::fold(0, |x, y| x + y);
///
///     assert_eq!(src.connect(sink), 174);
/// }
#[macro_export]
macro_rules! fuse {

    ( $x:expr , $( $y:expr ) , * ) => {
        {
            let mut src = $x;
            $(
                src = $crate::ConduitM::fuse(src, $y);
            )*
            src
        }
    };

}
