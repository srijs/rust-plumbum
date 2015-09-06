use std::mem::transmute;
use std::marker::PhantomData;
use std::collections::VecDeque;

use super::instr::Instr;
use super::Program;

pub struct Kleisli<I: Instr<Param=A>, A, B> {
    phan: PhantomData<(A, B)>,
    deque: VecDeque<Box<Fn(Box<()>) -> Program<I, ()> + 'static>>
}

unsafe fn fn_transmute<I: Instr, A, B, F: 'static + Fn(Box<A>) -> Program<I, B>>(f: F)
    -> Box<Fn(Box<()>) -> Program<I, ()>> {
    Box::new(move |ptr| transmute(f(transmute::<Box<()>, Box<A>>(ptr))))
}

impl<I: Instr<Param=A>, A> Kleisli<I, A, A> {
    pub fn new() -> Kleisli<I, A, A> {
        Kleisli { phan: PhantomData, deque: VecDeque::new() }
    }
}

pub fn append_boxed<I: 'static + Instr<Param=A>, A, B, C, F>
    (mut k: Kleisli<I, A, B>, f: F) -> Kleisli<I, A, C>
    where F: 'static + Fn(Box<B>) -> Program<I, C> {
    k.deque.push_back(unsafe { fn_transmute(f) });
    Kleisli { phan: PhantomData, deque: k.deque }
}

impl<I: 'static + Instr<Param=A>, A, B> Kleisli<I, A, B> {

    pub fn append<F, C>(self, f: F) -> Kleisli<I, A, C>
        where F: 'static + Fn(B) -> Program<I, C> {
        append_boxed(self, move |b| f(*b))
    }

    pub fn run(mut self, a: A) -> Program<I, B> {
        unsafe {
            let mut r = transmute::<Program<I, A>, Program<I, ()>>(Program::new(a));
            loop {
                match self.deque.pop_front() {
                    None => return transmute(r),
                    Some(f) => r = r.and_then_boxed(move |a| (*f)(a))
                }
            }
        }
    }

}

#[test]
fn kleisli_run_plus_one() {
    use super::instr::Identity;
    let k: Kleisli<Identity<i32>, _, _> = Kleisli::new().append(|a| Program::new(a + 1));
    assert_eq!(k.run(42), Program::new(43));
}
