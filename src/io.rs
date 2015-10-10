use std::io::{Result, Read, Write};

use super::{ConduitM, produce, consume, Flush};

fn read<R: Read>(r: &mut R, z: usize) -> Result<Vec<u8>> {
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
pub fn reader<'a, R: 'a + Read>(mut r: R, z: usize) -> ConduitM<'a, (), Vec<u8>, Result<()>> {
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
}

/// A conduit that consumes bytes and writes them to the given `Write`.
pub fn writer<'a, W: 'a + Write>(mut w: W) -> ConduitM<'a, Vec<u8>, (), Result<()>> {
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

fn write_flush<W: Write>(w: &mut W, f: Flush<Vec<u8>>) -> Result<()> {
    match f {
        Flush::Chunk(v) => w.write_all(&v),
        Flush::Flush => w.flush()
    }
}

/// A conduit that consumes bytes and writes them to the given `Write`,
/// while also providing flushing capabilities.
pub fn writer_flush<'a, W: 'a + Write>(mut w: W) -> ConduitM<'a, Flush<Vec<u8>>, (), Result<()>> {
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
