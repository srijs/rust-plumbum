use std::io;
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, RecvError, Sender, SyncSender, SendError};

use super::{ConduitM, Void, Chunk, Sink, Source, produce, produce_chunk, consume, consume_chunk, defer, leftover};

fn read<R: Read>(r: &mut R, z: usize) -> io::Result<Vec<u8>> {
    let mut v = Vec::with_capacity(z);
    unsafe { v.set_len(z); }
    match r.read(&mut v) {
        Err(e) => Err(e),
        Ok(n) => {
            unsafe { v.set_len(n); }
            Ok(v)
        }
    }
}

/// A conduit that produces bytes it reads from the given `Read`.
pub fn reader<'a, R: 'a + Read>(mut r: R, z: usize) -> ConduitM<'a, Void, u8, io::Result<()>> {
    defer().and_then(move |_| {
        match read(&mut r, z) {
            Err(e) => Err(e).into(),
            Ok(v) => {
                if v.len() == 0 {
                    Ok(()).into()
                } else {
                    produce_chunk(v).and_then(move |_| reader(r, z))
                }
            }
        }
    })
}

/// A conduit that consumes bytes and writes them to the given `Write`.
pub fn writer<'a, W: 'a + Write>(mut w: W) -> ConduitM<'a, u8, Void, io::Result<()>> {
    consume_chunk().and_then(|vo: Chunk<Vec<u8>>| {
        match vo {
            Chunk::End => Ok(()).into(),
            Chunk::Flush => match w.flush() {
                Err(e) => Err(e).into(),
                Ok(_) => writer(w)
            },
            Chunk::Chunk(v) => match w.write_all(&v) {
                Err(e) => Err(e).into(),
                Ok(_) => writer(w)
            }
        }
    })
}

/// A conduit that produces values it receives from the given `Receiver`.
pub fn receiver<'a, T: 'a>(r: Receiver<T>) -> Source<'a, T> {
    defer().and_then(|_| {
        match r.recv() {
            Err(RecvError) => ().into(),
            Ok(x) => produce(x).and(receiver(r))
        }
    })
}

/// A conduit that consumes values and writes them to the given `Sender`.
pub fn sender<'a, T: 'a>(s: Sender<T>) -> Sink<'a, T, ()> {
    consume().and_then(|xo| {
        match xo {
            None => ().into(),
            Some(x) => match s.send(x) {
                Err(SendError(x)) => leftover(x),
                Ok(_) => sender(s)
            }
        }
    })
}

/// A conduit that consumes values and writes them to the given `SyncSender`.
pub fn sync_sender<'a, T: 'a>(s: SyncSender<T>) -> Sink<'a, T, ()> {
    consume().and_then(|xo| {
        match xo {
            None => ().into(),
            Some(x) => match s.send(x) {
                Err(SendError(x)) => leftover(x),
                Ok(_) => sync_sender(s)
            }
        }
    })
}
