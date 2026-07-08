//! NEW-157 (feature 012): composable generation-count and/or time-based retention for `FileLog`'s
//! rotated backups, crash-safe and restart-safe, defaulting to today's unchanged
//! single-backup-overwrite behavior when unset (FR-004/FR-005/FR-011/FR-013, SC-003, SC-005).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use truefix_log::{FileLog, FileLogOptions, Log, RetentionPolicy};

fn unique_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "truefix-file-log-retention-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Every file in `dir` whose name starts with `messages.log.` (i.e. every rotated backup).
fn backups(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("messages.log.") && n != "messages.log")
        })
        .collect();
    out.sort();
    out
}

// --- T016: a configured generation count retains at most N prior generations ---

#[tokio::test]
async fn generation_count_retains_at_most_n_prior_generations() {
    let dir = unique_dir("t016-generations");
    let log = FileLog::open_with_options(
        &dir,
        FileLogOptions {
            max_size_bytes: Some(16),
            retention: Some(RetentionPolicy {
                generations: Some(3),
                roll_interval: None,
            }),
            ..FileLogOptions::default()
        },
    )
    .await
    .unwrap();
    for i in 0..40 {
        log.on_outgoing(&format!("line-{i:04}-padding-bytes"));
    }
    log.shutdown().await;

    let found = backups(&dir);
    assert!(
        found.len() <= 3,
        "expected at most 3 retained generations, found {}: {found:?}",
        found.len()
    );
    assert!(!found.is_empty(), "expected at least one rotated backup");

    let _ = fs::remove_dir_all(&dir);
}

// --- T017: a configured time-based roll interval creates a new dated file at the boundary ---

#[tokio::test]
async fn time_based_roll_interval_creates_a_new_dated_file_at_the_boundary() {
    let dir = unique_dir("t017-roll-interval");
    let log = FileLog::open_with_options(
        &dir,
        FileLogOptions {
            retention: Some(RetentionPolicy {
                generations: None,
                roll_interval: Some(Duration::from_millis(50)),
            }),
            ..FileLogOptions::default()
        },
    )
    .await
    .unwrap();
    log.on_outgoing("first-interval-line");
    tokio::time::sleep(Duration::from_millis(120)).await;
    log.on_outgoing("second-interval-line");
    log.shutdown().await;

    let found = backups(&dir);
    assert_eq!(
        found.len(),
        1,
        "expected exactly one dated backup after crossing one interval boundary, found {found:?}"
    );
    let backup_content = fs::read_to_string(&found[0]).unwrap();
    assert!(backup_content.contains("first-interval-line"));
    let active_content = fs::read_to_string(dir.join("messages.log")).unwrap();
    assert!(active_content.contains("second-interval-line"));
    assert!(!active_content.contains("first-interval-line"));

    let _ = fs::remove_dir_all(&dir);
}

// --- T018: generation-count and time-based rotation are composable ---

#[tokio::test]
async fn generation_count_and_roll_interval_are_composable() {
    let dir = unique_dir("t018-composable");
    let log = FileLog::open_with_options(
        &dir,
        FileLogOptions {
            retention: Some(RetentionPolicy {
                generations: Some(2),
                roll_interval: Some(Duration::from_millis(30)),
            }),
            ..FileLogOptions::default()
        },
    )
    .await
    .unwrap();
    for i in 0..5 {
        log.on_outgoing(&format!("interval-{i}"));
        tokio::time::sleep(Duration::from_millis(45)).await;
    }
    log.shutdown().await;

    let found = backups(&dir);
    assert!(
        found.len() <= 2,
        "composable policy must still cap retained backups at the configured generation count; \
         found {} backups: {found:?}",
        found.len()
    );
    assert!(!found.is_empty(), "expected at least one dated backup");

    let _ = fs::remove_dir_all(&dir);
}

// --- T019: no log content already accepted before a rotation is lost or corrupted ---

#[tokio::test]
async fn no_content_is_lost_across_rotation_under_either_policy() {
    let dir = unique_dir("t019-no-loss");
    let log = FileLog::open_with_options(
        &dir,
        FileLogOptions {
            max_size_bytes: Some(100),
            retention: Some(RetentionPolicy {
                generations: Some(10),
                roll_interval: None,
            }),
            ..FileLogOptions::default()
        },
    )
    .await
    .unwrap();
    // Sized so the whole run's content comfortably fits within the 10 retained generations (a
    // handful of rotations, well under the cap) -- this test is about corruption/truncation, not
    // about the (separately-tested, T016) pruning-beyond-the-cap behavior.
    const N: usize = 20;
    for i in 0..N {
        log.on_outgoing(&format!("unique-line-{i:04}"));
    }
    log.shutdown().await;

    let mut all_content = fs::read_to_string(dir.join("messages.log")).unwrap();
    for backup in backups(&dir) {
        all_content.push_str(&fs::read_to_string(backup).unwrap());
    }
    for i in 0..N {
        let marker = format!("unique-line-{i:04}");
        assert!(
            all_content.contains(&marker),
            "line {marker} should still be present somewhere across the active file + retained \
             backups (retained backups may prune older generations, but nothing within the kept \
             window should be corrupted/truncated)"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

// --- T020: a crash mid-rotation leaves exactly one unambiguous active file, no content loss ---

#[tokio::test]
async fn crash_mid_rotation_leaves_exactly_one_unambiguous_active_file() {
    let dir = unique_dir("t020-crash-mid-rotation");
    let log = FileLog::open(&dir).await.unwrap();
    log.on_outgoing("before-crash");
    log.shutdown().await;

    // Simulate a crash between rotate()'s rename() succeeding and its reopen() completing: the
    // active file is gone, only the pre-existing content (now the "backup") remains on disk.
    fs::rename(dir.join("messages.log"), dir.join("messages.log.1")).unwrap();
    assert!(!dir.join("messages.log").exists());

    // The next open() (simulating process restart) must recover cleanly: exactly one active file,
    // no error, and the pre-crash content still intact in its backup.
    let log2 = FileLog::open(&dir).await.unwrap();
    log2.on_outgoing("after-restart");
    log2.shutdown().await;

    assert!(dir.join("messages.log").exists());
    let active = fs::read_to_string(dir.join("messages.log")).unwrap();
    assert!(active.contains("after-restart"));
    let backup = fs::read_to_string(dir.join("messages.log.1")).unwrap();
    assert!(
        backup.contains("before-crash"),
        "content written before the simulated crash must not be lost"
    );

    let _ = fs::remove_dir_all(&dir);
}

// --- T021: a stale roll interval elapsed while stopped rolls once on the first write after restart ---

#[tokio::test]
async fn stale_roll_interval_rolls_once_on_first_write_after_restart() {
    let dir = unique_dir("t021-stale-interval");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("messages.log"), "stale-content-from-before-stop\n").unwrap();
    // Back-date the file's mtime well outside a short test interval, simulating "the process was
    // stopped and restarted after the roll interval elapsed" without waiting real wall-clock time.
    let old = std::time::SystemTime::now() - Duration::from_secs(3600);
    let f = fs::OpenOptions::new()
        .write(true)
        .open(dir.join("messages.log"))
        .unwrap();
    f.set_modified(old).unwrap();
    drop(f);

    let log = FileLog::open_with_options(
        &dir,
        FileLogOptions {
            retention: Some(RetentionPolicy {
                generations: None,
                roll_interval: Some(Duration::from_millis(50)),
            }),
            ..FileLogOptions::default()
        },
    )
    .await
    .unwrap();
    log.on_outgoing("first-write-after-restart");
    log.shutdown().await;

    let found = backups(&dir);
    assert_eq!(
        found.len(),
        1,
        "exactly one roll must happen on the first write after restart, not a gap file per \
         missed interval; found {found:?}"
    );
    let backup_content = fs::read_to_string(&found[0]).unwrap();
    assert!(backup_content.contains("stale-content-from-before-stop"));
    let active = fs::read_to_string(dir.join("messages.log")).unwrap();
    assert!(active.contains("first-write-after-restart"));
    assert!(
        !active.contains("stale-content-from-before-stop"),
        "the active file must not keep writing under the stale interval's name"
    );

    let _ = fs::remove_dir_all(&dir);
}

// --- T022: default (no retention configured) behavior is unchanged from today ---

#[tokio::test]
async fn default_behavior_without_retention_matches_today_exactly() {
    let dir = unique_dir("t022-default-unchanged");
    let log = FileLog::open_with_options(
        &dir,
        FileLogOptions {
            max_size_bytes: Some(32),
            ..FileLogOptions::default()
        },
    )
    .await
    .unwrap();
    for i in 0..10 {
        log.on_outgoing(&format!("8=FIX.4.4|35=D|11=ORDER{i}|10=000"));
    }
    log.shutdown().await;

    let backup = dir.join("messages.log.1");
    assert!(backup.exists(), "expected a single rotated backup file");
    assert!(
        !dir.join("messages.log.2").exists(),
        "unset retention must not introduce a generation cap beyond the single .1 backup"
    );
    let current_len = fs::metadata(dir.join("messages.log")).unwrap().len();
    assert!(current_len < 200, "current log file should stay bounded");

    let _ = fs::remove_dir_all(&dir);
}
