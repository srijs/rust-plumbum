//! The operational library makes it easy to implement monads with tricky control flow.
//!
//! This is very useful for: writing web applications in a sequential style,
//! programming games with a uniform interface for human and AI players and easy replay,
//! implementing fast parser monads, designing monadic DSLs, etc.
//!
//! A thorough introduction to the ideas behind this library is given in
//! ["The Operational Monad Tutorial"](http://apfelmus.nfshost.com/articles/operational-monad.html),
//! published in [Issue 15 of the Monad.Reader](http://themonadreader.wordpress.com/2010/01/26/issue-15/).

use std::fmt;

/// Contains the `Instr` trait, and a few useful utilities for working with instructions.
pub mod instr;
use instr::Instr;

mod kleisli;
pub use kleisli::Kleisli;

mod seq;
pub use seq::*;

/// Represents a program, i.e. a sequence of instructions.
///
/// - The _instructions_ are given by the type `I`.
/// - `A` is the return type of the program.
pub enum Program<'a, I: Instr, A> {
    /// The case `Pure(a)` means that the program contains no instructions and just returns the result `a`.
    Pure(Box<A>),
    /// The case `Then(instr, k)` means that the first instruction is `instr` and the remaining program is given by the kleisli arrow `k`.
    Then(Box<I>, Kleisli<'a, I, I::Return, A>)
}

impl<'a, I: 'a + Instr, A> Program<'a, I, A> {

    /// Using a value, constructs the empty program,
    /// i.e. a program that directly returns that value.
    ///
    /// Equivalent to the monadic `return`.
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

    /// Appends a continuation to a program. Which means,
    /// given a function from `A` to `Program<I, B>`,
    /// passes the return value of the program to the function,
    /// and returns the resulting program.
    ///
    /// Equivalent to the monadic `>>=` operator.
    pub fn and_then<B, F>(self, js: F) -> Program<'a, I, B>
        where F: 'a + Fn(A) -> Program<'a, I, B> {
        match self {
            Program::Pure(a) => js(*a),
            Program::Then(i, is) => Program::Then(i, is.append(js))
        }
    }

    /// Modifies the return value of the program.
    /// Seen differently, it lifts a function from
    /// `A` to `B` into a function from `Program<I, A>`
    /// to `Program<I, B>`.
    ///
    /// Equivalent to the monadic `liftM`.
    pub fn map<B, F>(self, f: F) -> Program<'a, I, B>
        where F: 'a + Fn(A) -> B {
        self.and_then(move |a| Program::new(f(a)))
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
