use std::fmt;

pub mod instr;
use instr::Instr;

mod kleisli;
pub use kleisli::Kleisli;

pub enum Program<'a, I: Instr, A> {
    Pure(Box<A>),
    Then(Box<I>, Kleisli<'a, I, I::Param, A>)
}

impl<'a, I: 'a + Instr, A> Program<'a, I, A> {

    pub fn new(a: A) -> Program<'a, I, A> {
        Program::Pure(Box::new(a))
    }

    fn and_then_boxed<B, F>(self, js: F) -> Program<'a, I, B>
        where F: 'a + Fn(Box<A>) -> Program<'a, I, B> {
        match self {
            Program::Pure(a) => js(a),
            Program::Then(i, is) => Program::Then(i, kleisli::append_boxed(is, js))
        }
    }

    pub fn and_then<B, F>(self, js: F) -> Program<'a, I, B>
        where F: 'a + Fn(A) -> Program<'a, I, B> {
        match self {
            Program::Pure(a) => js(*a),
            Program::Then(i, is) => Program::Then(i, is.append(js))
        }
    }

}

impl<'a, I: 'a + Instr, A: PartialEq> PartialEq for Program<'a, I, A> {
    fn eq(&self, other: &Program<'a, I, A>) -> bool {
        match (self, other) {
            (&Program::Pure(ref a), &Program::Pure(ref b)) => a == b,
            _ => false
        }
    }
}

impl<'a, I: 'a + Instr, A: fmt::Debug> fmt::Debug for Program<'a, I, A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Program::Pure(ref a) => write!(f, "Pure({:?})", a),
            &Program::Then(_, _) => write!(f, "Then(..)")
        }
    }
}
