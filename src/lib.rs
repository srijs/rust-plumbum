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

mod kleisli;
pub use kleisli::Kleisli;

mod pipe;
pub use pipe::*;

enum Void {}

/// Represents a conduit, i.e. a sequence of await/yield actions.
///
/// - `A` is the return type of the conduit.
pub enum ConduitM<'a, I, O, A> {
    /// The case `Pure(a)` means that the conduit contains requires further action and just returns the result `a`.
    Pure(Box<A>),
    /// The case `Await(k)` means that the conduit waits for a value of type `I`, and the remaining (suspended) program is given by the kleisli arrow `k`.
    Await(Kleisli<'a, Option<I>, I, O, A>),
    /// The case `Yield(o, k)` means that the conduit yields a value of type `O`, and the remaining (suspended) program is given by the kleisli arrow `k`.
    Yield(Box<O>, Kleisli<'a, (), I, O, A>)
}

/// Provides a stream of output values,
/// without consuming any input or producing a final result.
pub type Source<'a, O> = ConduitM<'a, (), O, ()>;

/// Consumes a stream of input values and produces a stream of output values,
/// without producing a final result.
pub type Conduit<'a, I, O> = ConduitM<'a, I, O, ()>;

/// Consumes a stream of input values and produces a final result,
/// without producing any output.
pub type Sink<'a, I, A> = ConduitM<'a, I, Void, A>;

impl<'a, I, O, A> ConduitM<'a, I, O, A> {

    fn and_then_boxed<B, F>(self, js: F) -> ConduitM<'a, I, O, B>
        where F: 'a + Fn(Box<A>) -> ConduitM<'a, I, O, B> {
        match self {
            ConduitM::Pure(a) => js(a),
            ConduitM::Await(is) => ConduitM::Await(kleisli::append_boxed(is, js)),
            ConduitM::Yield(o, is) => ConduitM::Yield(o, kleisli::append_boxed(is, js))
        }
    }

    /// Appends a continuation to a conduit. Which means,
    /// given a function from `A` to `ConduitM<I, O, B>`,
    /// passes the return value of the conduit to the function,
    /// and returns the resulting program.
    ///
    /// Equivalent to the monadic `>>=` operator.
    pub fn and_then<B, F>(self, js: F) -> ConduitM<'a, I, O, B>
        where F: 'a + Fn(A) -> ConduitM<'a, I, O, B> {
        match self {
            ConduitM::Pure(a) => js(*a),
            ConduitM::Await(is) => ConduitM::Await(is.append(js)),
            ConduitM::Yield(o, is) => ConduitM::Yield(o, is.append(js))
        }
    }

    /// Modifies the return value of the conduit.
    /// Seen differently, it lifts a function from
    /// `A` to `B` into a function from `ConduitM<I, O, A>`
    /// to `ConduitM<I, O, B>`.
    ///
    /// Equivalent to the monadic `liftM`.
    pub fn map<B, F>(self, f: F) -> ConduitM<'a, I, O, B>
        where F: 'a + Fn(A) -> B {
        self.and_then(move |a| point(f(a)))
    }

}

impl<'a, I, O, A: PartialEq> PartialEq for ConduitM<'a, I, O, A> {
    fn eq(&self, other: &ConduitM<'a, I, O, A>) -> bool {
        match (self, other) {
            (&ConduitM::Pure(ref a), &ConduitM::Pure(ref b)) => a == b,
            _ => false
        }
    }
}

impl<'a, I, O, A: fmt::Debug> fmt::Debug for ConduitM<'a, I, O, A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &ConduitM::Pure(ref a) => write!(f, "Pure({:?})", a),
            &ConduitM::Await(_) => write!(f, "Await(..)"),
            &ConduitM::Yield(_, _) => write!(f, "Yield(..)")
        }
    }
}

/// Using a value, constructs the empty conduit,
/// i.e. a conduit that directly returns that value.
///
/// Equivalent to the monadic `return`.
pub fn point<'a, I, O, A>(a: A) -> ConduitM<'a, I, O, A> {
    ConduitM::Pure(Box::new(a))
}

/// Wait for a single input value from upstream.
///
/// If no data is available, returns `None`.
/// Once it returns `None`, subsequent calls will also return `None`.
pub fn consume<'a, I, O>() -> ConduitM<'a, I, O, Option<I>> {
    ConduitM::Await(Kleisli::new())
}

/// Send a value downstream to the next component to consume.
///
/// If the downstream component terminates, this call will never return control.
pub fn produce<'a, I, O>(o: O) -> ConduitM<'a, I, O, ()> {
    ConduitM::Yield(Box::new(o), Kleisli::new())
}

/// Pulls data from the source and pushes it into the sink.
///
/// # Example
///
/// ```rust
/// use plumbum::{Source, Sink, consume, produce, connect};
///
/// let src = produce(42);
///
/// let sink = consume().map(|x| 1 + x.unwrap_or(0));
///
/// assert_eq!(connect(src, sink), 43);
/// ```
pub fn connect<A, B>(mut src: Source<A>, mut sink: Sink<A, B>) -> B {
    loop {
        let (next_src, next_sink) = match sink {
            ConduitM::Pure(b_box) => {
                return *b_box;
            },
            ConduitM::Await(k_sink) => {
                match src {
                    ConduitM::Pure(x) => {
                        (ConduitM::Pure(x), k_sink.run(None))
                    },
                    ConduitM::Await(k_src) => {
                        (k_src.run(Some(())), ConduitM::Await(k_sink))
                    },
                    ConduitM::Yield(a_box, k_src) => {
                        (k_src.run(()), k_sink.run(Some(*a_box)))
                    }
                }
            },
            ConduitM::Yield(_, _) => unreachable!()
        };
        src = next_src;
        sink = next_sink;
    }
}
