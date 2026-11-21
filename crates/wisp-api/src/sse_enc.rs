//! The sync stream compressed for a client that accepts it (perf audit 2026-11-18 #6): a new client's first look
//! at the task list is every page of it, 4.9 MB of JSON at 5,000 tasks, and it went out as it was. Each frame is
//! written through one streaming encoder per connection and flushed, so the client reads every frame the moment it
//! is sent (the patch latency budget) and the encoder keeps its window across frames (rows repeat their keys).
//! Big frames (a snapshot page, ~460 KB) are compressed on the blocking pool; small ones (a patch) in place.
use std::io::Write;

use bytes::Bytes;
use tokio::sync::mpsc;

use crate::respond::Encoding;

/// A frame this big or more is compressed off the async workers.
const OFF_THREAD: usize = 32 * 1024;
/// brotli's level for a live stream: fast enough for megabytes a second, most of the ratio of the slow levels.
const BR_QUALITY: u32 = 4;
const GZIP_LEVEL: u32 = 4;

enum Enc {
    Br(Box<brotli::CompressorWriter<Vec<u8>>>),
    Gzip(flate2::write::GzEncoder<Vec<u8>>),
}

impl Enc {
    fn new(e: Encoding) -> Self {
        match e {
            Encoding::Br => Self::Br(Box::new(brotli::CompressorWriter::new(Vec::new(), 16 * 1024, BR_QUALITY, 22))),
            Encoding::Gzip => Self::Gzip(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(GZIP_LEVEL))),
        }
    }

    /// `frame` compressed and flushed: what the client can decode in full now.
    fn frame(&mut self, frame: &[u8]) -> std::io::Result<Vec<u8>> {
        match self {
            Self::Br(w) => {
                w.write_all(frame)?;
                w.flush()?;
                Ok(std::mem::take(w.get_mut()))
            }
            Self::Gzip(w) => {
                w.write_all(frame)?;
                w.flush()?;
                Ok(std::mem::take(w.get_mut()))
            }
        }
    }
}

/// `rx`'s frames, compressed with `enc`, as a new receiver (its task ends with `rx` or the client).
pub fn compressed(enc: Encoding, mut rx: mpsc::Receiver<Bytes>) -> mpsc::Receiver<Bytes> {
    let (tx, out) = mpsc::channel::<Bytes>(64);
    tokio::spawn(async move {
        let mut e = Some(Enc::new(enc));
        while let Some(frame) = rx.recv().await {
            let Some(mut w) = e.take() else { return };
            let (back, done) = if frame.len() >= OFF_THREAD {
                match tokio::task::spawn_blocking(move || {
                    let r = w.frame(&frame);
                    (w, r)
                })
                .await
                {
                    Ok(pair) => pair,
                    Err(_) => return,
                }
            } else {
                let r = w.frame(&frame);
                (w, r)
            };
            e = Some(back);
            let Ok(bytes) = done else { return };
            if !bytes.is_empty() && tx.send(Bytes::from(bytes)).await.is_err() {
                return;
            }
        }
    });
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::io::Read;

    use super::*;

    async fn roundtrip(enc: Encoding) {
        let (tx, rx) = mpsc::channel(8);
        let mut out = compressed(enc, rx);
        let page = format!("id: 1\nevent: sync\ndata: [{}]\n\n", r#"{"id":"s1","title":"Fix the flaky auth test"},"#.repeat(2_000));
        let frames = [page.clone(), "id: 2\nevent: sync\ndata: [{\"t\":\"patch\"}]\n\n".to_owned()];
        let mut wire = Vec::new();
        for f in &frames {
            tx.send(Bytes::from(f.clone())).await.unwrap();
            // every frame decodes in full as soon as it was sent: nothing waits for the next one
            wire.extend_from_slice(&out.recv().await.unwrap());
            let mut got = String::new();
            match enc {
                Encoding::Br => drop(brotli::Decompressor::new(&wire[..], 4096).read_to_string(&mut got)),
                Encoding::Gzip => drop(flate2::read::GzDecoder::new(&wire[..]).read_to_string(&mut got)),
            }
            assert!(got.ends_with(f.as_str()), "{enc:?}: the frame decodes as soon as it is sent");
        }
        assert!(wire.len() * 10 < page.len(), "{enc:?}: {} bytes for {}", wire.len(), page.len());
    }

    #[tokio::test]
    async fn each_frame_decodes_when_sent_and_the_stream_is_a_fraction_of_its_json() {
        roundtrip(Encoding::Br).await;
        roundtrip(Encoding::Gzip).await;
    }
}
