pub trait Instr {
    type Param;
}

pub struct Identity<A>(pub A);

impl<A> Instr for Identity<A> {
    type Param = A;
}

pub enum Coproduct<I, J> {
    Left(I),
    Right(J)
}

impl<A, I: Instr<Param=A>, J: Instr<Param=A>> Instr for Coproduct<I, J> {
    type Param = A;
}
