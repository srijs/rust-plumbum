# Plumbum [![Build Status](https://travis-ci.org/srijs/rust-plumbum.svg)](https://travis-ci.org/srijs/rust-plumbum)

*Plumbum* (latin for lead) is a port of Michael Snoyman's excellent
[`conduit`](https://www.fpcomplete.com/user/snoyberg/library-documentation/conduit-overview)
library.

It allows for production, transformation, and consumption of streams of
data in constant memory.
It can be used for processing files, dealing with network interfaces,
or parsing structured data in an event-driven manner.

## Features

- Large and possibly infinite streams can be processed in constant memory.

- Chunks of data are dealt with lazily, one piece at a time, instead of needing to
  read in the entire body at once.

- The resulting components are pure computations, and allow us to retain
  composability while dealing with the imperative world of I/O.

## Basics

There are three main concepts:

1. A `Source` will produce a stream of data values and send them downstream.
2. A `Sink` will consume a stream of data values from upstream and produce a return value.
3. A `Conduit` will consume a stream of values from upstream and produces a new stream to send downstream.

In order to combine these different components, we have connecting and fusing.
The `connect` method will combine a `Source` and `Sink`,
feeding the values produced by the former into the latter, and producing a final result.
Fusion, on the other hand, will take two components and generate a new component.
For example, fusing a `Conduit` and `Sink` together into a new `Sink`,
will consume the same values as the original `Conduit` and produce the same result as the original `Sink`.

## Primitives

There are four core primitives:

1. `consume` takes a single value from upstream, if available.
2. `produce` sends a single value downstream.
3. `leftover` puts a single value back in the upstream queue,
   ready to be read by the next call to `consume`.
4. `defer` introduces a point of lazyiness, artifically deferring all further actions.

## Example

```rust
use plumbum::*;

fn source<'a>() -> Source<'a, i32> {
    defer()
    .and(produce(1))
    .and(produce(2))
    .and(produce(3))
    .and(produce(4))
}

fn conduit<'a>() -> Conduit<'a, i32, String> {
    // Get adjacent pairs from upstream
    consume().zip(consume()).and_then(|res| {
        match res {
            (Some(i1), Some(i2)) => {
                produce(format!("({},{})", i1, i2))
                .and(leftover(i2))
                .and(conduit())
            },
            _ => ().into()
        }
    })
}

fn sink<'a>() -> Sink<'a, String, String> {
    consume().and_then(|res| {
        match res {
            None => "...".to_string().into(),
            Some(str) => sink().and_then(move |next| {
                format!("{}:{}", str, next).into()
            })
        }
    })
}

fn main() {
    let res = source().fuse(conduit()).connect(sink());
    assert_eq!(res, "(1,2):(2,3):(3,4):...")
}
