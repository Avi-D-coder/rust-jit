//! Limits at the IO boundary, before the pinned JSON decoder allocates a reply.
use std::io::{self, BufRead, Read};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub(super) const MAX_REPLY_BYTES: usize = usize::MAX;
pub(super) const MAX_PREVIEW_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub(super) struct ReplyAllowance {
    remaining: Arc<AtomicUsize>,
    exhausted: Arc<AtomicBool>,
}
impl ReplyAllowance {
    fn new(bytes: usize) -> io::Result<Self> {
        let allowance = Self {
            remaining: Arc::new(AtomicUsize::new(0)),
            exhausted: Arc::new(AtomicBool::new(false)),
        };
        allowance.reset(bytes)?;
        Ok(allowance)
    }
    /// Only the serial session owner resets this, between completed operations.
    pub(super) fn reset(&self, bytes: usize) -> io::Result<()> {
        if bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid macro IO allowance",
            ));
        }
        self.exhausted.store(false, Ordering::Release);
        self.remaining.store(bytes, Ordering::Release);
        Ok(())
    }
    pub(super) fn exhausted(&self) -> bool {
        self.exhausted.load(Ordering::Acquire)
    }
}

/// A finite response allowance. Reaching the ceiling is an error, never a
/// fabricated EOF (which could turn a truncated reply into an accepted reply).
pub(super) struct BoundedReader<R> {
    inner: R,
    allowance: ReplyAllowance,
    preview: Vec<u8>,
    preview_limit: usize,
    truncated: bool,
}

impl<R: BufRead> BoundedReader<R> {
    pub(super) fn new(inner: R, reply_bytes: usize, preview_bytes: usize) -> io::Result<Self> {
        if reply_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid macro IO allowance",
            ));
        }
        Ok(Self {
            inner,
            allowance: ReplyAllowance::new(reply_bytes)?,
            preview: Vec::new(),
            preview_limit: preview_bytes,
            truncated: false,
        })
    }

    fn check_remaining(&self) -> io::Result<()> {
        if self.remaining() == 0 {
            self.allowance.exhausted.store(true, Ordering::Release);
            return Err(io::Error::other("macro reply byte allowance exhausted"));
        }
        Ok(())
    }

    fn remaining(&self) -> usize {
        self.allowance.remaining.load(Ordering::Acquire)
    }
    pub(super) fn allowance(&self) -> ReplyAllowance {
        self.allowance.clone()
    }

    fn record(&mut self, bytes: &[u8]) {
        let keep = (self.preview_limit - self.preview.len()).min(bytes.len());
        self.preview.extend_from_slice(&bytes[..keep]);
        self.truncated |= keep < bytes.len();
        self.allowance
            .remaining
            .fetch_sub(bytes.len(), Ordering::AcqRel);
    }

    pub(super) fn preview(&self) -> (&[u8], bool) {
        (&self.preview, self.truncated)
    }
}

impl<R: BufRead> Read for BoundedReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        self.check_remaining()?;
        let allowed = out.len().min(self.remaining());
        let count = self.inner.read(&mut out[..allowed])?;
        self.record(&out[..count]);
        Ok(count)
    }
}

impl<R: BufRead> BufRead for BoundedReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.check_remaining()?;
        let remaining = self.remaining();
        let available = self.inner.fill_buf()?;
        Ok(&available[..available.len().min(remaining)])
    }

    fn consume(&mut self, amount: usize) {
        // BufRead callers may consume only exposed bytes. Clamp even an invalid
        // caller instead of underflowing the allowance or reading hidden bytes.
        let remaining = self.remaining();
        if amount == 0 || remaining == 0 {
            return;
        }
        let Ok(bytes) = self.inner.fill_buf() else {
            return;
        };
        let count = amount.min(bytes.len()).min(remaining);
        let keep = (self.preview_limit - self.preview.len()).min(count);
        self.preview.extend_from_slice(&bytes[..keep]);
        self.truncated |= keep < count;
        self.allowance.remaining.fetch_sub(count, Ordering::AcqRel);
        self.inner.consume(count);
    }
}

pub(super) struct BoundedWriter<W> {
    inner: W,
    allowance: ReplyAllowance,
}
impl<W: std::io::Write> BoundedWriter<W> {
    pub(super) fn new(inner: W, bytes: usize) -> io::Result<Self> {
        Ok(Self {
            inner,
            allowance: ReplyAllowance::new(bytes)?,
        })
    }
    pub(super) fn allowance(&self) -> ReplyAllowance {
        self.allowance.clone()
    }
}
impl<W: std::io::Write> std::io::Write for BoundedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let remaining = self.allowance.remaining.load(Ordering::Acquire);
        if remaining == 0 {
            self.allowance.exhausted.store(true, Ordering::Release);
            return Err(io::Error::other("macro request byte allowance exhausted"));
        }
        let count = self.inner.write(&bytes[..bytes.len().min(remaining)])?;
        self.allowance.remaining.fetch_sub(count, Ordering::AcqRel);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn unterminated_protocol_line_stops_before_decoder_growth() {
        let mut reader = BoundedReader::new(Cursor::new(vec![b'x'; 4096]), 31, 7).unwrap();
        let mut line = String::new();
        let error = reader.read_line(&mut line).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(line.len(), 31);
        assert_eq!(reader.preview(), (&b"xxxxxxx"[..], true));
        assert!(reader.fill_buf().is_err());
    }

    #[test]
    fn exact_boundary_line_is_accepted_but_following_bytes_are_refused() {
        let mut reader = BoundedReader::new(Cursor::new(b"abc\nsecret"), 4, 4).unwrap();
        let mut line = String::new();
        assert_eq!(reader.read_line(&mut line).unwrap(), 4);
        assert_eq!(line, "abc\n");
        assert_eq!(reader.preview(), (&b"abc\n"[..], false));
        assert!(reader.read(&mut [0; 1]).is_err());
        assert_eq!(reader.read(&mut []).unwrap(), 0);
    }

    #[test]
    fn read_and_buffered_paths_share_one_allowance() {
        let mut reader = BoundedReader::new(Cursor::new(b"abcdef\n"), 6, 2).unwrap();
        let mut prefix = [0; 3];
        assert_eq!(reader.read(&mut prefix).unwrap(), 3);
        assert_eq!(&prefix, b"abc");
        assert_eq!(reader.fill_buf().unwrap(), b"def");
        reader.consume(usize::MAX);
        assert!(reader.fill_buf().is_err());
        assert_eq!(reader.preview(), (&b"ab"[..], true));
    }

    #[test]
    fn pinned_decoder_refuses_oversized_response_and_accepts_real_protocol() {
        use proc_macro_api::legacy_protocol::msg::{Message, Response};
        let mut bytes = Vec::new();
        Response::ApiVersionCheck(6).write(&mut bytes).unwrap();
        let mut reader = BoundedReader::new(Cursor::new(&bytes), bytes.len(), 0).unwrap();
        let response = Response::read(&mut reader, &mut String::new()).unwrap();
        assert!(matches!(response, Some(Response::ApiVersionCheck(6))));
        let mut reader = BoundedReader::new(Cursor::new(&bytes), bytes.len() - 1, 0).unwrap();
        assert!(Response::read(&mut reader, &mut String::new()).is_err());
    }

    #[test]
    fn bidirectional_decoder_obeys_frame_budget_and_rejects_malformed_cobs() {
        use proc_macro_api::{
            bidirectional_protocol::msg::{BidirectionalMessage, Response},
            legacy_protocol::msg::Message,
        };
        let mut bytes = Vec::new();
        BidirectionalMessage::Response(Response::ApiVersionCheck(6))
            .write(&mut bytes)
            .unwrap();
        let mut reader = BoundedReader::new(Cursor::new(&bytes), bytes.len(), 0).unwrap();
        assert!(matches!(
            BidirectionalMessage::read(&mut reader, &mut Vec::new()).unwrap(),
            Some(BidirectionalMessage::Response(Response::ApiVersionCheck(6)))
        ));
        let mut buffer = Vec::new();
        let mut reader = BoundedReader::new(Cursor::new(&bytes), bytes.len() - 1, 0).unwrap();
        assert!(BidirectionalMessage::read(&mut reader, &mut buffer).is_err());
        assert_eq!(buffer.len(), bytes.len() - 1);
        let mut reader = BoundedReader::new(Cursor::new(vec![1; 4096]), 31, 0).unwrap();
        assert!(BidirectionalMessage::read(&mut reader, &mut buffer).is_err());
        assert_eq!(buffer.len(), 31);
        let mut reader = BoundedReader::new(Cursor::new([255, 0]), 2, 0).unwrap();
        assert_eq!(
            BidirectionalMessage::read(&mut reader, &mut buffer)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn bidirectional_conversation_bounds_subrequests_and_preserves_callback_denial() {
        use proc_macro_api::{
            ServerError,
            bidirectional_protocol::{
                msg::{
                    ApiVersionCheck, BidirectionalMessage as Msg, Request, Response, SubRequest,
                    SubResponse,
                },
                run_conversation,
            },
            legacy_protocol::msg::Message,
        };
        let request = || Msg::Request(Request::ApiVersionCheck(ApiVersionCheck {}));
        let mut frames = Vec::new();
        Msg::SubRequest(SubRequest::FilePath { file_id: 7 })
            .write(&mut frames)
            .unwrap();
        Msg::Response(Response::ApiVersionCheck(6))
            .write(&mut frames)
            .unwrap();
        let mut reader = BoundedReader::new(Cursor::new(&frames), frames.len(), 0).unwrap();
        let mut writer = BoundedWriter::new(Cursor::new(Vec::new()), 1024).unwrap();
        let result = run_conversation(
            &mut writer,
            &mut reader,
            &mut Vec::new(),
            request(),
            &|_| {
                Err(ServerError {
                    message: "captured source denied".into(),
                    io: None,
                })
            },
        )
        .unwrap();
        assert!(matches!(
            result,
            Msg::Response(Response::ApiVersionCheck(6))
        ));
        let mut sent = Cursor::new(writer.inner.into_inner());
        assert!(matches!(
            Msg::read(&mut sent, &mut Vec::new()).unwrap(),
            Some(Msg::Request(Request::ApiVersionCheck(_)))
        ));
        assert!(
            matches!(Msg::read(&mut sent, &mut Vec::new()).unwrap(), Some(Msg::SubResponse(SubResponse::Cancel {reason})) if reason.contains("captured source denied"))
        );
        let mut reader = BoundedReader::new(Cursor::new(&frames), frames.len(), 0).unwrap();
        let mut writer = BoundedWriter::new(Cursor::new(Vec::new()), 8).unwrap();
        assert!(
            run_conversation(
                &mut writer,
                &mut reader,
                &mut Vec::new(),
                request(),
                &|_| Ok(SubResponse::FilePathResult {
                    name: "large".repeat(20)
                })
            )
            .is_err()
        );
        assert_eq!(writer.inner.get_ref().len(), 8);
        assert!(writer.allowance().exhausted());
    }

    #[test]
    fn invalid_allowances_are_refused_before_reading() {
        for (reply, preview) in [(0, 0)] {
            assert!(BoundedReader::new(Cursor::new(b"unused"), reply, preview).is_err());
        }
        let mut reader = BoundedReader::new(Cursor::new(b""), 1, 0).unwrap();
        assert_eq!(reader.read(&mut [0; 1]).unwrap(), 0);
    }
    #[test]
    fn request_writer_stops_at_exact_budget_and_resets_only_between_operations() {
        use std::io::Write;
        let mut writer = BoundedWriter::new(Cursor::new(Vec::new()), 4).unwrap();
        let quota = writer.allowance();
        writer.write_all(b"four").unwrap();
        assert!(!quota.exhausted());
        assert!(writer.write_all(b"extra").is_err());
        assert!(quota.exhausted());
        assert_eq!(writer.inner.get_ref(), b"four");
        quota.reset(2).unwrap();
        writer.write_all(b"ok").unwrap();
        assert!(writer.write_all(b"no").is_err());
        assert_eq!(writer.inner.get_ref(), b"fourok");
        assert!(quota.reset(0).is_err());
    }
}
