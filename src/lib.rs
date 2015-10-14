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
//!     defer()
//!     .and(produce(1))
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
use std::mem::{replace, swap};
use std::iter::{Extend, FromIterator};

mod chunk;
pub use chunk::Chunk;

/// Interfacing with `std::io`.
pub mod io;

mod kleisli;
pub use kleisli::Kleisli;

mod pipe;
pub use pipe::*;

mod fuse;
pub use fuse::*;

pub enum Void {}

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
    /// The case `Flush(k)` means that the conduit instructs the downstream to flush,
    /// and the remaining (suspended) program is given by the kleisli arrow `k`
    Flush(Kleisli<'a, (), I, O, A>),
    /// The case `Await(k)` means that the conduit waits for a value of type `I`,
    /// and the remaining (suspended) program is given by the kleisli arrow `k`.
    Await(Kleisli<'a, Chunk<Vec<I>>, I, O, A>),
    /// The case `Yield(o, k)` means that the conduit yields a value of type `O`,
    /// and the remaining (suspended) program is given by the kleisli arrow `k`.
    Yield(Vec<O>, Kleisli<'a, (), I, O, A>),
    /// The case `Leftover(i, k)` means that the conduit has a leftover value of type `I`,
    /// and the remaining (suspended) program is given by the kleisli arrow `k`.
    Leftover(Vec<I>, Kleisli<'a, (), I, O, A>)
}

/// Provides a stream of output values,
/// without consuming any input or producing a final result.
pub type Source<'a, O> = ConduitM<'a, Void, O, ()>;

impl<'a, O> ConduitM<'a, Void, O, ()> {

    /// Generalize a `Source` by universally quantifying the input type.
    pub fn to_producer<I>(self) -> ConduitM<'a, I, O, ()> where O: 'static {
        match self {
            ConduitM::Pure(x) => ConduitM::Pure(x),
            ConduitM::Defer(k) => ConduitM::Defer(Kleisli::from(move |_| {
                k.run(()).to_producer()
            })),
            ConduitM::Flush(k) => ConduitM::Flush(Kleisli::from(move |_| {
                k.run(()).to_producer()
            })),
            ConduitM::Await(k) => ConduitM::Defer(Kleisli::from(move |_| {
                k.run(Chunk::Chunk(Vec::new())).to_producer()
            })),
            ConduitM::Yield(o, k) => ConduitM::Yield(o, Kleisli::from(move |_| {
                k.run(()).to_producer()
            })),
            ConduitM::Leftover(_, k) => ConduitM::Defer(Kleisli::from(move |_| {
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
                ConduitM::Pure(a) => {
                    return *a;
                },
                ConduitM::Defer(k_sink) => {
                    (self, k_sink.run(()))
                },
                ConduitM::Flush(k_sink) => {
                    (self, k_sink.run(()))
                },
                ConduitM::Await(k_sink) => {
                    match self {
                        ConduitM::Pure(x) => {
                            (ConduitM::Pure(x), k_sink.run(Chunk::End))
                        },
                        ConduitM::Defer(k_src) => {
                            (k_src.run(()), ConduitM::Await(k_sink))
                        },
                        ConduitM::Flush(k_src) => {
                            (k_src.run(()), k_sink.run(Chunk::Flush))
                        },
                        ConduitM::Await(k_src) => {
                            (k_src.run(Chunk::Chunk(Vec::new())), ConduitM::Await(k_sink))
                        },
                        ConduitM::Yield(o, k_src) => {
                            (k_src.run(()), k_sink.run(Chunk::Chunk(o)))
                        },
                        ConduitM::Leftover(_, k_src) => {
                            (k_src.run(()), ConduitM::Await(k_sink))
                        }
                    }
                },
                ConduitM::Yield(_, k_sink) => {
                    (self, k_sink.run(()))
                },
                ConduitM::Leftover(o, k_sink) => {
                    (ConduitM::Yield(o, Kleisli::from(move |_| self)), k_sink.run(()))
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
            ConduitM::Defer(k) => ConduitM::Defer(Kleisli::from(move |_| {
                self.fuse(k.run(()))
            })),
            ConduitM::Flush(k) => ConduitM::Flush(Kleisli::from(move |_| {
                self.fuse(k.run(()))
            })),
            ConduitM::Yield(c, k) => ConduitM::Yield(c, Kleisli::from(move |_| {
                self.fuse(k.run(()))
            })),
            ConduitM::Leftover(o, k) => ConduitM::Defer(Kleisli::from(move |_| {
                ConduitM::Yield(o, Kleisli::from(move |_| self)).fuse(k.run(()))
            })),
            ConduitM::Await(k_right) => match self {
                ConduitM::Pure(_) => ConduitM::Defer(Kleisli::from(move |_| {
                    Conduit::fuse(().into(), k_right.run(Chunk::End))
                })),
                ConduitM::Defer(k_left) => ConduitM::Defer(Kleisli::from(move |_| {
                    k_left.run(()).fuse(ConduitM::Await(k_right))
                })),
                ConduitM::Flush(k_left) => ConduitM::Flush(Kleisli::from(move |_| {
                    k_left.run(()).fuse(k_right.run(Chunk::Flush))
                })),
                ConduitM::Yield(o, k_left) => ConduitM::Defer(Kleisli::from(move |_| {
                    k_left.run(()).fuse(k_right.run(Chunk::Chunk(o)))
                })),
                ConduitM::Leftover(i, k_left) => ConduitM::Leftover(i, Kleisli::from(move |_| {
                    k_left.run(()).fuse(ConduitM::Await(k_right))
                })),
                ConduitM::Await(k_left) => ConduitM::Await(Kleisli::from(move |a| {
                    k_left.run(a).fuse(ConduitM::Await(k_right))
                }))
            }
        }
    }

    /// Apply a transformation to all values in a stream.
    pub fn transform<F>(f: F) -> Self where I: 'a, O: 'a, F: 'a + Fn(I) -> O {
        consume().and_then(|io| match io {
            None => ().into(),
            Some(i) => produce(f(i)).and_then(move |_| Conduit::transform(f))
        })
    }

}

impl<'a, I, O: 'a> Extend<O> for ConduitM<'a, I, O, ()> {
    fn extend<T: IntoIterator<Item=O>>(&mut self, iterator: T)
        where I: 'a, T::IntoIter: 'a {
        let mut other = replace(self, ().into()).extend_iter(iterator.into_iter());
        swap(self, &mut other);
    }
}

impl<'a, I, O: 'a> FromIterator<O> for ConduitM<'a, I, O, ()> {
    fn from_iter<T: IntoIterator<Item=O>>(iterator: T) -> Self
        where I: 'a, T::IntoIter: 'a {
        ConduitM::extend_iter(().into(), iterator.into_iter())
    }
}

/// Consumes a stream of input values and produces a final result,
/// without producing any output.
pub type Sink<'a, I, A> = ConduitM<'a, I, Void, A>;

impl<'a, I, A> ConduitM<'a, I, Void, A> {

    /// Generalize a `Sink` by universally quantifying the output type.
    pub fn to_consumer<O>(self) -> ConduitM<'a, I, O, A> where I: 'static, A: 'a {
        match self {
            ConduitM::Pure(x) => ConduitM::Pure(x),
            ConduitM::Defer(k) => ConduitM::Defer(Kleisli::from(move |_| {
                k.run(()).to_consumer()
            })),
            ConduitM::Flush(k) => ConduitM::Flush(Kleisli::from(move |_| {
                k.run(()).to_consumer()
            })),
            ConduitM::Await(k) => ConduitM::Await(Kleisli::from(move |chunk| {
                k.run(chunk).to_consumer()
            })),
            ConduitM::Yield(_, k) => ConduitM::Defer(Kleisli::from(move |_| {
                k.run(()).to_consumer()
            })),
            ConduitM::Leftover(vec, k) => ConduitM::Leftover(vec, Kleisli::from(move |_| {
                k.run(()).to_consumer()
            }))
        }
    }

    fn sink<F>(a: A, f: F) -> Self
        where I: 'a, A: 'a, F: 'a + Fn(A, I) -> Result<A, A> {
        consume().and_then(|io| {
            match io {
                None => a.into(),
                Some(is) => match f(a, is) {
                    Ok(a) => Self::sink(a, f),
                    Err(a) => a.into()
                }
            }
        })
    }

    /// Fold all values from upstream into a final value.
    pub fn fold<F>(a: A, f: F) -> Self
        where I: 'a, A: 'a, F: 'a + Fn(A, I) -> A {
        Self::sink(a, move |a, i| Ok(f(a, i)))
    }

}

impl<'a, I, O, A> ConduitM<'a, I, O, A> {

    fn and_then_boxed<B, F>(self, js: F) -> ConduitM<'a, I, O, B>
        where F: 'a + FnOnce(Box<A>) -> ConduitM<'a, I, O, B> {
        match self {
            ConduitM::Pure(a) => js(a),
            ConduitM::Defer(is) => ConduitM::Defer(kleisli::append_boxed(is, js)),
            ConduitM::Flush(is) => ConduitM::Flush(kleisli::append_boxed(is, js)),
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
            ConduitM::Flush(is) => ConduitM::Flush(is.append(js)),
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

    fn extend_iter<T: 'a + Iterator<Item=O>>(self, mut iterator: T) -> Self
        where I: 'a, O: 'a, A: 'a {
        self.and_then(|a| {
            let next = iterator.next();
            match next {
                None => a.into(),
                Some(x) => produce(x).and(a.into()).extend_iter(iterator)
            }
        })
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
            &ConduitM::Flush(_) => write!(f, "Flush(..)"),
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
pub fn consume_chunk<'a, I, O>() -> ConduitM<'a, I, O, Chunk<Vec<I>>> {
    ConduitM::Await(Kleisli::new())
}

/// Wait for an input chunk from upstream.
///
/// If no data is available, returns `None`.
/// Once it returns `None`, subsequent calls will also return `None`.
pub fn consume<'a, I: 'a, O: 'a>() -> ConduitM<'a, I, O, Option<I>> {
    ConduitM::Await(Kleisli::from(move |iso: Chunk<Vec<I>>| {
        match iso {
            Chunk::End => None.into(),
            Chunk::Flush => consume(),
            Chunk::Chunk(mut is) => if is.len() > 0 {
                let i = is.remove(0);
                leftover_chunk(is).and(ConduitM::from(Some(i)))
            } else {
                consume()
            }
        }
    }))
}

/// Send a chunk of values downstream to the next component to consume.
///
/// If the downstream component terminates, this call will never return control.
pub fn produce_chunk<'a, I, O>(o: Vec<O>) -> ConduitM<'a, I, O, ()> {
    ConduitM::Yield(o, Kleisli::new())
}

/// Send a value downstream to the next component to consume.
///
/// If the downstream component terminates, this call will never return control.
pub fn produce<'a, I, O>(o: O) -> ConduitM<'a, I, O, ()> {
    let mut v = Vec::with_capacity(1);
    v.push(o);
    ConduitM::Yield(v, Kleisli::new())
}

/// Defers a conduit action. Can be used to introduce artifical laziness.
pub fn defer<'a, I, O>() -> ConduitM<'a, I, O, ()> {
    ConduitM::Defer(Kleisli::new())
}

/// Provide a single piece of leftover input to be consumed by the
/// next component in the current binding.
pub fn leftover_chunk<'a, I, O>(i: Vec<I>) -> ConduitM<'a, I, O, ()> {
    ConduitM::Leftover(i, Kleisli::new())
}

/// Provide a single piece of leftover input to be consumed by the
/// next component in the current binding.
pub fn leftover<'a, I, O>(i: I) -> ConduitM<'a, I, O, ()> {
    let mut v = Vec::with_capacity(1);
    v.push(i);
    ConduitM::Leftover(v, Kleisli::new())
}
