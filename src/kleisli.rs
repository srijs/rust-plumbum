use std::mem::transmute;
use std::marker::PhantomData;
use std::collections::VecDeque;

use super::instr::Instr;
use super::Program;

pub struct Kleisli<'a, I: Instr<Param=A>, A, B> {
    phan: PhantomData<(A, B)>,
    deque: VecDeque<Box<Fn(Box<()>) -> Program<'a, I, ()> + 'a>>
}

unsafe fn fn_transmute<'a, I: Instr, A, B, F: 'a + Fn(Box<A>) -> Program<'a, I, B>>(f: F)
    -> Box<Fn(Box<()>) -> Program<'a, I, ()> + 'a> {
    Box::new(move |ptr| transmute(f(transmute::<Box<()>, Box<A>>(ptr))))
}

impl<'a, I: Instr<Param=A>, A> Kleisli<'a, I, A, A> {
    pub fn new() -> Kleisli<'a, I, A, A> {
        Kleisli { phan: PhantomData, deque: VecDeque::new() }
    }
}

pub fn append_boxed<'a, I: 'a + Instr<Param=A>, A, B, C, F>
    (mut k: Kleisli<'a, I, A, B>, f: F) -> Kleisli<'a, I, A, C>
    where F: 'a + Fn(Box<B>) -> Program<'a, I, C> {
    k.deque.push_back(unsafe { fn_transmute(f) });
    Kleisli { phan: PhantomData, deque: k.deque }
}

impl<'a, I: 'a + Instr<Param=A>, A, B> Kleisli<'a, I, A, B> {

    pub fn append<F, C>(self, f: F) -> Kleisli<'a, I, A, C>
        where F: 'a + Fn(B) -> Program<'a, I, C> {
        append_boxed(self, move |b| f(*b))
    }

    pub fn run(mut self, a: A) -> Program<'a, I, B> {
        unsafe {
            let mut r = transmute::<Program<'a, I, A>, Program<'a, I, ()>>(Program::new(a));
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
