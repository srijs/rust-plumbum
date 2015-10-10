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
///         Conduit::transform(|x| 2 * x),
///         Conduit::transform(|x: i32| x.to_string())
///     };
///     let sink = Sink::fold(Vec::new(), |mut v, x| {
///         v.push(x);
///         return v
///     });
///
///     assert_eq!(src.connect(sink), vec!["86","88"]);
/// }
#[macro_export]
macro_rules! fuse {

    ( $x:expr ) => {
        $x
    };

    ( $x:expr , $( $t:tt )* ) => {
        $crate::ConduitM::fuse($x, fuse!{ $( $t )* });
    };

}
