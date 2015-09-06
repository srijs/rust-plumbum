use std::mem::transmute;
use std::marker::PhantomData;
use std::collections::VecDeque;

use super::instr::Instr;
use super::Operational;

pub struct Kleisli<I: Instr<Param=A>, A, B> {
    phan: PhantomData<(A, B)>,
    deque: VecDeque<Box<Fn(Box<()>) -> Operational<I, ()> + 'static>>
}

unsafe fn fn_transmute<I: Instr, A, B, F: 'static + Fn(Box<A>) -> Operational<I, B>>(f: F)
    -> Box<Fn(Box<()>) -> Operational<I, ()>> {
    Box::new(move |ptr| {
        let a_box = transmute::<Box<()>, Box<A>>(ptr);
        transmute(f(a_box))
    })
}

impl<I: Instr<Param=A>, A> Kleisli<I, A, A> {
    pub fn new() -> Kleisli<I, A, A> {
        Kleisli { phan: PhantomData, deque: VecDeque::new() }
    }
}

impl<I: 'static + Instr<Param=A>, A, B> Kleisli<I, A, B> {

    pub fn append<F, C>(mut self, f: F) -> Kleisli<I, A, C>
        where F: 'static + Fn(Box<B>) -> Operational<I, C> {
        let g = unsafe { fn_transmute(f) };
        (&mut self.deque).push_back(g);
        Kleisli { phan: PhantomData, deque: self.deque }
    }

    pub fn run(mut self, a: A) -> Operational<I, B> {
        unsafe {
            let mut r: Operational<I, ()> = transmute::<Operational<I, A>, _>(Operational::new(a));
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
    let k: Kleisli<Identity<i32>, _, _> = Kleisli::new().append(|a| Operational::new(*a + 1));
    if let Operational::Pure(x) = k.run(42) {
        assert_eq!(*x, 43);
    }
}
