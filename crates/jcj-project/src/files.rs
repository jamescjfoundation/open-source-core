// SPDX-License-Identifier: MIT OR Apache-2.0
//! Projects on disk: opening, saving, backups and the recovery file.
//!
//! Every write goes through craft-atomic, so a failed or interrupted write never leaves a file
//! half-written, and every path passes the automation file gateway (so an agent confined to some
//! folders cannot read or write outside them).
//!
//! - **Backups** are copies of the saved file in a backups folder the app chooses, named
//!   `<name> <YYYY-MM-DD HH.MM.SS> UTC.<ext>`; the oldest beyond the number to keep are removed.
//!   The time is UTC so the names sort and never repeat across a clock change; a second backup in
//!   the same second gets a number, and an existing file is never replaced.
//! - **The recovery file** sits next to the project file (`.<file name>.recovery`): auto-saves
//!   can write it without touching the user's file, [`recovery`] finds one left by a crash, and a
//!   successful [`save`] removes it.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use jcj_upstream::engine::file_access;

use crate::{App, Error, Limits, Project, Result};

/// Backups kept by default, and at most.
pub const DEFAULT_BACKUPS: usize = 1_000;
/// The most backups [`backup`] keeps of one file.
pub const MAX_BACKUPS: usize = 10_000;

fn shown(path: &Path) -> String {
    path.display().to_string()
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |source| Error::Io { path: path.to_path_buf(), source }
}

fn check_read(path: &Path) -> Result<()> {
    file_access::check_read(&shown(path)).map_err(Error::Refused)
}

pub(crate) fn check_write(path: &Path) -> Result<()> {
    file_access::check_write(&shown(path)).map_err(Error::Refused)
}

/// `path` with extension `ext` (no dot), unless it already has it (any case).
pub fn with_extension(path: &Path, ext: &str) -> PathBuf {
    let ext = ext.trim_start_matches('.');
    let has = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(ext));
    if has { path.to_path_buf() } else { path.with_extension(ext) }
}

/// A file's bytes, refusing one larger than `limit` without reading it all.
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    check_read(path)?;
    let file = std::fs::File::open(path).map_err(io(path))?;
    let meta = file.metadata().map_err(io(path))?;
    if !meta.is_file() {
        return Err(Error::Invalid(format!("{}: not a file", shown(path))));
    }
    if meta.len() > limit {
        return Err(Error::TooLarge(format!("{}: the file is larger than {} MB", shown(path), limit >> 20)));
    }
    let mut out = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
    file.take(limit.saturating_add(1)).read_to_end(&mut out).map_err(io(path))?;
    if out.len() as u64 > limit {
        return Err(Error::TooLarge(format!("{}: the file is larger than {} MB", shown(path), limit >> 20)));
    }
    Ok(out)
}

/// Write `bytes` to `path` atomically, through the file gateway.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    check_write(path)?;
    jcj_upstream::atomic::atomic_write(path, bytes).map_err(io(path))
}

/// Open the project file at `path`.
pub fn open(path: &Path, app: &App) -> Result<Project> {
    open_with(path, app, &Limits::default())
}

/// [`open`] within `limits`.
pub fn open_with(path: &Path, app: &App, limits: &Limits) -> Result<Project> {
    let bytes = read_bounded(path, limits.max_file)?;
    Project::from_bytes_with(&bytes, app, limits).map_err(|e| named(path, e))
}

/// Put the file's name in front of a format error.
fn named(path: &Path, e: Error) -> Error {
    match e {
        Error::Damaged(why) => Error::Damaged(format!("{}: {why}", shown(path))),
        Error::TooLarge(why) => Error::TooLarge(format!("{}: {why}", shown(path))),
        other => other,
    }
}

/// Save `project` at `path` atomically, then remove the recovery file left beside it (it is
/// older than what was just saved).
pub fn save(path: &Path, project: &Project) -> Result<()> {
    let bytes = project.to_bytes()?;
    write_atomic(path, &bytes)?;
    let _ = discard_recovery(path);
    Ok(())
}

// ---- The recovery file ----------------------------------------------------------------------------

/// Where the recovery copy of the project file at `path` is kept: `.<file name>.recovery` in
/// the same folder.
pub fn recovery_path(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Untitled".into());
    path.with_file_name(format!(".{name}.recovery"))
}

/// Write a recovery copy of `project`, whose file is `path`, without touching that file.
/// Returns the recovery file's path.
pub fn write_recovery(path: &Path, project: &Project) -> Result<PathBuf> {
    let rec = recovery_path(path);
    write_atomic(&rec, &project.to_bytes()?)?;
    Ok(rec)
}

/// A recovery copy found beside a project file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovery {
    /// The recovery file.
    pub path: PathBuf,
    /// It was written after the project file was last saved (or there is no project file): it
    /// holds changes the file doesn't.
    pub newer: bool,
}

/// The recovery copy beside the project file at `path`, if there is one.
pub fn recovery(path: &Path) -> Option<Recovery> {
    let rec = recovery_path(path);
    let rec_time = std::fs::metadata(&rec).ok().filter(|m| m.is_file())?.modified().ok();
    let file_time = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
    let newer = match (rec_time, file_time) {
        (Some(r), Some(f)) => r >= f,
        _ => true,
    };
    Some(Recovery { path: rec, newer })
}

/// Open the recovery copy of the project file at `path`.
pub fn open_recovery(path: &Path, app: &App) -> Result<Project> {
    open(&recovery_path(path), app)
}

/// Remove the recovery copy beside the project file at `path`; none there is not an error.
pub fn discard_recovery(path: &Path) -> Result<()> {
    let rec = recovery_path(path);
    check_write(&rec)?;
    match std::fs::remove_file(&rec) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io(&rec)(e)),
    }
}

// ---- Backups ------------------------------------------------------------------------------------

/// The time part of a backup's name: `2026-11-19 14.03.22 UTC`.
pub fn stamp(at: SystemTime) -> String {
    let secs = at.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let dt = chrono::DateTime::from_timestamp(i64::try_from(secs).unwrap_or(0), 0).unwrap_or_default();
    dt.format("%Y-%m-%d %H.%M.%S UTC").to_string()
}

/// The length of a [`stamp`].
const STAMP_LEN: usize = 23;

/// The name and extension backups of `saved` take.
fn backup_parts(saved: &Path) -> (String, String) {
    let stem = saved.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()).unwrap_or("Untitled").to_string();
    let ext = saved.extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    (stem, ext)
}

/// Whether `file` is a backup of a file named `stem` with extension `ext` (`<stem> <stamp>[ n]<ext>`).
fn is_backup_of(file: &str, stem: &str, ext: &str) -> bool {
    let Some(rest) = file.strip_prefix(stem).and_then(|r| r.strip_prefix(' ')) else { return false };
    let Some(rest) = rest.strip_suffix(ext) else { return false };
    // "YYYY-MM-DD HH.MM.SS UTC", then possibly " n" for a second backup in the same second.
    rest.get(..STAMP_LEN).is_some_and(|s| {
        s.char_indices().all(|(i, c)| match i {
            4 | 7 => c == '-',
            10 | 19 => c == ' ',
            13 | 16 => c == '.',
            20..=22 => "UTC".chars().nth(i - 20) == Some(c),
            _ => c.is_ascii_digit(),
        })
    }) && rest
        .get(STAMP_LEN..)
        .is_some_and(|tail| tail.is_empty() || tail.strip_prefix(' ').is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())))
}

/// The backups of the file `saved` in `dir`, oldest first.
pub fn backups_of(dir: &Path, saved: &Path) -> Vec<PathBuf> {
    let (stem, ext) = backup_parts(saved);
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<((String, u64), PathBuf)> = entries
        .filter_map(std::result::Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            if !is_backup_of(&name, &stem, &ext) {
                return None;
            }
            // The stamp sorts as text; a numbered copy comes after the one without a number.
            let tail = name.get(stem.len() + 1..name.len() - ext.len())?;
            let stamp = tail.get(..STAMP_LEN)?.to_string();
            let n = tail.get(STAMP_LEN..).map(str::trim).filter(|n| !n.is_empty()).and_then(|n| n.parse().ok()).unwrap_or(0);
            Some(((stamp, n), e.path()))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.into_iter().map(|(_, p)| p).collect()
}

/// Copy the file just saved at `saved` into `dir` as a dated backup, then remove the oldest
/// backups of the same file beyond `keep` (1 to [`MAX_BACKUPS`]). Returns the new backup's path.
pub fn backup(saved: &Path, dir: &Path, keep: usize, at: SystemTime) -> Result<PathBuf> {
    let bytes = read_bounded(saved, Limits::default().max_file)?;
    backup_bytes(&bytes, saved, dir, keep, at)
}

/// [`backup`] of bytes that are not a file yet (a packed copy of a project folder): write
/// `bytes` into `dir` as a dated backup named after the file `saved`, its name and extension,
/// then remove the oldest backups of the same name beyond `keep`.
pub fn backup_bytes(bytes: &[u8], saved: &Path, dir: &Path, keep: usize, at: SystemTime) -> Result<PathBuf> {
    let keep = keep.clamp(1, MAX_BACKUPS);
    let (stem, ext) = backup_parts(saved);
    check_write(dir)?;
    std::fs::create_dir_all(dir).map_err(io(dir))?;
    let base = format!("{stem} {}", stamp(at));
    let mut written = None;
    for n in 0..100u32 {
        let name = if n == 0 { format!("{base}{ext}") } else { format!("{base} {}{ext}", n + 1) };
        let path = dir.join(name);
        check_write(&path)?;
        match jcj_upstream::atomic::write_new(&path, bytes) {
            Ok(()) => {
                written = Some(path);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io(&path)(e)),
        }
    }
    let written = written.ok_or_else(|| Error::Invalid(format!("{}: too many backups in one second", shown(dir))))?;
    let all = backups_of(dir, saved);
    if all.len() > keep {
        for old in all.get(..all.len() - keep).unwrap_or_default() {
            if *old != written {
                let _ = std::fs::remove_file(old);
            }
        }
    }
    Ok(written)
}

/// Save `project` at `path` ([`save`]), then back the saved file up into `backups` keeping
/// `keep` ([`backup`]). A failed backup does not undo the save: it is returned as the error of
/// the second step, with the file saved.
pub fn save_with_backup(path: &Path, project: &Project, backups: &Path, keep: usize, at: SystemTime) -> Result<PathBuf> {
    save(path, project)?;
    backup(path, backups, keep, at)
}

#[cfg(test)]
mod tests;
