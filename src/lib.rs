pub mod instr;
use instr::Instr;

mod kleisli;
pub use kleisli::Kleisli;

pub enum Operational<I: Instr, A> {
    Pure(Box<A>),
    Then(Box<I>, Kleisli<I, I::Param, A>)
}

impl<I: 'static + Instr, A> Operational<I, A> {

    pub fn new(a: A) -> Operational<I, A> {
        Operational::Pure(Box::new(a))
    }

    fn and_then_boxed<B, F>(self, js: F) -> Operational<I, B>
        where F: 'static + Fn(Box<A>) -> Operational<I, B> {
        match self {
            Operational::Pure(a) => js(a),
            Operational::Then(i, is) => Operational::Then(i, kleisli::append_boxed(is, js))
        }
    }

    pub fn and_then<B, F>(self, js: F) -> Operational<I, B>
        where F: 'static + Fn(A) -> Operational<I, B> {
        match self {
            Operational::Pure(a) => js(*a),
            Operational::Then(i, is) => Operational::Then(i, is.append(js))
        }
    }

}

#[test]
fn it_works() {
}
