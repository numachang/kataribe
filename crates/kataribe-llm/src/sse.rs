//! Server-Sent Events の行単位デコーダ。
//!
//! HTTP から独立した純粋な部品にしてある。バイト列がどこで区切られて渡されても
//! （行の途中、マルチバイト文字の途中のどちらでも）正しく動く。改行 (`\n`, 0x0A) は
//! 妥当な UTF-8 の中で継続バイトとして現れないため、`\n` で区切ったバイト列は必ず
//! 文字境界として安全に文字列化できる。

/// デコーダが出す 1 つの出来事。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SseEvent {
    /// `data:` 行から組み立てた本文（複数行にまたがる場合は `\n` で結合済み）。
    Data(String),
    /// `data: [DONE]` を受け取った。
    Done,
}

/// SSE ストリームを行単位で読み、`data:` イベントを組み立てるデコーダ。
#[derive(Debug, Default)]
pub(crate) struct SseDecoder {
    buffer: Vec<u8>,
    data_lines: Vec<String>,
}

impl SseDecoder {
    /// 新しく届いたバイト列を渡し、それまでに完成したイベントを返す。
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(chunk);

        let mut events = Vec::new();
        while let Some(newline_at) = self.buffer.iter().position(|&byte| byte == b'\n') {
            let line_bytes: Vec<u8> = self.buffer.drain(..=newline_at).collect();
            let line = decode_line(&line_bytes);
            self.handle_line(&line, &mut events);
        }
        events
    }

    /// ストリーム終了時に呼ぶ。改行で終わらない末尾の行が残っていれば処理する。
    pub fn finish(mut self) -> Vec<SseEvent> {
        let mut events = Vec::new();
        if !self.buffer.is_empty() {
            let line = decode_line(&self.buffer);
            self.buffer.clear();
            self.handle_line(&line, &mut events);
        }
        if !self.data_lines.is_empty() {
            events.push(self.dispatch());
        }
        events
    }

    fn handle_line(&mut self, line: &str, events: &mut Vec<SseEvent>) {
        if line.is_empty() {
            if !self.data_lines.is_empty() {
                events.push(self.dispatch());
            }
            return;
        }
        if line.starts_with(':') {
            // コメント行。無視する。
            return;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            let value = rest.strip_prefix(' ').unwrap_or(rest);
            self.data_lines.push(value.to_string());
        }
        // data 以外のフィールド（event:, id:, retry: など）は今のところ使わないため無視する。
    }

    fn dispatch(&mut self) -> SseEvent {
        let payload = self.data_lines.join("\n");
        self.data_lines.clear();
        if payload == "[DONE]" {
            SseEvent::Done
        } else {
            SseEvent::Data(payload)
        }
    }
}

/// 1 行分のバイト列（末尾の `\n`・`\r` を含みうる）から、行末を取り除いた文字列を作る。
fn decode_line(line_bytes: &[u8]) -> String {
    let mut trimmed = line_bytes;
    if trimmed.last() == Some(&b'\n') {
        trimmed = &trimmed[..trimmed.len() - 1];
    }
    if trimmed.last() == Some(&b'\r') {
        trimmed = &trimmed[..trimmed.len() - 1];
    }
    String::from_utf8_lossy(trimmed).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_all(decoder: &mut SseDecoder, chunks: &[&[u8]]) -> Vec<SseEvent> {
        let mut events = Vec::new();
        for chunk in chunks {
            events.extend(decoder.push(chunk));
        }
        events
    }

    #[test]
    fn 一行の_data_を空行でディスパッチする() {
        let mut decoder = SseDecoder::default();
        let events = push_all(&mut decoder, &[b"data: {\"a\":1}\n\n"]);
        assert_eq!(events, vec![SseEvent::Data("{\"a\":1}".to_string())]);
    }

    #[test]
    fn 複数行の_data_は_改行で結合する() {
        let mut decoder = SseDecoder::default();
        let events = push_all(&mut decoder, &[b"data: line1\ndata: line2\n\n"]);
        assert_eq!(events, vec![SseEvent::Data("line1\nline2".to_string())]);
    }

    #[test]
    fn コロンで始まる行はコメントとして無視する() {
        let mut decoder = SseDecoder::default();
        let events = push_all(
            &mut decoder,
            &[b": keep-alive\ndata: hello\n\n: another comment\n"],
        );
        assert_eq!(events, vec![SseEvent::Data("hello".to_string())]);
    }

    #[test]
    fn 空行だけでは何も起きない() {
        let mut decoder = SseDecoder::default();
        let events = push_all(&mut decoder, &[b"\n\n\n"]);
        assert_eq!(events, Vec::<SseEvent>::new());
    }

    #[test]
    fn done_マーカーを認識する() {
        let mut decoder = SseDecoder::default();
        let events = push_all(&mut decoder, &[b"data: [DONE]\n\n"]);
        assert_eq!(events, vec![SseEvent::Done]);
    }

    #[test]
    fn 行の途中で分割されたチャンクでも正しく組み立てる() {
        let mut decoder = SseDecoder::default();
        let events = push_all(
            &mut decoder,
            &[b"da", b"ta: hel", b"lo world", b"\n", b"\n"],
        );
        assert_eq!(events, vec![SseEvent::Data("hello world".to_string())]);
    }

    #[test]
    fn マルチバイト文字の途中で分割されたチャンクでも正しく組み立てる() {
        // 「本」は UTF-8 で 3 バイト（E6 9C AC）。その境界でチャンクを分割する。
        let text = "本文です";
        let bytes = text.as_bytes();
        let split_at = 2; // 1 バイト目・2 バイト目の間で切る（先頭文字の途中）。
        let mut decoder = SseDecoder::default();
        let mut line = Vec::new();
        line.extend_from_slice(b"data: ");
        line.extend_from_slice(bytes);
        line.extend_from_slice(b"\n\n");

        let (first, rest) = line.split_at(split_at + b"data: ".len());
        let events = push_all(&mut decoder, &[first, rest]);
        assert_eq!(events, vec![SseEvent::Data(text.to_string())]);
    }

    #[test]
    fn 複数イベントが同じチャンクにまとまって届いても順番どおりに返す() {
        let mut decoder = SseDecoder::default();
        let events = push_all(
            &mut decoder,
            &[b"data: one\n\ndata: two\n\ndata: [DONE]\n\n"],
        );
        assert_eq!(
            events,
            vec![
                SseEvent::Data("one".to_string()),
                SseEvent::Data("two".to_string()),
                SseEvent::Done,
            ]
        );
    }

    #[test]
    fn crlf_改行にも対応する() {
        let mut decoder = SseDecoder::default();
        let events = push_all(&mut decoder, &[b"data: hello\r\n\r\n"]);
        assert_eq!(events, vec![SseEvent::Data("hello".to_string())]);
    }

    #[test]
    fn finish_は末尾の改行なしの行を最終イベントとして処理する() {
        let mut decoder = SseDecoder::default();
        decoder.push(b"data: partial");
        let events = decoder.finish();
        assert_eq!(events, vec![SseEvent::Data("partial".to_string())]);
    }
}
