//! Bound each stdio frame before the SDK allocates/decodes the whole message.
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};

pub const MAX_REQUEST_BYTES: usize = 1_048_576;
pub struct BoundedLines<R> {
    inner: R,
    line_bytes: usize,
}
impl<R> BoundedLines<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            line_bytes: 0,
        }
    }
}
impl<R: AsyncRead + Unpin> AsyncRead for BoundedLines<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let mut scratch = [0u8; 8192];
        let size = buf.remaining().min(scratch.len());
        let mut read = ReadBuf::new(&mut scratch[..size]);
        match Pin::new(&mut this.inner).poll_read(cx, &mut read) {
            Poll::Ready(Ok(())) => {
                for &byte in read.filled() {
                    if byte == b'\n' {
                        this.line_bytes = 0;
                    } else {
                        this.line_bytes += 1;
                        if this.line_bytes > MAX_REQUEST_BYTES {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "MCP frame exceeds 1 MiB",
                            )));
                        }
                    }
                }
                buf.put_slice(read.filled());
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    #[tokio::test]
    async fn resets_per_frame_and_rejects_oversized_input() {
        let data = vec![b'x'; MAX_REQUEST_BYTES + 1];
        assert!(BoundedLines::new(&data[..])
            .read_to_end(&mut Vec::new())
            .await
            .is_err());
        let data = b"abc\ndef\n";
        let mut output = Vec::new();
        BoundedLines::new(&data[..])
            .read_to_end(&mut output)
            .await
            .unwrap();
        assert_eq!(data, &output[..]);
    }
}
