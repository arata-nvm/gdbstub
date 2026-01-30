//! Buffered wrapper that batches reads to avoid one syscall per byte.

use crate::conn::Connection;
use crate::conn::ConnectionExt;
use std::io::{self, ErrorKind};

const READ_BUF_SIZE: usize = 512;

/// Wraps a [`ConnectionExt`] and buffers reads so that multiple bytes are
/// read in one syscall instead of one byte at a time.
///
/// Requires the inner connection's error type to be [`std::io::Error`]
/// (e.g. [`TcpStream`](std::net::TcpStream)).
#[derive(Debug)]
pub struct BufferedConnection<C> {
    inner: C,
    buf: [u8; READ_BUF_SIZE],
    len: usize,
    read_pos: usize,
}

impl<C> BufferedConnection<C> {
    /// Create a new buffered connection wrapping `inner`.
    pub fn new(inner: C) -> Self {
        Self {
            inner,
            buf: [0; READ_BUF_SIZE],
            len: 0,
            read_pos: 0,
        }
    }

    /// Get a reference to the inner connection.
    pub fn get_ref(&self) -> &C {
        &self.inner
    }

    /// Get a mutable reference to the inner connection.
    pub fn get_mut(&mut self) -> &mut C {
        &mut self.inner
    }

    /// Unwrap and return the inner connection.
    pub fn into_inner(self) -> C {
        self.inner
    }
}

impl<C> Connection for BufferedConnection<C>
where
    C: Connection<Error = io::Error>,
{
    type Error = io::Error;

    fn write(&mut self, byte: u8) -> Result<(), Self::Error> {
        self.inner.write(byte)
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.inner.write_all(buf)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush()
    }

    fn on_session_start(&mut self) -> Result<(), Self::Error> {
        self.inner.on_session_start()
    }
}

impl<C> ConnectionExt for BufferedConnection<C>
where
    C: ConnectionExt<Error = io::Error>,
{
    fn read(&mut self) -> Result<u8, Self::Error> {
        if self.read_pos < self.len {
            let b = self.buf[self.read_pos];
            self.read_pos += 1;
            return Ok(b);
        }
        let n = self.inner.read_buf(&mut self.buf)?;
        if n == 0 {
            return Err(io::Error::from(ErrorKind::UnexpectedEof));
        }
        self.len = n;
        self.read_pos = 1;
        Ok(self.buf[0])
    }

    fn read_buf(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut written = 0;
        while written < buf.len() {
            if self.read_pos < self.len {
                let n = (self.len - self.read_pos).min(buf.len() - written);
                buf[written..written + n].copy_from_slice(&self.buf[self.read_pos..self.read_pos + n]);
                self.read_pos += n;
                written += n;
            } else {
                let n = self.inner.read_buf(&mut self.buf)?;
                if n == 0 {
                    break;
                }
                self.len = n;
                self.read_pos = 0;
            }
        }
        Ok(if written > 0 { written } else { 0 })
    }

    fn peek(&mut self) -> Result<Option<u8>, Self::Error> {
        if self.read_pos < self.len {
            return Ok(Some(self.buf[self.read_pos]));
        }
        let n = self.inner.read_buf(&mut self.buf)?;
        if n == 0 {
            return Ok(None);
        }
        self.len = n;
        self.read_pos = 0;
        Ok(Some(self.buf[0]))
    }
}
