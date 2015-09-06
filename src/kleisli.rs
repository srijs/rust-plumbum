use std::mem::transmute;
use std::marker::PhantomData;
use std::collections::VecDeque;

use super::instr::Instr;
use super::Program;

/// The Kleisli arrow from `A` to `Program<I, B>`.
pub struct Kleisli<'a, I: Instr<Return=A>, A, B> {
    phan: PhantomData<(A, B)>,
    deque: VecDeque<Box<Fn(Box<()>) -> Program<'a, I, ()> + 'a>>
}

unsafe fn fn_transmute<'a, I: Instr, A, B, F: 'a + Fn(Box<A>) -> Program<'a, I, B>>(f: F)
    -> Box<Fn(Box<()>) -> Program<'a, I, ()> + 'a> {
    Box::new(move |ptr| transmute(f(transmute::<Box<()>, Box<A>>(ptr))))
}

impl<'a, I: Instr<Return=A>, A> Kleisli<'a, I, A, A> {
    /// Creates the identity arrow.
    ///
    /// # Example
    ///
    /// ```rust
    /// use operational::{Kleisli, Program};
    /// use operational::instr::Identity;
    ///
    /// let k: Kleisli<Identity<_>, _, _> = Kleisli::new();
    /// assert_eq!(k.run(42), Program::new(42));
    /// ```
    pub fn new() -> Kleisli<'a, I, A, A> {
        Kleisli { phan: PhantomData, deque: VecDeque::new() }
    }
}

pub fn append_boxed<'a, I: 'a + Instr<Return=A>, A, B, C, F>
    (mut k: Kleisli<'a, I, A, B>, f: F) -> Kleisli<'a, I, A, C>
    where F: 'a + Fn(Box<B>) -> Program<'a, I, C> {
    k.deque.push_back(unsafe { fn_transmute(f) });
    Kleisli { phan: PhantomData, deque: k.deque }
}

impl<'a, I: 'a + Instr<Return=A>, A, B> Kleisli<'a, I, A, B> {

    /// Appends the given function to the tail of the arrow.
    /// This corresponds to closure composition at the codomain (post-composition).
    ///
    /// # Example
    ///
    /// ```rust
    /// use operational::{Kleisli, Program};
    /// use operational::instr::Identity;
    ///
    /// let k: Kleisli<Identity<_>, _, _> = Kleisli::new()
    ///     .append(|x| Program::new(x + 1));
    /// assert_eq!(k.run(42), Program::new(43));
    /// ```
    pub fn append<F, C>(self, f: F) -> Kleisli<'a, I, A, C>
        where F: 'a + Fn(B) -> Program<'a, I, C> {
        append_boxed(self, move |b| f(*b))
    }

    /// Given an input, runs the arrow to completion and return
    /// the resulting program.
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

#[test]
fn kleisli_run_to_string() {
    use super::instr::Identity;
    let k: Kleisli<Identity<i32>, _, _> = Kleisli::new().append(|a: i32| Program::new(a.to_string()));
    assert_eq!(k.run(42), Program::new("42".to_string()));
}
