// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use jcj_doc::Document;

use super::*;

const APP: App = App::new("org.example.test", 1);

/// A fresh folder for one test.
fn temp_dir(name: &str) -> PathBuf {
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("jcj-project-files-{name}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn project(title: &str) -> Project {
    let mut d = Document::new();
    d.core.title = title.into();
    Project::with_document(&APP, "Main", d).unwrap()
}

fn title(p: &Project) -> String {
    p.documents().next().map(|(_, _, d)| d.core.title.clone()).unwrap_or_default()
}

#[test]
fn extensions_are_the_apps() {
    assert_eq!(with_extension(Path::new("dir/Draft"), "proj"), Path::new("dir/Draft.proj"));
    assert_eq!(with_extension(Path::new("Draft.PROJ"), ".proj"), Path::new("Draft.PROJ"));
    assert_eq!(with_extension(Path::new("Draft.v2"), "proj"), Path::new("Draft.proj"));
}

#[test]
fn save_and_open() {
    let dir = temp_dir("roundtrip");
    let path = dir.join("Draft.proj");
    save(&path, &project("One")).unwrap();
    assert_eq!(title(&open(&path, &APP).unwrap()), "One");
    assert!(matches!(open(&dir.join("missing.proj"), &APP), Err(Error::Io { .. })));
    std::fs::write(dir.join("junk.proj"), b"not a project").unwrap();
    assert!(matches!(open(&dir.join("junk.proj"), &APP), Err(Error::NotAProject)));
    let tiny = Limits { max_file: 10, ..Limits::default() };
    assert!(matches!(open_with(&path, &APP, &tiny), Err(Error::TooLarge(_))));
    assert!(matches!(open(&dir, &APP), Err(Error::Invalid(_) | Error::Io { .. })), "a folder is not a file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_save_leaves_the_old_file() {
    let dir = temp_dir("failed");
    let path = dir.join("Draft.proj");
    save(&path, &project("One")).unwrap();
    let before = std::fs::read(&path).unwrap();
    // Writing into a folder that does not exist fails without touching anything else.
    assert!(save(&dir.join("no/such/folder/Draft.proj"), &project("Two")).is_err());
    // A project that cannot be written is refused before the file is touched.
    let mut bad = project("Three");
    bad.app_id = "bad id".into();
    assert!(save(&path, &bad).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stamps_are_utc_dates() {
    assert_eq!(stamp(UNIX_EPOCH), "1970-01-01 00.00.00 UTC");
    assert_eq!(stamp(UNIX_EPOCH + Duration::from_secs(951_782_400)), "2000-02-29 00.00.00 UTC");
    assert_eq!(stamp(UNIX_EPOCH + Duration::from_secs(1_795_100_602)), "2026-11-19 15.03.22 UTC");
}

#[test]
fn backups_are_named_kept_and_rotated() {
    let dir = temp_dir("backups");
    let saved = dir.join("Draft.proj");
    save(&saved, &project("One")).unwrap();
    let backups = dir.join("Backups");
    let t0 = UNIX_EPOCH + Duration::from_secs(1_795_100_602);
    let first = backup(&saved, &backups, 3, t0).unwrap();
    assert_eq!(first.file_name().and_then(|n| n.to_str()), Some("Draft 2026-11-19 15.03.22 UTC.proj"));
    assert_eq!(std::fs::read(&first).unwrap(), std::fs::read(&saved).unwrap());
    // Two in the same second get a number; never overwritten.
    let second = backup(&saved, &backups, 3, t0).unwrap();
    assert_eq!(second.file_name().and_then(|n| n.to_str()), Some("Draft 2026-11-19 15.03.22 UTC 2.proj"));
    backup(&saved, &backups, 3, t0 + Duration::from_secs(60)).unwrap();
    // Each backup is the file as saved then.
    save(&saved, &project("Two")).unwrap();
    let newest = backup(&saved, &backups, 3, t0 + Duration::from_secs(120)).unwrap();
    let kept = backups_of(&backups, &saved);
    assert_eq!(kept.len(), 3, "the oldest is removed");
    assert!(!kept.contains(&first) && kept[0] == second && kept.last() == Some(&newest));
    assert_eq!(title(&open(&kept[0], &APP).unwrap()), "One");
    assert_eq!(title(&open(&newest, &APP).unwrap()), "Two");
    // Other files' backups and other files are left alone.
    std::fs::write(backups.join("Other 2026-11-19 15.03.22 UTC.proj"), b"x").unwrap();
    std::fs::write(backups.join("Draft 2026-11-19 15.03.22 UTC.txt"), b"x").unwrap();
    std::fs::write(backups.join("Draft notes.proj"), b"x").unwrap();
    save_with_backup(&saved, &project("Three"), &backups, 1, t0 + Duration::from_secs(180)).unwrap();
    assert_eq!(backups_of(&backups, &saved).len(), 1);
    for other in ["Other 2026-11-19 15.03.22 UTC.proj", "Draft 2026-11-19 15.03.22 UTC.txt", "Draft notes.proj"] {
        assert!(backups.join(other).exists(), "{other}");
    }
    assert!(!is_backup_of("Draft 2026-11-19 15.03.22 UTC x.proj", "Draft", ".proj"));
    assert!(!is_backup_of("Draft2 2026-11-19 15.03.22 UTC.proj", "Draft", ".proj"));
    assert!(is_backup_of("Draft 2026-11-19 15.03.22 UTC 12", "Draft", ""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn numbered_backups_sort_by_number() {
    let dir = temp_dir("numbered");
    let saved = dir.join("A.proj");
    save(&saved, &project("One")).unwrap();
    let t0 = UNIX_EPOCH + Duration::from_secs(1_795_100_602);
    let made: Vec<PathBuf> = (0..11).map(|_| backup(&saved, &dir.join("b"), 100, t0).unwrap()).collect();
    assert_eq!(backups_of(&dir.join("b"), &saved), made, "2, 3, ... 11 after the first, not 10, 11, 2");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_recovery_file_sits_beside_the_project_until_it_is_saved() {
    let dir = temp_dir("recovery");
    let path = dir.join("Draft.proj");
    assert_eq!(recovery_path(&path), dir.join(".Draft.proj.recovery"));
    assert_eq!(recovery(&path), None);
    // An untitled project's first recovery copy: there is no file yet.
    let rec = write_recovery(&path, &project("Unsaved")).unwrap();
    assert_eq!(recovery(&path), Some(Recovery { path: rec.clone(), newer: true }));
    save(&path, &project("Saved")).unwrap();
    assert_eq!(recovery(&path), None, "a save removes the recovery copy");
    write_recovery(&path, &project("Changed")).unwrap();
    let found = recovery(&path).unwrap();
    assert!(found.newer);
    assert_eq!(title(&open_recovery(&path, &APP).unwrap()), "Changed");
    assert_eq!(title(&open(&path, &APP).unwrap()), "Saved", "the project file is untouched");
    discard_recovery(&path).unwrap();
    discard_recovery(&path).unwrap();
    assert_eq!(recovery(&path), None);
    let _ = std::fs::remove_dir_all(&dir);
}
