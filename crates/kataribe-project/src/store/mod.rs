//! 作品フォルダの外に出られない、安全なファイル操作。
//!
//! 読み書きはすべて [`RelPath`] を経由する。実際のパスへ解決する際、既存の祖先を
//! canonicalize してルートの内側にあることを確認するため、シンボリックリンクなどで
//! 実体がルートの外を指していても拒否できる。

use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::error::ProjectError;
use crate::layout;
use crate::path::RelPath;

mod batch;
mod folder;
#[cfg(test)]
mod test_support;

pub use batch::{EntryCondition, PendingChange, PendingWrite};
pub use folder::FolderFile;

/// 内容のハッシュ（SHA-256、16 進小文字）。書き込み時の競合検出に使う。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, type = "string"))]
pub struct ContentHash(String);

impl ContentHash {
    fn of(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            use fmt::Write as _;
            let _ = write!(hex, "{byte:02x}");
        }
        Self(hex)
    }

    /// 16 進小文字の文字列表現。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        let is_valid = raw.len() == 64
            && raw
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
        if is_valid {
            Ok(Self(raw))
        } else {
            Err(serde::de::Error::custom(
                "ハッシュの形式が正しくありません（64 桁の 16 進小文字）",
            ))
        }
    }
}

/// 読み込んだテキストファイルの内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFile {
    /// BOM を除き、改行を LF に正規化した内容。
    pub content: String,
    /// `content` の SHA-256 ハッシュ。
    pub hash: ContentHash,
}

/// 書き込みが許される条件。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum WriteCondition {
    /// 常に許可する。
    #[default]
    Any,
    /// ファイルが存在しない場合のみ許可する（新規作成）。
    Absent,
    /// 現在の内容のハッシュが一致する場合のみ許可する。
    Matches(ContentHash),
}

/// バックアップを作る頻度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackupMode {
    /// [`BackupPolicy`] に従い間引きながら作る（既定）。
    #[default]
    Throttled,
    /// 上書きのたびに必ず作る。
    Always,
    /// 作らない。
    Never,
}

/// `write_text` の挙動をまとめた設定。
#[derive(Debug, Clone, Default)]
pub struct WriteOptions {
    /// 書き込みを許す条件。
    pub condition: WriteCondition,
    /// バックアップの作り方。
    pub backup: BackupMode,
}

/// バックアップの間引き方針。
#[derive(Debug, Clone, Copy)]
pub struct BackupPolicy {
    /// `Throttled` のとき、同じファイルの直前のバックアップからこの間隔を空ける。
    pub min_interval: Duration,
    /// 1 ファイルあたり残す件数。
    pub keep: usize,
}

impl Default for BackupPolicy {
    fn default() -> Self {
        Self {
            min_interval: Duration::from_mins(10),
            keep: 20,
        }
    }
}

/// 現在時刻の取得元。テストで固定時刻に差し替えられるようにするための境界。
pub trait Clock: fmt::Debug + Send + Sync {
    /// 現在時刻。
    fn now(&self) -> Timestamp;
}

/// システム時計を使う既定の [`Clock`]。
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}

/// [`ProjectStore::open_with`] に渡す設定。
#[derive(Debug, Clone)]
pub struct ProjectStoreOptions {
    /// バックアップの間引き方針。
    pub backup_policy: BackupPolicy,
    /// 現在時刻の取得元。
    pub clock: Arc<dyn Clock>,
}

impl Default for ProjectStoreOptions {
    fn default() -> Self {
        Self {
            backup_policy: BackupPolicy::default(),
            clock: Arc::new(SystemClock),
        }
    }
}

/// ディレクトリ内の項目の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirEntryKind {
    /// ファイル。
    File,
    /// ディレクトリ。
    Dir,
}

/// `list_dir` が返す一項目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// 項目のパス。
    pub path: RelPath,
    /// ファイルかディレクトリか。
    pub kind: DirEntryKind,
}

/// 1 件のバックアップ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    /// バックアップ本体のパス。
    pub path: RelPath,
    /// 作られた日時。
    pub created_at: Timestamp,
}

/// 一時的なロック（ウイルス対策ソフトや検索インデクサなど）を数回だけ再試行する間隔。
const RETRY_DELAYS_MS: [u64; 5] = [20, 50, 100, 200, 400];

/// 作品フォルダの外に出られない、安全なファイル操作を提供する。
#[derive(Debug)]
pub struct ProjectStore {
    root: PathBuf,
    canonical_root: PathBuf,
    backup_policy: BackupPolicy,
    clock: Arc<dyn Clock>,
    /// 「条件の確認から置き換えまで」をこのプロセス内で排他する。自動保存と変更案の適用が
    /// 同時に同じファイルへ来ても、両方が同じ内容を前提に通って片方の変更が消えないようにする。
    write_lock: Mutex<()>,
}

impl ProjectStore {
    /// `root` を開く。存在するディレクトリでなければならない。
    pub fn open(root: &Path) -> Result<Self, ProjectError> {
        Self::open_with(root, ProjectStoreOptions::default())
    }

    /// バックアップ方針や時計を指定して開く。
    pub fn open_with(root: &Path, options: ProjectStoreOptions) -> Result<Self, ProjectError> {
        let canonical_root =
            root.canonicalize()
                .map_err(|source| ProjectError::RootNotOpenable {
                    root: root.to_path_buf(),
                    source,
                })?;
        if !canonical_root.is_dir() {
            let source = std::io::Error::other("フォルダではありません");
            return Err(ProjectError::RootNotOpenable {
                root: root.to_path_buf(),
                source,
            });
        }
        Ok(Self {
            root: root.to_path_buf(),
            canonical_root,
            backup_policy: options.backup_policy,
            clock: options.clock,
            write_lock: Mutex::new(()),
        })
    }

    /// このストアが開いている作品フォルダのルート。
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// ルートを正規化した絶対パス。書き方の違う同じフォルダ（大文字小文字・区切り文字）を
    /// 同じ作品として見分けるのに使う。
    #[must_use]
    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    /// テキストファイルを読み込む。存在しなければエラー。
    pub fn read_text(&self, path: &RelPath) -> Result<TextFile, ProjectError> {
        self.read_text_opt(path)?
            .ok_or_else(|| ProjectError::NotFound { path: path.clone() })
    }

    /// テキストファイルを読み込む。存在しなければ `Ok(None)`。
    ///
    /// UTF-8 として読み、先頭の BOM を除き、改行を LF に正規化する。
    pub fn read_text_opt(&self, path: &RelPath) -> Result<Option<TextFile>, ProjectError> {
        self.read_bytes_opt(path)?
            .map(|bytes| decode_text(path, &bytes))
            .transpose()
    }

    /// ファイルの中身をそのまま読む。存在しなければ `Ok(None)`。
    fn read_bytes_opt(&self, path: &RelPath) -> Result<Option<Vec<u8>>, ProjectError> {
        match fs::read(self.resolve(path)?) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(ProjectError::Io {
                path: path.clone(),
                source,
            }),
        }
    }

    /// テキストファイルを書き込む。
    ///
    /// 条件に合わなければ何も書かずに `Conflict` エラーを返す。内容が現在と完全に同じなら、
    /// 書き込みもバックアップも行わずに成功する。書き込みは同じディレクトリの一時ファイルに
    /// 書いてから置き換えるため、途中でクラッシュしても元のファイルは壊れない。
    pub fn write_text(
        &self,
        path: &RelPath,
        content: &str,
        options: WriteOptions,
    ) -> Result<ContentHash, ProjectError> {
        self.write_one(
            &PendingWrite {
                path,
                content,
                condition: options.condition,
            },
            options.backup,
        )
    }

    fn lock_writes(&self) -> MutexGuard<'_, ()> {
        self.write_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// パスが存在するかどうか。フォルダ外に解決される場合は `false` を返す。
    #[must_use]
    pub fn exists(&self, path: &RelPath) -> bool {
        self.resolve(path).is_ok_and(|resolved| resolved.exists())
    }

    /// ディレクトリの中身を名前順で列挙する。`.` で始まる項目は除く。
    pub fn list_dir(&self, path: &RelPath) -> Result<Vec<DirEntry>, ProjectError> {
        let resolved = self.resolve(path)?;
        let read_dir = fs::read_dir(&resolved).map_err(|source| ProjectError::Io {
            path: path.clone(),
            source,
        })?;

        let mut entries = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(|source| ProjectError::Io {
                path: path.clone(),
                source,
            })?;
            let Ok(name) = entry.file_name().into_string() else {
                tracing::warn!(parent = %path, "ファイル名が UTF-8 として解釈できないため無視しました");
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let child_path = match path.join(&name) {
                Ok(child) => child,
                Err(source) => {
                    tracing::warn!(parent = %path, name = %name, error = %source, "使えない名前のため無視しました");
                    continue;
                }
            };
            let metadata = entry.metadata().map_err(|source| ProjectError::Io {
                path: child_path.clone(),
                source,
            })?;
            let kind = if metadata.is_dir() {
                DirEntryKind::Dir
            } else {
                DirEntryKind::File
            };
            entries.push(DirEntry {
                path: child_path,
                kind,
            });
        }
        entries.sort_by(|a, b| a.path.file_name().cmp(b.path.file_name()));
        Ok(entries)
    }

    /// `from` を `to` へ移動する。`to` が既に存在する場合はエラー。
    pub fn rename(&self, from: &RelPath, to: &RelPath) -> Result<(), ProjectError> {
        let _guard = self.lock_writes();
        let resolved_from = self.resolve(from)?;
        if !resolved_from.exists() {
            return Err(ProjectError::NotFound { path: from.clone() });
        }
        let resolved_to = self.resolve(to)?;
        if resolved_to.exists() {
            return Err(ProjectError::AlreadyExists { path: to.clone() });
        }
        create_parent_dir(&resolved_to, to)?;
        rename_with_retry(&resolved_from, &resolved_to).map_err(|source| ProjectError::Io {
            path: to.clone(),
            source,
        })
    }

    /// ファイルを削除する代わりに `.kataribe/trash/<日時>/<相対パス>` へ移す。
    pub fn remove(&self, path: &RelPath) -> Result<(), ProjectError> {
        let _guard = self.lock_writes();
        let resolved = self.resolve(path)?;
        if !resolved.exists() {
            return Err(ProjectError::NotFound { path: path.clone() });
        }
        self.trash_only(path, resolved)
    }

    /// `path` のバックアップを新しい順に列挙する。
    pub fn backups(&self, path: &RelPath) -> Result<Vec<Backup>, ProjectError> {
        let backup_dir = backup_dir_of(path)?;
        Ok(self
            .list_parsed_backups(&backup_dir)?
            .into_iter()
            .map(|(path, parsed)| Backup {
                path,
                created_at: parsed.timestamp,
            })
            .collect())
    }

    /// `backup_dir` の中身を解釈し、新しい順（同時刻なら衝突回避の連番が大きい順）に並べて返す。
    /// ファイル名を解釈できない項目は無視する。
    fn list_parsed_backups(
        &self,
        backup_dir: &RelPath,
    ) -> Result<Vec<(RelPath, ParsedBackupName)>, ProjectError> {
        if !self.exists(backup_dir) {
            return Ok(Vec::new());
        }
        let entries = self.list_dir(backup_dir)?;
        let mut result = Vec::with_capacity(entries.len());
        for entry in entries {
            if entry.kind != DirEntryKind::File {
                continue;
            }
            if let Some(parsed) = parse_backup_file_name(entry.path.file_stem()) {
                result.push((entry.path, parsed));
            } else {
                tracing::warn!(path = %entry.path, "バックアップのファイル名を解釈できませんでした");
            }
        }
        result.sort_by_key(|(_, parsed)| {
            std::cmp::Reverse((parsed.timestamp, parsed.collision_counter))
        });
        Ok(result)
    }

    /// クレート内部向け: ディレクトリを作る（無ければ）。作品の初期骨格作成に使う。
    pub(crate) fn ensure_dir(&self, path: &RelPath) -> Result<(), ProjectError> {
        let resolved = self.resolve(path)?;
        fs::create_dir_all(resolved).map_err(|source| ProjectError::Io {
            path: path.clone(),
            source,
        })
    }

    fn maybe_backup(&self, path: &RelPath, mode: BackupMode) -> Result<(), ProjectError> {
        match mode {
            BackupMode::Never => Ok(()),
            BackupMode::Always => self.create_backup(path),
            BackupMode::Throttled => {
                let existing = self.backups(path)?;
                let due = match existing.first() {
                    None => true,
                    Some(latest) => self.throttle_elapsed(latest.created_at),
                };
                if due {
                    self.create_backup(path)
                } else {
                    Ok(())
                }
            }
        }
    }

    fn throttle_elapsed(&self, latest: Timestamp) -> bool {
        let now = self.clock.now();
        let elapsed_ms = now.as_millisecond().saturating_sub(latest.as_millisecond());
        let min_interval_ms =
            i64::try_from(self.backup_policy.min_interval.as_millis()).unwrap_or(i64::MAX);
        elapsed_ms >= min_interval_ms
    }

    fn create_backup(&self, path: &RelPath) -> Result<(), ProjectError> {
        let resolved = self.resolve(path)?;
        let raw = match fs::read(&resolved) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(ProjectError::Io {
                    path: path.clone(),
                    source,
                });
            }
        };

        let backup_dir = backup_dir_of(path)?;
        let stamp = timestamp_stamp(self.clock.now());
        let backup_path = self.unique_backup_path(&backup_dir, &stamp, path.extension())?;
        let resolved_backup = self.resolve(&backup_path)?;
        if let Some(parent) = resolved_backup.parent() {
            fs::create_dir_all(parent).map_err(|source| ProjectError::Io {
                path: backup_path.clone(),
                source,
            })?;
        }
        fs::write(&resolved_backup, &raw).map_err(|source| ProjectError::Io {
            path: backup_path.clone(),
            source,
        })?;

        self.prune_backups(&backup_dir)
    }

    /// `stamp`（+ 拡張子）を基本の名前としつつ、同じ名前が既にあれば `-1`, `-2`, … を付けて
    /// 衝突を避けたバックアップのパスを返す。同じミリ秒に複数回バックアップしても、
    /// 前のバックアップを上書きしないようにするため。
    fn unique_backup_path(
        &self,
        backup_dir: &RelPath,
        stamp: &str,
        extension: Option<&str>,
    ) -> Result<RelPath, ProjectError> {
        let mut collision_counter = 0u32;
        loop {
            let file_name = format_backup_file_name(stamp, collision_counter, extension);
            let candidate = backup_dir.join(&file_name)?;
            if !self.exists(&candidate) {
                return Ok(candidate);
            }
            collision_counter += 1;
        }
    }

    fn prune_backups(&self, backup_dir: &RelPath) -> Result<(), ProjectError> {
        let parsed = self.list_parsed_backups(backup_dir)?;
        for (stale_path, _) in parsed.into_iter().skip(self.backup_policy.keep) {
            let resolved = self.resolve(&stale_path)?;
            fs::remove_file(resolved).map_err(|source| ProjectError::Io {
                path: stale_path,
                source,
            })?;
        }
        Ok(())
    }

    /// `path` を実際のファイルシステム上のパスへ解決する。
    ///
    /// 既存の祖先を canonicalize し、ルートの内側にあることを確認する。これにより、
    /// シンボリックリンクなどで実体がルートの外を指していても拒否できる。
    fn resolve(&self, path: &RelPath) -> Result<PathBuf, ProjectError> {
        let mut candidate = self.root.clone();
        for segment in path.as_str().split('/') {
            candidate.push(segment);
        }
        self.ensure_within_root(&candidate, path)?;
        Ok(candidate)
    }

    fn ensure_within_root(&self, candidate: &Path, path: &RelPath) -> Result<(), ProjectError> {
        let mut probe = candidate.to_path_buf();
        while !probe.exists() {
            if !probe.pop() {
                break;
            }
        }
        let canonical_existing = probe.canonicalize().map_err(|source| ProjectError::Io {
            path: path.clone(),
            source,
        })?;
        if canonical_existing.starts_with(&self.canonical_root) {
            Ok(())
        } else {
            Err(ProjectError::PathEscapesRoot { path: path.clone() })
        }
    }
}

fn backup_dir_of(path: &RelPath) -> Result<RelPath, ProjectError> {
    Ok(RelPath::new(&format!(
        "{}/{}",
        layout::BACKUPS_DIR,
        path.as_str()
    ))?)
}

fn create_parent_dir(target: &Path, path: &RelPath) -> Result<(), ProjectError> {
    let Some(parent) = target.parent() else {
        return Err(ProjectError::PathEscapesRoot { path: path.clone() });
    };
    fs::create_dir_all(parent).map_err(|source| ProjectError::Io {
        path: path.clone(),
        source,
    })
}

/// `target` と同じフォルダの一時ファイルで、`target` を置き換える。
fn atomic_write(target: &Path, path: &RelPath, bytes: &[u8]) -> Result<(), ProjectError> {
    let Some(parent) = target.parent() else {
        return Err(ProjectError::PathEscapesRoot { path: path.clone() });
    };
    persist_temp_file(stage_temp_file(parent, path, bytes)?, target, path)
}

/// `dir` の一時ファイルに `bytes` を書き、ディスクへ確実に書き出す。`path` は、失敗を知らせるための対象のパス。
fn stage_temp_file(
    dir: &Path,
    path: &RelPath,
    bytes: &[u8],
) -> Result<NamedTempFile, ProjectError> {
    let io_error = |source| ProjectError::Io {
        path: path.clone(),
        source,
    };
    let mut temp = NamedTempFile::new_in(dir).map_err(io_error)?;
    temp.write_all(bytes).map_err(io_error)?;
    temp.as_file().sync_all().map_err(io_error)?;
    Ok(temp)
}

/// `from` を `to` へ移す。一時的なロックは短く再試行する。
fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    retry_transient((), |()| fs::rename(from, to).map_err(|error| ((), error)))
}

/// 一時ファイルで `target` を置き換える。一時的なロックは短く再試行する。
fn persist_temp_file(
    temp: NamedTempFile,
    target: &Path,
    path: &RelPath,
) -> Result<(), ProjectError> {
    retry_transient(temp, |temp| {
        temp.persist(target).map_err(|err| (err.file, err.error))
    })
    .map(|_| ())
    .map_err(|source| ProjectError::Io {
        path: path.clone(),
        source,
    })
}

/// `operation` を呼ぶ。一時的なエラー（[`is_transient_lock_error`]）であれば、
/// [`RETRY_DELAYS_MS`] の間隔を空けながら数回だけ再試行する。
///
/// `operation` は、失敗したときに次の試行へ引き継ぐ状態（`state`、例えばまだ
/// 使える一時ファイルのハンドル）を、発生したエラーと一緒に返す。
fn retry_transient<S, T>(
    mut state: S,
    mut operation: impl FnMut(S) -> Result<T, (S, std::io::Error)>,
) -> Result<T, std::io::Error> {
    let mut attempt = 0usize;
    loop {
        match operation(state) {
            Ok(value) => return Ok(value),
            Err((recovered_state, error))
                if attempt < RETRY_DELAYS_MS.len() && is_transient_lock_error(&error) =>
            {
                std::thread::sleep(Duration::from_millis(RETRY_DELAYS_MS[attempt]));
                attempt += 1;
                state = recovered_state;
            }
            Err((_, error)) => return Err(error),
        }
    }
}

fn check_condition_against(
    current: Option<&TextFile>,
    path: &RelPath,
    condition: &WriteCondition,
) -> Result<(), ProjectError> {
    let ok = match condition {
        WriteCondition::Any => true,
        WriteCondition::Absent => current.is_none(),
        WriteCondition::Matches(expected) => {
            current.is_some_and(|current| &current.hash == expected)
        }
    };
    if ok {
        Ok(())
    } else {
        Err(ProjectError::Conflict { path: path.clone() })
    }
}

fn is_transient_lock_error(error: &std::io::Error) -> bool {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        return true;
    }
    // Windows: ERROR_ACCESS_DENIED(5) / ERROR_SHARING_VIOLATION(32) / ERROR_LOCK_VIOLATION(33).
    // ウイルス対策ソフトや検索インデクサが一時的にファイルを開いている場合に起こる。
    matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

/// ファイルの中身を UTF-8 のテキストとして読み、正規化する。
fn decode_text(path: &RelPath, bytes: &[u8]) -> Result<TextFile, ProjectError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| ProjectError::InvalidUtf8 { path: path.clone() })?;
    let content = normalize_text(text);
    let hash = ContentHash::of(content.as_bytes());
    Ok(TextFile { content, hash })
}

/// BOM を除き、CRLF・CR を LF に正規化する。
///
/// [`ProjectStore`] が読み書きするテキストはすべてこれを通す。作品フォルダの外から来る
/// テキスト（CLI の `--idea-file` など）を同じ規則で扱いたい場合にも、ここを再利用する
/// （改行の正規化を作品フォルダ内外で重複させないため）。
#[must_use]
pub fn normalize_text(text: &str) -> String {
    let without_bom = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut normalized = String::with_capacity(without_bom.len());
    let mut chars = without_bom.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            normalized.push('\n');
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

/// `YYYYMMDD-HHMMSS-mmm` 形式のタイムスタンプ文字列（UTC）。
fn timestamp_stamp(now: Timestamp) -> String {
    format!(
        "{}-{:03}",
        now.strftime("%Y%m%d-%H%M%S"),
        now.subsec_millisecond()
    )
}

/// [`timestamp_stamp`] の逆変換。
fn parse_timestamp_stamp(stem: &str) -> Option<Timestamp> {
    if stem.len() != 19 {
        return None;
    }
    let year: i16 = stem.get(0..4)?.parse().ok()?;
    let month: i8 = stem.get(4..6)?.parse().ok()?;
    let day: i8 = stem.get(6..8)?.parse().ok()?;
    if stem.get(8..9)? != "-" {
        return None;
    }
    let hour: i8 = stem.get(9..11)?.parse().ok()?;
    let minute: i8 = stem.get(11..13)?.parse().ok()?;
    let second: i8 = stem.get(13..15)?.parse().ok()?;
    if stem.get(15..16)? != "-" {
        return None;
    }
    let millisecond: i32 = stem.get(16..19)?.parse().ok()?;

    let datetime = DateTime::new(
        year,
        month,
        day,
        hour,
        minute,
        second,
        millisecond * 1_000_000,
    )
    .ok()?;
    let zoned = datetime.to_zoned(TimeZone::UTC).ok()?;
    Some(zoned.timestamp())
}

/// バックアップのファイル名（拡張子を除いた部分）を解釈した結果。
#[derive(Debug, Clone, Copy)]
struct ParsedBackupName {
    timestamp: Timestamp,
    /// 同じミリ秒に複数回バックアップしたときに付く連番。衝突していなければ 0。
    collision_counter: u32,
}

/// バックアップのファイル名を組み立てる。`collision_counter` が 0 より大きければ
/// `-<collision_counter>` を付けて、同じミリ秒に作った前のバックアップと名前が衝突しないようにする。
fn format_backup_file_name(stamp: &str, collision_counter: u32, extension: Option<&str>) -> String {
    let stem = if collision_counter == 0 {
        stamp.to_string()
    } else {
        format!("{stamp}-{collision_counter}")
    };
    match extension {
        Some(ext) => format!("{stem}.{ext}"),
        None => stem,
    }
}

/// [`format_backup_file_name`] の逆変換（ファイル名から拡張子を除いた部分を受け取る）。
fn parse_backup_file_name(stem: &str) -> Option<ParsedBackupName> {
    if stem.len() < 19 {
        return None;
    }
    let (timestamp_part, suffix) = stem.split_at(19);
    let collision_counter = if suffix.is_empty() {
        0
    } else {
        suffix.strip_prefix('-')?.parse().ok()?
    };
    let timestamp = parse_timestamp_stamp(timestamp_part)?;
    Some(ParsedBackupName {
        timestamp,
        collision_counter,
    })
}

#[cfg(test)]
mod tests {

    use tempfile::TempDir;

    use super::test_support::{FixedClock, rel, write};
    use super::*;

    fn open_store_with_clock(
        dir: &TempDir,
        clock: Arc<FixedClock>,
    ) -> (ProjectStore, Arc<FixedClock>) {
        let options = ProjectStoreOptions {
            backup_policy: BackupPolicy::default(),
            clock: clock.clone(),
        };
        let store = ProjectStore::open_with(dir.path(), options).unwrap();
        (store, clock)
    }

    fn permission_denied() -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "テスト用の権限エラー")
    }

    #[test]
    fn is_transient_lock_error_recognizes_permission_denied_and_windows_codes() {
        assert!(is_transient_lock_error(&permission_denied()));
        assert!(is_transient_lock_error(&std::io::Error::from_raw_os_error(
            5
        ))); // ERROR_ACCESS_DENIED
        assert!(is_transient_lock_error(&std::io::Error::from_raw_os_error(
            32
        ))); // ERROR_SHARING_VIOLATION
        assert!(is_transient_lock_error(&std::io::Error::from_raw_os_error(
            33
        ))); // ERROR_LOCK_VIOLATION
        assert!(!is_transient_lock_error(&std::io::Error::from(
            std::io::ErrorKind::NotFound
        )));
        assert!(!is_transient_lock_error(
            &std::io::Error::from_raw_os_error(2)
        ));
    }

    #[test]
    fn retry_transient_succeeds_after_a_few_transient_failures() {
        let attempts = std::cell::Cell::new(0u32);
        let result = retry_transient((), |()| {
            attempts.set(attempts.get() + 1);
            if attempts.get() < 3 {
                Err(((), permission_denied()))
            } else {
                Ok("成功")
            }
        });
        assert_eq!(result.unwrap(), "成功");
        assert_eq!(attempts.get(), 3, "2 回失敗したあと 3 回目で成功するはず");
    }

    #[test]
    fn retry_transient_gives_up_after_exhausting_retries() {
        let attempts = std::cell::Cell::new(0u32);
        let result: Result<(), std::io::Error> = retry_transient((), |()| {
            attempts.set(attempts.get() + 1);
            Err(((), permission_denied()))
        });
        assert!(result.is_err());
        assert_eq!(
            attempts.get(),
            u32::try_from(RETRY_DELAYS_MS.len()).unwrap() + 1,
            "最初の試行 + 再試行の回数だけ呼ばれるはず"
        );
    }

    #[test]
    fn retry_transient_does_not_retry_non_transient_errors() {
        let attempts = std::cell::Cell::new(0u32);
        let result: Result<(), std::io::Error> = retry_transient((), |()| {
            attempts.set(attempts.get() + 1);
            Err(((), std::io::Error::from(std::io::ErrorKind::NotFound)))
        });
        assert!(result.is_err());
        assert_eq!(
            attempts.get(),
            1,
            "再試行しても無駄なエラーなら 1 回で諦めるはず"
        );
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        let path = rel("concept.md");

        let hash = store
            .write_text(&path, "本文\n", WriteOptions::default())
            .unwrap();
        let read = store.read_text(&path).unwrap();
        assert_eq!(read.content, "本文\n");
        assert_eq!(read.hash, hash);
    }

    #[test]
    fn read_text_opt_returns_none_for_missing_file() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        assert_eq!(store.read_text_opt(&rel("missing.md")).unwrap(), None);
    }

    #[test]
    fn read_text_normalizes_bom_and_crlf() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        let path = rel("concept.md");
        let raw = "\u{feff}一行目\r\n二行目\r三行目\n";
        fs::write(dir.path().join("concept.md"), raw).unwrap();

        let read = store.read_text(&path).unwrap();
        assert_eq!(read.content, "一行目\n二行目\n三行目\n");
    }

    #[test]
    fn write_text_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        let path = rel("characters/rin.md");

        store
            .write_text(&path, "内容", WriteOptions::default())
            .unwrap();
        assert!(dir.path().join("characters/rin.md").is_file());
    }

    #[test]
    fn write_text_is_atomic_and_uses_lf_only() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        let path = rel("concept.md");
        store
            .write_text(&path, "内容\r\n", WriteOptions::default())
            .unwrap();

        let raw = fs::read(dir.path().join("concept.md")).unwrap();
        assert!(!raw.contains(&b'\r'));
        assert!(!raw.starts_with(&[0xEF, 0xBB, 0xBF]));
    }

    #[test]
    fn write_text_absent_condition_rejects_existing_file() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        let path = rel("concept.md");
        store
            .write_text(&path, "初回", WriteOptions::default())
            .unwrap();

        let result = store.write_text(
            &path,
            "二回目",
            WriteOptions {
                condition: WriteCondition::Absent,
                backup: BackupMode::Never,
            },
        );
        assert!(matches!(result, Err(ProjectError::Conflict { .. })));
    }

    #[test]
    fn write_text_matches_condition_detects_conflict() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        let path = rel("concept.md");
        let hash = store
            .write_text(&path, "初回", WriteOptions::default())
            .unwrap();

        // 正しいハッシュなら書き込める。
        let new_hash = store
            .write_text(
                &path,
                "更新後",
                WriteOptions {
                    condition: WriteCondition::Matches(hash.clone()),
                    backup: BackupMode::Never,
                },
            )
            .unwrap();
        assert_ne!(hash, new_hash);

        // 古いハッシュのままだと競合になる。
        let result = store.write_text(
            &path,
            "さらに更新",
            WriteOptions {
                condition: WriteCondition::Matches(hash),
                backup: BackupMode::Never,
            },
        );
        assert!(matches!(result, Err(ProjectError::Conflict { .. })));
    }

    #[test]
    fn write_text_skips_write_and_backup_when_content_is_unchanged() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        let path = rel("concept.md");
        store
            .write_text(&path, "同じ内容\n", WriteOptions::default())
            .unwrap();

        store
            .write_text(
                &path,
                "同じ内容\r\n", // 正規化後は同じ内容になる
                WriteOptions {
                    condition: WriteCondition::Any,
                    backup: BackupMode::Always,
                },
            )
            .unwrap();

        assert_eq!(store.backups(&path).unwrap().len(), 0);
    }

    #[test]
    fn backup_is_created_before_overwrite_with_always_mode() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        let path = rel("concept.md");
        store
            .write_text(&path, "v1", WriteOptions::default())
            .unwrap();
        store
            .write_text(
                &path,
                "v2",
                WriteOptions {
                    condition: WriteCondition::Any,
                    backup: BackupMode::Always,
                },
            )
            .unwrap();

        let backups = store.backups(&path).unwrap();
        assert_eq!(backups.len(), 1);
    }

    #[test]
    fn backup_names_do_not_collide_within_the_same_millisecond() {
        // 時計を固定したまま複数回バックアップする（＝同じミリ秒に重なる状況を再現する）。
        let dir = TempDir::new().unwrap();
        let clock = Arc::new(FixedClock::new(
            Timestamp::from_second(1_700_000_000).unwrap(),
        ));
        let (store, _clock) = open_store_with_clock(&dir, clock);
        let path = rel("concept.md");
        let always = WriteOptions {
            condition: WriteCondition::Any,
            backup: BackupMode::Always,
        };

        store
            .write_text(&path, "v1", WriteOptions::default())
            .unwrap();
        store.write_text(&path, "v2", always.clone()).unwrap();
        store.write_text(&path, "v3", always).unwrap();

        let backups = store.backups(&path).unwrap();
        assert_eq!(
            backups.len(),
            2,
            "同じミリ秒でも前のバックアップを上書きせず両方残るはず"
        );

        let mut contents: Vec<String> = backups
            .iter()
            .map(|backup| store.read_text(&backup.path).unwrap().content)
            .collect();
        contents.sort();
        assert_eq!(contents, vec!["v1".to_string(), "v2".to_string()]);
    }

    #[test]
    fn throttled_backup_skips_within_interval_and_creates_after() {
        let dir = TempDir::new().unwrap();
        let clock = Arc::new(FixedClock::new(
            Timestamp::from_second(1_700_000_000).unwrap(),
        ));
        let (store, clock) = open_store_with_clock(&dir, clock);
        let path = rel("concept.md");

        store
            .write_text(&path, "v1", WriteOptions::default())
            .unwrap();
        store
            .write_text(
                &path,
                "v2",
                WriteOptions {
                    condition: WriteCondition::Any,
                    backup: BackupMode::Throttled,
                },
            )
            .unwrap();
        assert_eq!(
            store.backups(&path).unwrap().len(),
            1,
            "同時刻付近の 2 回目は間引かれるはず"
        );

        clock.advance(11 * 60 * 1000); // 11 分進める（既定の間隔は 10 分）
        store
            .write_text(
                &path,
                "v3",
                WriteOptions {
                    condition: WriteCondition::Any,
                    backup: BackupMode::Throttled,
                },
            )
            .unwrap();
        assert_eq!(
            store.backups(&path).unwrap().len(),
            2,
            "間隔を空ければ新しいバックアップができるはず"
        );
    }

    #[test]
    fn backups_are_pruned_to_keep_count() {
        let dir = TempDir::new().unwrap();
        let clock = Arc::new(FixedClock::new(
            Timestamp::from_second(1_700_000_000).unwrap(),
        ));
        let options = ProjectStoreOptions {
            backup_policy: BackupPolicy {
                min_interval: Duration::from_secs(0),
                keep: 2,
            },
            clock: clock.clone(),
        };
        let store = ProjectStore::open_with(dir.path(), options).unwrap();
        let path = rel("concept.md");

        store
            .write_text(&path, "v1", WriteOptions::default())
            .unwrap();
        for content in ["v2", "v3", "v4"] {
            clock.advance(1000);
            store
                .write_text(
                    &path,
                    content,
                    WriteOptions {
                        condition: WriteCondition::Any,
                        backup: BackupMode::Always,
                    },
                )
                .unwrap();
        }

        let backups = store.backups(&path).unwrap();
        assert_eq!(
            backups.len(),
            2,
            "keep=2 を超えた古いバックアップは消えるはず"
        );
        // 新しい順で返る。
        assert!(backups[0].created_at >= backups[1].created_at);
    }

    #[test]
    fn remove_moves_file_into_trash_instead_of_deleting() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        let path = rel("concept.md");
        store
            .write_text(&path, "内容", WriteOptions::default())
            .unwrap();

        store.remove(&path).unwrap();
        assert!(!store.exists(&path));

        let trash_root = dir.path().join(".kataribe/trash");
        let mut found = false;
        for entry in fs::read_dir(&trash_root).unwrap() {
            let entry = entry.unwrap();
            if entry.path().join("concept.md").is_file() {
                found = true;
            }
        }
        assert!(found, "ゴミ箱に元のファイルが見つかるはず");
    }

    #[test]
    fn rename_rejects_when_destination_exists() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        store
            .write_text(&rel("a.md"), "a", WriteOptions::default())
            .unwrap();
        store
            .write_text(&rel("b.md"), "b", WriteOptions::default())
            .unwrap();

        let result = store.rename(&rel("a.md"), &rel("b.md"));
        assert!(matches!(result, Err(ProjectError::AlreadyExists { .. })));
    }

    #[test]
    fn rename_moves_file() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        store
            .write_text(&rel("a.md"), "内容", WriteOptions::default())
            .unwrap();

        store.rename(&rel("a.md"), &rel("sub/b.md")).unwrap();
        assert!(!store.exists(&rel("a.md")));
        assert_eq!(store.read_text(&rel("sub/b.md")).unwrap().content, "内容");
    }

    #[test]
    fn list_dir_sorts_by_name_and_hides_dotfiles() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        store
            .write_text(&rel("world/b.md"), "b", WriteOptions::default())
            .unwrap();
        store
            .write_text(&rel("world/a.md"), "a", WriteOptions::default())
            .unwrap();
        store
            .write_text(&rel("world/.hidden.md"), "h", WriteOptions::default())
            .unwrap();

        let entries = store.list_dir(&rel("world")).unwrap();
        let names: Vec<&str> = entries.iter().map(|entry| entry.path.file_name()).collect();
        assert_eq!(names, vec!["a.md", "b.md"]);
    }

    #[test]
    fn exists_is_false_for_a_path_that_does_not_exist() {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        assert!(!store.exists(&rel("does/not/exist.md")));
    }

    /// ディレクトリへのシンボリックリンクを作る。Windows で権限が無いなど、作れなければ
    /// `false` を返す（呼び出し側はテストを skip してよい）。
    #[cfg(unix)]
    fn create_dir_symlink(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn create_dir_symlink(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }

    #[test]
    fn symlink_escaping_root_is_rejected() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        // ルートの外に実在するファイルを置く。シンボリックリンク経由なら見えてしまう内容。
        fs::write(outside.path().join("secret.txt"), "外部の内容").unwrap();

        let link_path = dir.path().join("escape");
        if !create_dir_symlink(outside.path(), &link_path) {
            eprintln!("シンボリックリンクを作れないため、このテストは skip します");
            return;
        }

        let store = ProjectStore::open(dir.path()).unwrap();

        // シンボリックリンクの先には実際にファイルがあるが、ルートの外なので存在しないと扱う。
        assert!(!store.exists(&rel("escape/secret.txt")));

        let read_result = store.read_text(&rel("escape/secret.txt"));
        assert!(matches!(
            read_result,
            Err(ProjectError::PathEscapesRoot { .. })
        ));

        let write_result =
            store.write_text(&rel("escape/evil.md"), "内容", WriteOptions::default());
        assert!(matches!(
            write_result,
            Err(ProjectError::PathEscapesRoot { .. })
        ));
    }

    /// 同じ内容を前提にした書き込みが同時に来ても、通るのは 1 つだけ（残りは競合）。
    /// 排他が無いと、どれも同じハッシュで条件を通り、後から書いたものが先の変更を黙って消す。
    #[test]
    fn concurrent_writes_on_the_same_base_let_only_one_through() {
        const WRITERS: usize = 8;
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::open(dir.path()).unwrap();
        let base = write(&store, "concept.md", "元の企画");
        let concept = rel("concept.md");
        let barrier = std::sync::Barrier::new(WRITERS);

        let results: Vec<Result<ContentHash, ProjectError>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..WRITERS)
                .map(|writer| {
                    let (store, concept, base, barrier) = (&store, &concept, &base, &barrier);
                    scope.spawn(move || {
                        barrier.wait();
                        store.write_text(
                            concept,
                            &format!("書き手 {writer} の企画"),
                            WriteOptions {
                                condition: WriteCondition::Matches(base.clone()),
                                backup: BackupMode::Never,
                            },
                        )
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect()
        });

        let succeeded = results.iter().filter(|result| result.is_ok()).count();
        let conflicted = results
            .iter()
            .filter(|result| matches!(result, Err(ProjectError::Conflict { .. })))
            .count();
        assert_eq!((succeeded, conflicted), (1, WRITERS - 1));
    }
}
