//! `store` のテストで共有する補助。

use std::sync::Mutex;

use jiff::Timestamp;

use super::{Clock, ContentHash, ProjectStore, WriteOptions};
use crate::path::RelPath;

/// 指定した時刻で止まり、`advance` で進める時計。
#[derive(Debug)]
pub(super) struct FixedClock(Mutex<Timestamp>);

impl FixedClock {
    pub(super) fn new(timestamp: Timestamp) -> Self {
        Self(Mutex::new(timestamp))
    }

    pub(super) fn advance(&self, millis: i64) {
        let mut guard = self.0.lock().unwrap();
        *guard = Timestamp::from_millisecond(guard.as_millisecond() + millis).unwrap();
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        *self.0.lock().unwrap()
    }
}

pub(super) fn rel(path: &str) -> RelPath {
    RelPath::new(path).unwrap()
}

/// 既定の設定でファイルを書き、書いた内容のハッシュを返す（テストの準備）。
pub(super) fn write(store: &ProjectStore, path: &str, content: &str) -> ContentHash {
    store
        .write_text(&rel(path), content, WriteOptions::default())
        .unwrap()
}

/// ファイルの内容。無ければ `None`。
pub(super) fn read(store: &ProjectStore, path: &str) -> Option<String> {
    store
        .read_text_opt(&rel(path))
        .unwrap()
        .map(|file| file.content)
}
