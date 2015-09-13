/// Provides a convient syntax for monadic computations.
///
/// # Example
///
/// ```
/// #[macro_use] extern crate operational;
/// use operational::Program;
/// use operational::instr::Identity;
/// fn main() {
///     let l: Program<Identity<i32>, _> = seq!{
///         for x = Program::new(42);
///         let y = x + 5;
///         Program::new(y + 5)
///     };
///     assert_eq!(l, Program::new(52));
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
