/// Provides a convient syntax for conduit operations.
///
/// # Example
///
/// ```
/// #[macro_use] extern crate plumbum;
/// use plumbum::*;
/// fn main() {
///     let src = pipe!{
///         produce(42);
///         produce(43);
///     };
///     let sink = pipe!{
///         for x = consume();
///         for y = consume();
///         return x.unwrap_or(0) + y.unwrap_or(0)
///     };
///     assert_eq!(src.connect(sink), 85);
/// }
#[macro_export]
macro_rules! pipe {

    (let $p: pat = $e: expr ; $( $t: tt )*) => (
        { let $p = $e ; pipe! { $( $t )* } }
    );

    (let $p: ident : $ty: ty = $e: expr ; $( $t: tt )*) => (
        { let $p: $ty = $e ; pipe! { $( $t )* } }
    );

    (for $p: pat = $e: expr ; $( $t: tt )*) => (
        $crate::ConduitM::and_then($e, move |$p| pipe! { $( $t )* } )
    );

    (for $p: ident : $ty: ty = $e: expr ; $( $t: tt )*) => (
        $crate::ConduitM::and_then($e, move |$p : $ty| pipe! { $( $t )* } )
    );

    ($e: expr ; $( $t: tt )*) => (
        $crate::ConduitM::and_then($e, move |_| pipe! { $( $t )* } )
    );

    (return $e: expr) => (From::from($e));

    ($e: expr) => ($e);

    () => (From::from(()));

}
