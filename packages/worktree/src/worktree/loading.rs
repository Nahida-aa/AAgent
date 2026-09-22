use super::*;

pub async fn decode_file_text(
    fs: &dyn Fs,
    abs_path: &Path,
) -> Result<(String, &'static Encoding, bool)> {
    let mut file = fs
        .open_sync(&abs_path)
        .await
        .with_context(|| format!("opening file {abs_path:?}"))?;

    let (file_first_bytes, reached_eof) = read_file_header(&mut *file, abs_path)?;
    let (_, byte_content) = decode_byte_header(&file_first_bytes);
    anyhow::ensure!(
        byte_content != ByteContent::Binary,
        "Binary files are not supported"
    );

    // If the file is eligible for opening, read the rest of the file.
    let mut content = file_first_bytes;
    if !reached_eof {
        read_file_to_end(&mut *file, &mut content, abs_path).await?;
    }
    let decoded = decode_text(content)?;
    Ok((decoded.text, decoded.encoding, decoded.has_bom))
}

pub(crate) async fn decode_file_text_to_rope(
    fs: &dyn Fs,
    abs_path: &Path,
) -> Result<(Rope, LineEnding, &'static Encoding, bool)> {
    let mut file = fs
        .open_sync(abs_path)
        .await
        .with_context(|| format!("opening file {abs_path:?}"))?;

    let (prefix, reached_eof) = read_file_header(&mut *file, abs_path)?;
    let (bom_encoding, byte_content) = decode_byte_header(&prefix);
    anyhow::ensure!(
        byte_content != ByteContent::Binary,
        "Binary files are not supported"
    );

    // Only BOM-less, non-UTF-16 files are candidates for streaming: everything
    // else needs the whole byte buffer in hand to decode or to detect encoding.
    if bom_encoding.is_none()
        && byte_content == ByteContent::Unknown
        && let Some((rope, line_ending)) =
            stream_utf8_into_rope(&mut *file, prefix, reached_eof, abs_path).await?
    {
        return Ok((rope, line_ending, encoding_rs::UTF_8, false));
    }

    // Not plain UTF-8 after all. Re-read the file and decode it all at once.
    let (mut text, encoding, has_bom) = decode_file_text(fs, abs_path).await?;
    let line_ending = LineEnding::detect(&text);
    LineEnding::normalize(&mut text);
    Ok((Rope::from(text), line_ending, encoding, has_bom))
}

async fn stream_utf8_into_rope(
    file: &mut (dyn Read + Send),
    prefix: Vec<u8>,
    reached_eof: bool,
    abs_path: &Path,
) -> Result<Option<(Rope, LineEnding)>> {
    let mut rope = Rope::new();
    let mut line_ending = None;
    let mut scratch = String::new();
    let mut pending = prefix;
    let mut buf = vec![0u8; STREAM_BLOCK_BYTES];
    let mut eof = reached_eof;

    loop {
        // Fill a whole block before decoding, so that each `Rope::push` gets a
        // slice large enough to build its chunks in parallel.
        while !eof && pending.len() < STREAM_BLOCK_BYTES {
            let n = file
                .read(&mut buf)
                .with_context(|| format!("reading bytes of the file {abs_path:?}"))?;
            if n == 0 {
                eof = true;
            } else {
                pending.extend_from_slice(&buf[..n]);
            }
        }

        // Decode as much of `pending` as forms complete UTF-8.
        let valid_len = match std::str::from_utf8(&pending) {
            Ok(_) => pending.len(),
            // A multi-byte character straddling the block boundary; the rest of
            // it arrives with the next block.
            Err(e) if e.error_len().is_none() && !eof => e.valid_up_to(),
            // Genuinely not UTF-8, or truncated at EOF: fall back.
            Err(_) => return Ok(None),
        };

        // Hold back a trailing carriage return until we know what follows it.
        let emit_len = if !eof && valid_len > 0 && pending[valid_len - 1] == b'\r' {
            valid_len - 1
        } else {
            valid_len
        };

        let text = std::str::from_utf8(&pending[..emit_len])
            .expect("a prefix of validated UTF-8 is itself valid UTF-8");

        // ISO-2022-JP and friends are valid UTF-8 but carry escape sequences, so
        // they need the full-file encoding detector rather than this fast path.
        if text.contains('\x1b') {
            return Ok(None);
        }

        // `LineEnding::detect` only inspects the first 1000 bytes, so the first
        // block gives the same answer the whole file would.
        if line_ending.is_none() && !text.is_empty() {
            line_ending = Some(LineEnding::detect(text));
        }

        push_normalized(&mut rope, text, &mut scratch);
        pending.drain(..emit_len);

        if eof {
            break;
        }

        yield_now().await;
    }

    // At EOF everything should have been consumed. Anything left over is a
    // truncated multi-byte sequence, which means this is not valid UTF-8.
    if !pending.is_empty() {
        return Ok(None);
    }

    Ok(Some((rope, line_ending.unwrap_or_default())))
}

fn read_file_header(file: &mut dyn Read, abs_path: &Path) -> Result<(Vec<u8>, bool)> {
    let mut header = Vec::with_capacity(FILE_ANALYSIS_BYTES);
    let mut buf = [0u8; FILE_ANALYSIS_BYTES];
    let mut reached_eof = false;
    while header.len() < FILE_ANALYSIS_BYTES {
        let n = file
            .read(&mut buf)
            .with_context(|| format!("reading bytes of the file {abs_path:?}"))?;
        if n == 0 {
            reached_eof = true;
            break;
        }
        header.extend_from_slice(&buf[..n]);
    }
    Ok((header, reached_eof))
}
async fn read_file_to_end(
    file: &mut (dyn Read + Send),
    content: &mut Vec<u8>,
    abs_path: &Path,
) -> Result<()> {
    let mut buf = vec![0u8; STREAM_BLOCK_BYTES];
    loop {
        let mut block_len = 0;
        while block_len < buf.len() {
            let n = file
                .read(&mut buf[block_len..])
                .with_context(|| format!("reading remaining bytes of the file {abs_path:?}"))?;
            if n == 0 {
                break;
            }
            block_len += n;
        }

        if block_len == 0 {
            break;
        }

        content.extend_from_slice(&buf[..block_len]);
        if block_len < buf.len() {
            break;
        }

        yield_now().await;
    }
    Ok(())
}
fn push_normalized(rope: &mut Rope, text: &str, scratch: &mut String) {
    if !text.contains('\r') {
        rope.push(text);
        return;
    }

    scratch.clear();
    scratch.reserve(text.len());
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut ix = 0;
    while ix < bytes.len() {
        if bytes[ix] == b'\r' {
            scratch.push_str(&text[start..ix]);
            scratch.push('\n');
            ix += if bytes.get(ix + 1) == Some(&b'\n') {
                2
            } else {
                1
            };
            start = ix;
        } else {
            ix += 1;
        }
    }
    scratch.push_str(&text[start..]);
    rope.push(scratch);
}
pub fn decode_byte_header(prefix: &[u8]) -> (Option<&'static Encoding>, ByteContent) {
    if let Some((encoding, _bom_len)) = Encoding::for_bom(prefix) {
        return (Some(encoding), ByteContent::Unknown);
    }
    (None, analyze_byte_content(prefix))
}
