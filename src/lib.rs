//! *Plumbum* (latin for lead) is a port of Michael Snoyman's excellent
//! [`conduit`](https://www.fpcomplete.com/user/snoyberg/library-documentation/conduit-overview)
//! library.
//!
//! It allows for production, transformation, and consumption of streams of
//! data in constant memory.
//! It can be used for processing files, dealing with network interfaces,
//! or parsing structured data in an event-driven manner.
//!
//! ## Features
//!
//! - Large and possibly infinite streams can be processed in constant memory.
//!
//! - Chunks of data are dealt with lazily, one piece at a time, instead of needing to
//!   read in the entire body at once.
//!
//! - The resulting components are pure computations, and allow us to retain
//!   composability while dealing with the imperative world of I/O.
//!
//! ## Basics
//!
//! There are three main concepts:
//!
//! 1. A `Source` will produce a stream of data values and send them downstream.
//! 2. A `Sink` will consume a stream of data values from upstream and produce a return value.
//! 3. A `Conduit` will consume a stream of values from upstream and produces a new stream to send downstream.
//!
//! In order to combine these different components, we have connecting and fusing.
//! The `connect` method will combine a `Source` and `Sink`,
//! feeding the values produced by the former into the latter, and producing a final result.
//! Fusion, on the other hand, will take two components and generate a new component.
//! For example, fusing a `Conduit` and `Sink` together into a new `Sink`,
//! will consume the same values as the original `Conduit` and produce the same result as the original `Sink`.
//!
//! ## Primitives
//!
//! There are four core primitives:
//!
//! 1. `consume` takes a single value from upstream, if available.
//! 2. `produce` sends a single value downstream.
//! 3. `leftover` puts a single value back in the upstream queue,
//!    ready to be read by the next call to `consume`.
//! 3. `defer` introduces a point of lazyiness, artifically deferring all further actions.
//!
//! ## Example
//!
//! ```
//! use plumbum::*;
//!
//! fn source<'a>() -> Source<'a, i32> {
//!     produce(1)
//!     .and(produce(2))
//!     .and(produce(3))
//!     .and(produce(4))
//! }
//!
//! fn conduit<'a>() -> Conduit<'a, i32, String> {
//!     // Get adjacent pairs from upstream
//!     consume().zip(consume()).and_then(|res| {
//!         match res {
//!             (Some(i1), Some(i2)) => {
//!                 produce(format!("({},{})", i1, i2))
//!                 .and(leftover(i2))
//!                 .and(conduit())
//!             },
//!             _ => ().into()
//!         }
//!     })
//! }
//!
//! fn sink<'a>() -> Sink<'a, String, String> {
//!     consume().and_then(|res| {
//!         match res {
//!             None => "...".to_string().into(),
//!             Some(str) => sink().and_then(move |next| {
//!                 format!("{}:{}", str, next).into()
//!             })
//!         }
//!     })
//! }
//!
//! fn main() {
//!     let res = source().fuse(conduit()).connect(sink());
//!     assert_eq!(res, "(1,2):(2,3):(3,4):...")
//! }
use std::fmt;
use std::iter::FromIterator;

/// Interfacing with `std::io`.
pub mod io;

mod kleisli;
pub use kleisli::Kleisli;

mod pipe;
pub use pipe::*;

mod fuse;
pub use fuse::*;

enum Void {}

/// Represents a conduit, i.e. a sequence of await/yield actions.
///
/// - `I` is the type of values the conduit consumes from upstream.
/// - `O` is the type of values the conduit passes downstream.
/// - `A` is the return type of the conduit.
pub enum ConduitM<'a, I, O, A> {
    /// The case `Pure(a)` means that the conduit contains no further actions and just returns the result `a`.
    Pure(Box<A>),
    /// The case `Defer(k)` means that the conduit needs another iteration to make progress,
    /// and the remaining (suspended) program is given by the kleisli arrow `k`
    Defer(Kleisli<'a, (), I, O, A>),
    /// The case `Await(k)` means that the conduit waits for a value of type `I`,
    /// and the remaining (suspended) program is given by the kleisli arrow `k`.
    Await(Kleisli<'a, Option<I>, I, O, A>),
    /// The case `Yield(o, k)` means that the conduit yields a value of type `O`,
    /// and the remaining (suspended) program is given by the kleisli arrow `k`.
    Yield(Box<O>, Kleisli<'a, (), I, O, A>),
    /// The case `Leftover(i, k)` means that the conduit has a leftover value of type `I`,
    /// and the remaining (suspended) program is given by the kleisli arrow `k`.
    Leftover(Box<I>, Kleisli<'a, (), I, O, A>)
}

/// Provides a stream of output values,
/// without consuming any input or producing a final result.
pub type Source<'a, O> = ConduitM<'a, (), O, ()>;

impl<'a, O> ConduitM<'a, (), O, ()> {

    /// Generalize a `Source` by universally quantifying the input type.
    pub fn to_producer<I>(self) -> ConduitM<'a, I, O, ()> where O: 'static {
        match self {
            ConduitM::Pure(x) => ConduitM::Pure(x),
            ConduitM::Defer(k) => ConduitM::Defer(Kleisli::new().append(move |_| {
                k.run(()).to_producer()
            })),
            ConduitM::Await(k) => ConduitM::Defer(Kleisli::new().append(move |_| {
                k.run(Some(())).to_producer()
            })),
            ConduitM::Yield(o, k) => ConduitM::Yield(o, Kleisli::new().append(move |_| {
                k.run(()).to_producer()
            })),
            ConduitM::Leftover(_, k) => ConduitM::Defer(Kleisli::new().append(move |_| {
                k.run(()).to_producer()
            }))
        }
    }

    /// Pulls data from the source and pushes it into the sink.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::iter::FromIterator;
    /// use plumbum::{Source, Sink, produce};
    ///
    /// let src = Source::from_iter(vec![42, 43]);
    /// let sink = Sink::fold(0, |x, y| x + y);
    ///
    /// assert_eq!(src.connect(sink), 85);
    /// ```
    pub fn connect<A>(mut self, mut sink: Sink<'a, O, A>) -> A where O: 'static {
        loop {
            let (next_src, next_sink) = match sink {
                ConduitM::Pure(b_box) => {
                    return *b_box;
                },
                ConduitM::Defer(k_sink) => {
                    (self, k_sink.run(()))
                },
                ConduitM::Await(k_sink) => {
                    match self {
                        ConduitM::Pure(x) => {
                            (ConduitM::Pure(x), k_sink.run(None))
                        },
                        ConduitM::Defer(k_src) => {
                            (k_src.run(()), ConduitM::Await(k_sink))
                        },
                        ConduitM::Await(k_src) => {
                            (k_src.run(Some(())), ConduitM::Await(k_sink))
                        },
                        ConduitM::Yield(a_box, k_src) => {
                            (k_src.run(()), k_sink.run(Some(*a_box)))
                        },
                        ConduitM::Leftover(_, k_src) => {
                            (k_src.run(()), ConduitM::Await(k_sink))
                        }
                    }
                },
                ConduitM::Yield(_, _) => unreachable!(),
                ConduitM::Leftover(o_box, k_sink) => {
                    (ConduitM::Yield(o_box, Kleisli::new().append(move |_| self)), k_sink.run(()))
                }
            };
            self = next_src;
            sink = next_sink;
        }
    }

}

/// Consumes a stream of input values and produces a stream of output values,
/// without producing a final result.
pub type Conduit<'a, I, O> = ConduitM<'a, I, O, ()>;

impl<'a, I, O> ConduitM<'a, I, O, ()> {

    /// Combines two conduits together into a new conduit.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::iter::FromIterator;
    /// use plumbum::{Conduit, Source, Sink};
    ///
    /// let src = Source::from_iter(vec![42, 43]);
    /// let conduit = Conduit::transform(|x| 1 + x);
    /// let sink = Sink::fold(0, |x, y| x + y);
    ///
    /// assert_eq!(src.fuse(conduit).connect(sink), 87);
    /// ```
    pub fn fuse<P, A>(self, other: ConduitM<'a, O, P, A>) -> ConduitM<'a, I, P, A>
        where I: 'static, O: 'static, P: 'static, A: 'a {
        match other {
            ConduitM::Pure(r) => ConduitM::Pure(r),
            ConduitM::Defer(k) => ConduitM::Defer(Kleisli::new().append(move |_| {
                self.fuse(k.run(()))
            })),
            ConduitM::Yield(c, k) => ConduitM::Yield(c, Kleisli::new().append(move |_| {
                self.fuse(k.run(()))
            })),
            ConduitM::Leftover(o_box, k_right) => ConduitM::Defer(Kleisli::new().append(move |_| {
                ConduitM::Yield(o_box, Kleisli::new().append(move |_| self)).fuse(k_right.run(()))
            })),
            ConduitM::Await(k_right) => match self {
                ConduitM::Pure(_) => ConduitM::fuse(().into(), k_right.run(None)),
                ConduitM::Defer(k_left) => ConduitM::Defer(Kleisli::new().append(move |_| {
                    k_left.run(()).fuse(ConduitM::Await(k_right))
                })),
                ConduitM::Yield(b, k_left) => k_left.run(()).fuse(k_right.run(Some(*b))),
                ConduitM::Leftover(i_box, k_left) => ConduitM::Leftover(i_box, Kleisli::new().append(move |_| {
                    k_left.run(()).fuse(ConduitM::Await(k_right))
                })),
                ConduitM::Await(k_left) => ConduitM::Await(Kleisli::new().append(move |a| {
                    k_left.run(a).fuse(ConduitM::Await(k_right))
                }))
            }
        }
    }

    /// Apply a transformation to all values in a stream.
    pub fn transform<F>(f: F) -> Self where F: 'a + Fn(I) -> O {
        consume().and_then(|io| match io {
            None => ().into(),
            Some(o) => produce(f(o)).and_then(move |_| Conduit::transform(f))
        })
    }

}

impl<'a, I, O: 'a> FromIterator<O> for ConduitM<'a, I, O, ()> {
    fn from_iter<T: IntoIterator<Item=O>>(iterator: T) -> Self {
        let mut conduit: Self = ().into();
        for x in iterator {
            conduit = conduit.and_then(move |_| produce(x));
        }
        conduit
    }
}

/// Consumes a stream of input values and produces a final result,
/// without producing any output.
pub type Sink<'a, I, A> = ConduitM<'a, I, Void, A>;

impl<'a, I, A> ConduitM<'a, I, Void, A> {

    /// Generalize a `Sink` by universally quantifying the output type.
    pub fn to_consumer<O>(self) -> ConduitM<'a, I, O, A> {
        unsafe { std::mem::transmute(self) }
    }

    fn sink<F>(a: A, f: F) -> Self
        where A: 'a, F: 'a + Fn(A, I) -> Result<A, A> {
        consume().and_then(|io| {
            match io {
                None => a.into(),
                Some(i) => match f(a, i) {
                    Ok(a) => Self::sink(a, f),
                    Err(a) => a.into()
                }
            }
        })
    }

    /// Fold all values from upstream into a final value.
    pub fn fold<F>(a: A, f: F) -> Self
        where A: 'a, F: 'a + Fn(A, I) -> A {
        Self::sink(a, move |a, i| Ok(f(a, i)))
    }

}

impl<'a, I, O, A> ConduitM<'a, I, O, A> {

    fn and_then_boxed<B, F>(self, js: F) -> ConduitM<'a, I, O, B>
        where F: 'a + FnOnce(Box<A>) -> ConduitM<'a, I, O, B> {
        match self {
            ConduitM::Pure(a) => js(a),
            ConduitM::Defer(is) => ConduitM::Defer(kleisli::append_boxed(is, js)),
            ConduitM::Await(is) => ConduitM::Await(kleisli::append_boxed(is, js)),
            ConduitM::Yield(o, is) => ConduitM::Yield(o, kleisli::append_boxed(is, js)),
            ConduitM::Leftover(i, is) => ConduitM::Leftover(i, kleisli::append_boxed(is, js))
        }
    }

    /// Appends a continuation to a conduit. Which means,
    /// given a function from `A` to `ConduitM<I, O, B>`,
    /// passes the return value of the conduit to the function,
    /// and returns the resulting program.
    pub fn and_then<B, F>(self, js: F) -> ConduitM<'a, I, O, B>
        where F: 'a + FnOnce(A) -> ConduitM<'a, I, O, B> {
        match self {
            ConduitM::Pure(a) => js(*a),
            ConduitM::Defer(is) => ConduitM::Defer(is.append(js)),
            ConduitM::Await(is) => ConduitM::Await(is.append(js)),
            ConduitM::Yield(o, is) => ConduitM::Yield(o, is.append(js)),
            ConduitM::Leftover(i, is) => ConduitM::Leftover(i, is.append(js))
        }
    }

    /// Appends two conduits together, which means, it returns a new conduit that
    /// executes both conduits sequentially, and forwards the return value
    /// of the second.
    pub fn and<B: 'a>(self, other: ConduitM<'a, I, O, B>) -> ConduitM<'a, I, O, B>
        where I: 'a, O: 'a {
        self.and_then(|_| other)
    }

    /// Zips two conduits together, which means, it returns a new conduit that
    /// executes both conduits sequentially, and forwards both return values.
    pub fn zip<B: 'a>(self, other: ConduitM<'a, I, O, B>) -> ConduitM<'a, I, O, (A, B)>
        where A: 'a, I: 'a, O: 'a {
        self.and_then(|a| other.map(|b| (a, b)))
    }

    /// Modifies the return value of the conduit.
    /// Seen differently, it lifts a function from
    /// `A` to `B` into a function from `ConduitM<I, O, A>`
    /// to `ConduitM<I, O, B>`.
    pub fn map<B, F>(self, f: F) -> ConduitM<'a, I, O, B>
        where F: 'a + FnOnce(A) -> B {
        self.and_then(move |a| f(a).into())
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
            &ConduitM::Defer(_) => write!(f, "Defer(..)"),
            &ConduitM::Await(_) => write!(f, "Await(..)"),
            &ConduitM::Yield(_, _) => write!(f, "Yield(..)"),
            &ConduitM::Leftover(_, _) => write!(f, "Leftover(..)")
        }
    }
}

impl<'a, I, O, A> From<A> for ConduitM<'a, I, O, A> {
    fn from(a: A) -> ConduitM<'a, I, O, A> {
        ConduitM::Pure(Box::new(a))
    }
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

/// Defers a conduit action. Can be used to introduce artifical laziness.
pub fn defer<'a, I, O>() -> ConduitM<'a, I, O, ()> {
    ConduitM::Defer(Kleisli::new())
}

/// Provide a single piece of leftover input to be consumed by the
/// next component in the current binding.
pub fn leftover<'a, I, O>(i: I) -> ConduitM<'a, I, O, ()> {
    ConduitM::Leftover(Box::new(i), Kleisli::new())
}

/// Provide for a stream of data that can be flushed.
///
/// A number of conduits need the ability to flush the stream at some point.
/// This provides a single wrapper datatype to be used in all such circumstances.
pub enum Flush<O> {
    Chunk(O),
    Flush
}

impl<O> Flush<O> {
    pub fn map<P, F: FnOnce(O) -> P>(self, f: F) -> Flush<P> {
        match self {
            Flush::Chunk(o) => Flush::Chunk(f(o)),
            Flush::Flush => Flush::Flush
        }
    }
}

impl<O> From<O> for Flush<O> {
    fn from(o: O) -> Flush<O> {
        Flush::Chunk(o)
    }
}
