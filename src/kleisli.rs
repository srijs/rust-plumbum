use std::mem::transmute;
use std::marker::PhantomData;
use std::collections::VecDeque;

use super::ConduitM;

/// The Kleisli arrow from `A` to `ConduitM<I, O, B>`.
pub struct Kleisli<'a, A, I, O, B> {
    phan: PhantomData<(A, B)>,
    deque: VecDeque<Box<Fn(Box<()>) -> ConduitM<'a, I, O, ()> + 'a>>
}

unsafe fn fn_transmute<'a, I, O, A, B, F: 'a + Fn(Box<A>) -> ConduitM<'a, I, O, B>>(f: F)
    -> Box<Fn(Box<()>) -> ConduitM<'a, I, O, ()> + 'a> {
    Box::new(move |ptr| transmute(f(transmute::<Box<()>, Box<A>>(ptr))))
}

impl<'a, I, O, A> Kleisli<'a, A, I, O, A> {
    /// Creates the identity arrow.
    ///
    /// # Example
    ///
    /// ```rust
    /// use plumbum::Kleisli;
    ///
    /// let k: Kleisli<i32, (), (), i32> = Kleisli::new();
    /// assert_eq!(k.run(42), 42.into());
    /// ```
    pub fn new() -> Kleisli<'a, A, I, O, A> {
        Kleisli { phan: PhantomData, deque: VecDeque::new() }
    }
}

pub fn append_boxed<'a, I, O, A, B, C, F>
    (mut k: Kleisli<'a, A, I, O, B>, f: F) -> Kleisli<'a, A, I, O, C>
    where F: 'a + Fn(Box<B>) -> ConduitM<'a, I, O, C> {
    k.deque.push_back(unsafe { fn_transmute(f) });
    Kleisli { phan: PhantomData, deque: k.deque }
}

impl<'a, I, O, A, B> Kleisli<'a, A, I, O, B> {

    /// Appends the given function to the tail of the arrow.
    /// This corresponds to closure composition at the codomain (post-composition).
    ///
    /// # Example
    ///
    /// ```rust
    /// use plumbum::Kleisli;
    ///
    /// let k: Kleisli<i32, (), (), i32> = Kleisli::new().append(|x: i32| (x + 1).into());
    /// assert_eq!(k.run(42), 43.into());
    /// ```
    pub fn append<F, C>(self, f: F) -> Kleisli<'a, A, I, O, C>
        where F: 'a + Fn(B) -> ConduitM<'a, I, O, C> {
        append_boxed(self, move |b| f(*b))
    }

    /// Given an input, runs the arrow to completion and return
    /// the resulting program.
    pub fn run(mut self, a: A) -> ConduitM<'a, I, O, B> {
        unsafe {
            let mut r = transmute::<ConduitM<'a, I, O, A>, ConduitM<'a, I, O, ()>>(a.into());
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
    let k: Kleisli<i32, (), (), i32> = Kleisli::new().append(|a: i32| (a + 1).into());
    assert_eq!(k.run(42), 43.into());
}

#[test]
fn kleisli_run_to_string() {
    let k: Kleisli<i32, (), (), String> =
        Kleisli::new().append(|a: i32| (a.to_string()).into());
    assert_eq!(k.run(42), "42".to_string().into());
}
