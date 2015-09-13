/// Provides a convient syntax for monadic computations.
///
/// # Example
///
/// ```
/// #[macro_use] extern crate operational;
/// use operational::point;
/// use operational::instr::identity;
/// fn main() {
///     let l = seq!{
///         for x = point(42);
///         for _ = point(0);
///         let y = x + 5;
///         point(y + 5)
///     };
///     assert_eq!(l, identity(52));
/// }
#[macro_export]
macro_rules! seq {

    (let $p: pat = $e: expr ; $( $t: tt )*) => (
        { let $p = $e ; seq! { $( $t )* } }
    );

    (let $p: ident : $ty: ty = $e: expr ; $( $t: tt )*) => (
        { let $p: $ty = $e ; seq! { $( $t )* } }
    );

    (for $p: pat = $e: expr ; $( $t: tt )*) => (
        $e.and_then(move |$p| seq! { $( $t )* } )
    );

    (for $p: ident : $ty: ty = $e: expr ; $( $t: tt )*) => (
        $e.and_then(move |$p : $ty| seq! { $( $t )* } )
    );

    ($f: expr) => ($f)

}
