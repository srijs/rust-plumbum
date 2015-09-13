use super::{Program, point};
use super::Kleisli;

/// The trait for an instruction set.
///
/// Holds its return type as an associated type.
pub trait Instr {
    type Return;

    /// Lift the instruction into a program.
    fn to_program<'a>(self) -> Program<'a, Self, Self::Return> where Self: Sized {
        Program::Then(Box::new(self), Kleisli::new())
    }

}

/// The instruction that does nothing.
pub struct Identity<A>(pub A);

impl<A> Instr for Identity<A> {
    type Return = A;
}

pub fn identity<'a, A: 'a>(a: A) -> Program<'a, Identity<A>, A> {
    point(a)
}

/// Combines two instruction sets into one.
///
/// This is useful to compose multiple smaller
/// instruction sets into a bigger one.
pub enum Coproduct<I, J> {
    Left(I),
    Right(J)
}

impl<A, I: Instr<Return=A>, J: Instr<Return=A>> Instr for Coproduct<I, J> {
    type Return = A;
}
