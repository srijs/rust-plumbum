use std::io;
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, RecvError, Sender, SyncSender, SendError};

use super::{ConduitM, produce, consume, defer, leftover, Flush};

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
pub fn reader<'a, R: 'a + Read>(mut r: R, z: usize) -> ConduitM<'a, (), Vec<u8>, io::Result<()>> {
    defer().and_then(move |_| {
        match read(&mut r, z) {
            Err(e) => Err(e).into(),
            Ok(v) => {
                if v.len() == 0 {
                    Ok(()).into()
                } else {
                    produce(v).and_then(move |_| reader(r, z))
                }
            }
        }
    })
}

/// A conduit that consumes bytes and writes them to the given `Write`.
pub fn writer<'a, W: 'a + Write>(mut w: W) -> ConduitM<'a, Vec<u8>, (), io::Result<()>> {
    consume().and_then(|vo: Option<Vec<u8>>| {
        match vo {
            None => Ok(()).into(),
            Some(v) => match w.write_all(&v) {
                Err(e) => Err(e).into(),
                Ok(_) => writer(w)
            }
        }
    })
}

fn write_flush<W: Write>(w: &mut W, f: Flush<Vec<u8>>) -> io::Result<()> {
    match f {
        Flush::Chunk(v) => w.write_all(&v),
        Flush::Flush => w.flush()
    }
}

/// A conduit that consumes bytes and writes them to the given `Write`,
/// while also providing flushing capabilities.
pub fn writer_flush<'a, W: 'a + Write>(mut w: W) -> ConduitM<'a, Flush<Vec<u8>>, (), io::Result<()>> {
    consume().and_then(|fo| {
        match fo {
            None => Ok(()).into(),
            Some(f) => match write_flush(&mut w, f) {
                Err(e) => Err(e).into(),
                Ok(_) => writer_flush(w)
            }
        }
    })
}

pub fn receiver<'a, T: 'a>(r: Receiver<T>) -> ConduitM<'a, (), T, ()> {
    defer().and_then(|_| {
        match r.recv() {
            Err(RecvError) => ().into(),
            Ok(x) => produce(x).and(receiver(r))
        }
    })
}

pub fn sender<'a, T: 'a>(s: Sender<T>) -> ConduitM<'a, T, (), ()> {
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

pub fn sync_sender<'a, T: 'a>(s: SyncSender<T>) -> ConduitM<'a, T, (), ()> {
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
