pub mod instr;
use instr::Instr;

mod kleisli;
pub use kleisli::Kleisli;

pub enum Program<I: Instr, A> {
    Pure(Box<A>),
    Then(Box<I>, Kleisli<I, I::Param, A>)
}

impl<I: 'static + Instr, A> Program<I, A> {

    pub fn new(a: A) -> Program<I, A> {
        Program::Pure(Box::new(a))
    }

    fn and_then_boxed<B, F>(self, js: F) -> Program<I, B>
        where F: 'static + Fn(Box<A>) -> Program<I, B> {
        match self {
            Program::Pure(a) => js(a),
            Program::Then(i, is) => Program::Then(i, kleisli::append_boxed(is, js))
        }
    }

    pub fn and_then<B, F>(self, js: F) -> Program<I, B>
        where F: 'static + Fn(A) -> Program<I, B> {
        match self {
            Program::Pure(a) => js(*a),
            Program::Then(i, is) => Program::Then(i, is.append(js))
        }
    }

}
