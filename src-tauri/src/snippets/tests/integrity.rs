//! File: snippets/tests/integrity.rs
//! Purpose: Protect the integrity of the snippet file and the expander that reads it: folder deletion keeps the clash rules, exports restore folder behavior at any supported size, folder references and ids hold, use counts are never guessed or lost, reads are one committed state, a storage failure rolls a whole import back, a failed include types nothing, and damaged rows are refused by the schema or named.
//! Contents: scenario tests over real SQLite files, with a second connection or a held lock where a race or a failure is needed; count tests share one lock; the sync retry decision is tested without a file because the owed uses are process-wide.

use std::io::Read;
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;

use super::helpers::{draft, notepad, open_db, save};
use crate::lock::lock;
use crate::snippets::bulk::add_all;
use crate::snippets::check::{MAX_BODY, MAX_NAME, MAX_PATTERNS, MAX_PROGRAM, MAX_PROGRAMS, MAX_SNIPPETS, MAX_TRIGGER, MAX_TRIGGERS};
use crate::snippets::expand::template::expand;
use crate::snippets::expand::{count, expand_reading, retrying, Index, SMALL_BODY};
use crate::snippets::model::{Draft, Folder, FolderDraft, Insert, Snippet};
use crate::snippets::policy::Policy;
use crate::snippets::store::{self, Failure};
use crate::snippets::{folders, transfer};

/// Held by every test that claims use-count writes: the owed uses are one list for the process.
static COUNTS: Mutex<()> = Mutex::new(());

/// A second connection to the file at `dir` that waits only 20 ms for a lock, so a held lock fails fast. Input: the folder. Returns the connection.
fn impatient(dir: &Path) -> Connection {
    store::open_waiting(&dir.join("snippets.db"), Duration::from_millis(20)).expect("second connection")
}

/// A connection holding the exclusive lock on the file at `dir` until it commits: no other connection can read or write meanwhile. Input: the folder. Returns the connection.
fn holding_lock(dir: &Path) -> Connection {
    let holder = Connection::open(dir.join("snippets.db")).expect("holder");
    holder.execute_batch("BEGIN EXCLUSIVE").expect("lock");
    holder
}

/// A folder draft. Inputs: name, switch and the programs it is limited to. Returns the draft.
fn folder(name: &str, enabled: bool, only: &[&str]) -> FolderDraft {
    FolderDraft { name: name.into(), enabled, only_apps: only.iter().map(|p| p.to_string()).collect(), skip_apps: vec![] }
}

/// Saves `d` into folder `id`. Inputs: the connection, the folder and the draft. Returns what `save` returns.
fn save_in(conn: &Connection, id: i64, d: Draft) -> Result<(), String> {
    save(conn, None, &Draft { folder_id: Some(id), ..d })
}

/// The id of the stored snippet named `name`. Inputs: the connection and the name. Returns the id.
fn id_of(conn: &Connection, name: &str) -> i64 {
    store::list(conn).expect("list").into_iter().find(|s| s.name == name).expect(name).id
}

/// Deleting a disabled folder unfiles its snippets and so switches them on: when one would clash with a live trigger (equal or prefix) or pass the pattern limit, the deletion is refused and the folder and its snippets stay as they were; without a clash it goes through.
#[test]
fn deleting_a_disabled_folder_whose_snippets_would_clash_is_refused() {
    let (_dir, conn) = open_db();
    let policy = Policy::default();
    save(&conn, None, &draft("Live", &[":sig"], "x")).expect("live");
    for (name, trigger) in [("Equal", ":sig"), ("Prefix", ":si")] {
        let old = folders::save_folder(&conn, None, &folder(name, false, &[]), &policy).expect("folder");
        save_in(&conn, old, draft(name, &[trigger], "y")).expect("a disabled folder's trigger does not clash");
        let Err(message) = folders::remove_folder(&conn, old, &policy) else { panic!("{name} deleted") };
        assert!(message.contains("clashes with"), "{message}");
        assert!(folders::folders(&conn).expect("folders").iter().any(|f| f.id == old), "{name} is still there");
        assert_eq!(store::get(&conn, id_of(&conn, name)).expect("get").expect("kept").folder_id, Some(old));
    }
    (0..MAX_PATTERNS).for_each(|i| save(&conn, None, &draft(&format!("P{i}"), &[&format!(r"re:k{i}\d")], "x")).expect("pattern"));
    let many = folders::save_folder(&conn, None, &folder("Patterns", false, &[]), &policy).expect("folder");
    save_in(&conn, many, draft("One more", &[r"re:extra\d"], "x")).expect("disabled pattern");
    let Err(message) = folders::remove_folder(&conn, many, &policy) else { panic!("51 patterns switched on") };
    assert!(message.contains(&format!("{MAX_PATTERNS}")), "{message}");
    let harmless = folders::save_folder(&conn, None, &folder("Harmless", false, &[]), &policy).expect("folder");
    save_in(&conn, harmless, draft("Fine", &[":fine"], "z")).expect("snippet");
    folders::remove_folder(&conn, harmless, &policy).expect("no clash");
    assert_eq!(store::get(&conn, id_of(&conn, "Fine")).expect("get").expect("kept").folder_id, None);
}

/// A whole library exported and imported into an empty one comes back with each folder's switch and programs, so a disabled folder's snippets stay off (and may share a live trigger) and a program-bound folder's stay bound; an older export without the folder list still imports, its folders made enabled everywhere, so the shared trigger is then skipped as a clash.
#[test]
fn a_full_export_restores_folder_switches_and_programs() {
    let (_dir, conn) = open_db();
    let policy = Policy::default();
    let work = folders::save_folder(&conn, None, &folder("Work", true, &["Slack\tslack"]), &policy).expect("work");
    let old = folders::save_folder(&conn, None, &folder("Old", false, &[]), &policy).expect("old");
    save_in(&conn, work, draft("Standup", &[":standup"], "Yesterday")).expect("standup");
    save_in(&conn, old, draft("Old sig", &[":sig"], "Old")).expect("old sig");
    save(&conn, None, &draft("Sig", &[":sig"], "New")).expect("sig");
    let text = transfer::export(&store::list(&conn).expect("list"), &folders::folders(&conn).expect("folders"));

    let parsed = transfer::parse(&text).expect("reads back");
    let (_empty_dir, empty) = open_db();
    assert_eq!(add_all(&empty, &parsed.entries, &parsed.folders, &policy), Ok((3, 0)));
    let shape =
        |c: &Connection| folders::folders(c).expect("folders").into_iter().map(|f| (f.name, f.enabled, f.only_apps)).collect::<Vec<_>>();
    assert_eq!(shape(&empty), shape(&conn));
    let live: Vec<_> = store::effective_all(&empty, &policy).expect("live").into_iter().map(|e| (e.trigger, e.only_apps)).collect();
    assert_eq!(live, [(":sig".to_string(), vec![]), (":standup".to_string(), vec!["Slack\tslack".to_string()])]);

    let mut older: serde_json::Value = serde_json::from_str(&text).expect("json");
    older.as_object_mut().expect("object").remove("folders");
    let parsed = transfer::parse(&older.to_string()).expect("an older export reads");
    let (_older_dir, fresh) = open_db();
    assert_eq!(add_all(&fresh, &parsed.entries, &parsed.folders, &policy), Ok((2, 0)), "the old shared trigger clashes");
    assert!(folders::folders(&fresh).expect("folders").iter().all(|f| f.enabled && f.only_apps.is_empty()));
}

/// The import limit covers the largest export Cue can write: one snippet and its folder at every limit, each character the longest JSON can make it, times the snippet limit, fits.
#[test]
fn the_import_limit_covers_the_largest_export() {
    let worst = |n: usize| "\u{1}".repeat(n);
    let programs = vec![format!("{}\tx", worst(MAX_PROGRAM - 2)); MAX_PROGRAMS];
    let snippet = Snippet {
        id: 1,
        name: worst(MAX_NAME),
        triggers: vec!["\"".repeat(MAX_TRIGGER); MAX_TRIGGERS],
        body: worst(MAX_BODY),
        uses: i64::MAX,
        last_used: i64::MAX,
        folder_id: Some(1),
        enabled: true,
        insert: Insert::Rich,
        only_apps: programs.clone(),
        skip_apps: programs.clone(),
    };
    let group = Folder { id: 1, name: worst(MAX_NAME), enabled: false, only_apps: programs.clone(), skip_apps: programs };
    let one = transfer::export(&[snippet], &[group]).len() as u64;
    assert!(one * MAX_SNIPPETS as u64 <= transfer::MAX_FILE_BYTES, "{one} bytes per entry");
}

/// A library whose export is larger than the old 5 MiB import limit (263 texts at the length limit) is written to a file, read back from one open handle and imported whole into an empty library.
#[test]
fn an_export_above_the_old_limit_imports_whole() {
    let (dir, conn) = open_db();
    let entries: Vec<_> = (0..263).map(|i| (draft(&format!("Big {i}"), &[], &"a".repeat(MAX_BODY)), String::new())).collect();
    assert_eq!(add_all(&conn, &entries, &[], &Policy::default()), Ok((263, 0)));
    let text = transfer::export(&store::list(&conn).expect("list"), &[]);
    assert!(text.len() > 5 * 1024 * 1024, "{} bytes", text.len());
    let path = dir.path().join("export.json");
    std::fs::write(&path, &text).expect("write");
    let read = transfer::read_limited(std::fs::File::open(&path).expect("open"), transfer::MAX_FILE_BYTES).expect("read");
    let parsed = transfer::parse(&read).expect("parse");
    let (_empty_dir, empty) = open_db();
    assert_eq!(add_all(&empty, &parsed.entries, &parsed.folders, &Policy::default()), Ok((263, 0)));
}

/// An import reads at most the limit from its one handle: an endless reader stops one byte past it and is refused; exactly the limit is read; bytes that are not text are refused.
#[test]
fn an_import_read_stops_at_the_limit() {
    assert_eq!(transfer::read_limited(std::io::repeat(b'a'), 1024).err().as_deref(), Some("This file is too large to be a snippets file."));
    assert_eq!(transfer::read_limited(std::io::repeat(b'a').take(1024), 1024).map(|t| t.len()), Ok(1024));
    assert!(transfer::read_limited(&[0xff_u8, 0xfe][..], 1024).is_err());
}

/// A snippet saved into a folder that does not exist (a stale page) is refused; the file refuses one stored by hand; deleting a folder unfiles its snippets through the foreign key.
#[test]
fn a_snippet_never_points_at_a_missing_folder() {
    let (_dir, conn) = open_db();
    let policy = Policy::default();
    let gone = folders::save_folder(&conn, None, &folder("Gone", true, &[]), &policy).expect("folder");
    folders::remove_folder(&conn, gone, &policy).expect("delete");
    let Err(message) = save_in(&conn, gone, draft("Stale", &[":stale"], "x")) else { panic!("saved into a deleted folder") };
    assert_eq!(message, "That folder no longer exists. Reopen the Snippets page.");
    assert!(store::list(&conn).expect("list").is_empty());
    assert!(conn.execute("INSERT INTO snippets (name, body, folder_id) VALUES ('a', 'b', 999)", []).is_err(), "foreign key enforced");
    let kept = folders::save_folder(&conn, None, &folder("Kept", true, &[]), &policy).expect("folder");
    save_in(&conn, kept, draft("Inside", &[":in"], "x")).expect("snippet");
    folders::remove_folder(&conn, kept, &policy).expect("delete");
    assert_eq!(store::get(&conn, id_of(&conn, "Inside")).expect("get").expect("kept").folder_id, None);
}

/// A deleted snippet's or folder's id is never handed out again, so a stale edit, a late use count or a stale folder change cannot land on a newcomer: the edit and the folder change fail and recreate nothing, the count changes nothing.
#[test]
fn deleted_ids_are_never_reused() {
    let (_dir, conn) = open_db();
    let policy = Policy::default();
    save(&conn, None, &draft("First", &[":one"], "x")).expect("first");
    save(&conn, None, &draft("Last", &[":two"], "x")).expect("last");
    let last = id_of(&conn, "Last");
    store::remove(&conn, last).expect("delete");
    save(&conn, None, &draft("Newcomer", &[":three"], "x")).expect("newcomer");
    let newcomer = id_of(&conn, "Newcomer");
    assert!(newcomer > last, "{newcomer} reuses {last}");
    assert!(save(&conn, Some(last), &draft("Stale", &[":four"], "x")).is_err());
    store::bump(&conn, last).expect("a late count is not an error");
    assert_eq!(store::get(&conn, newcomer).expect("get").expect("newcomer").uses, 0);

    let (a, b) = (folder("A", true, &[]), folder("B", true, &[]));
    folders::save_folder(&conn, None, &a, &policy).expect("a");
    let b_id = folders::save_folder(&conn, None, &b, &policy).expect("b");
    folders::remove_folder(&conn, b_id, &policy).expect("delete b");
    let c_id = folders::save_folder(&conn, None, &folder("C", true, &[]), &policy).expect("c");
    assert!(c_id > b_id, "{c_id} reuses {b_id}");
    assert_eq!(folders::save_folder(&conn, Some(b_id), &b, &policy), Err("That folder no longer exists. Reopen the Snippets page.".into()));
    assert_eq!(folders::folders(&conn).expect("folders").len(), 2, "the stale change made no folder");
}

/// `{counter}` reads wait for a claimed use-count write; a write that fails (the file locked by another connection) stays owed, and counter reads are refused until it is written, then see it counted once.
#[test]
fn counter_reads_wait_for_claimed_and_owed_uses() {
    let _counts = lock(&COUNTS);
    let (dir, conn) = open_db();
    save(&conn, None, &draft("Count", &[":n"], "No. {counter}")).expect("snippet");
    let id = id_of(&conn, "Count");
    let quick = impatient(dir.path());
    let bump = |id| store::bump(&quick, id);
    let uses = || -> Result<i64, String> { Ok(store::get(&quick, id).map_err(String::from)?.expect("saved").uses + 1) };

    let claim = count::claim();
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            let mine = impatient(dir.path());
            count::read(|_| Ok(()), || -> Result<i64, String> { Ok(store::get(&mine, id).map_err(String::from)?.expect("saved").uses + 1) })
        });
        claim.write(id, |id| store::bump(&conn, id)).expect("written");
        assert_eq!(reader.join().expect("reader"), Ok(2), "the read waited for the claimed write");
    });

    let holder = holding_lock(dir.path());
    assert!(count::claim().write(id, bump).is_err(), "the locked file refuses the write");
    let refused = count::read(bump, uses).expect_err("a counter read while a use is owed");
    assert!(refused.contains("{counter}"), "{refused}");
    holder.execute_batch("COMMIT").expect("unlock");
    assert_eq!(count::read(bump, uses), Ok(3), "the owed use is written once, then read");
    assert_eq!(count::read(bump, uses), Ok(3), "and only once");
}

/// A use counted while the expander's index is being read (as `expand::publish` reads it, through `count::swap`) does not wait for that read, and is not lost: the use lands after the read's snapshot and on the old index, so the swap reads again before it puts the index in place, and include precedence then matches a fresh read of the file.
#[test]
fn a_use_counted_during_an_index_read_neither_waits_nor_is_lost() {
    let _counts = lock(&COUNTS);
    let (dir, conn) = open_db();
    save(&conn, None, &draft("Zeta", &[":zeta"], "from zeta")).expect("zeta");
    save(&conn, None, &draft("alpha", &["Zeta"], "from alpha")).expect("alpha");
    let (zeta, alpha) = (id_of(&conn, "Zeta"), id_of(&conn, "alpha"));
    for used in [zeta, zeta, alpha] {
        store::bump(&conn, used).expect("bump");
    }
    let running = Mutex::new(Arc::new(Index::read(&conn).expect("index")));
    let mut reads = 0;
    let (index, gate) = count::swap(|| {
        reads += 1;
        let index = Index::read(&impatient(dir.path()))?;
        if reads == 1 {
            let (done, finished) = mpsc::channel();
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    let writer = impatient(dir.path());
                    let counted = count::claim().write(alpha, |id| {
                        store::bump(&writer, id)?;
                        lock(&running).counted(id);
                        Ok(())
                    });
                    done.send(counted).expect("send");
                });
                let counted = finished.recv_timeout(Duration::from_secs(2)).expect("the use waited for the index read");
                counted.expect("counted");
            });
        }
        Ok::<_, Failure>(index)
    })
    .expect("swap");
    *lock(&running) = Arc::new(index);
    drop(gate);
    assert_eq!(reads, 2, "the use landed during the first read, so the swap read again");
    let fresh = Index::read(&conn).expect("index").include("zeta", &|_| None);
    assert_eq!(fresh.as_deref(), Some("from alpha"));
    assert_eq!(lock(&running).include("zeta", &|_| None), fresh, "the swapped index kept the use");
}

/// The expander's sync tries a read once more, after the pause, only when another program held the file locked (SQLITE_BUSY); damage or a refusal is returned at once, and a second lock is returned as the failure.
#[test]
fn only_a_locked_file_is_read_again_after_the_pause() {
    let busy = || Failure::Sqlite(rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY), None));
    let run = |mut replies: Vec<Result<(), Failure>>| {
        let (mut attempts, mut paused) = (0, false);
        let done = retrying(
            || {
                attempts += 1;
                replies.remove(0)
            },
            || paused = true,
        );
        (done.map_err(|e| e.is_busy()), attempts, paused)
    };
    assert_eq!(run(vec![Err(busy()), Ok(())]), (Ok(()), 2, true), "a passing lock is read again");
    assert_eq!(run(vec![Err(busy()), Err(busy())]), (Err(true), 2, true), "a lock that stays is reported");
    assert_eq!(run(vec![Err(Failure::Refused("damaged".into()))]), (Err(false), 1, false), "a refusal is not retried");
    assert_eq!(run(vec![Ok(())]), (Ok(()), 1, false));
}

/// The reads of one snapshot see one committed state: a folder deletion on another connection cannot commit between reading the snippets and reading the folders (it waits, and here gives up), and goes through once the snapshot ends.
#[test]
fn a_snapshot_cannot_be_torn_by_a_folder_deletion() {
    let (dir, conn) = open_db();
    let policy = Policy::default();
    let work = folders::save_folder(&conn, None, &folder("Work", true, &[]), &policy).expect("folder");
    save_in(&conn, work, draft("Standup", &[":standup"], "x")).expect("snippet");
    let other = impatient(dir.path());
    let (snippets, groups, removed) = store::snapshot(&conn, |c| {
        let snippets = store::list(c)?;
        let removed = folders::remove_folder(&other, work, &policy);
        Ok::<_, Failure>((snippets, folders::folders(c)?, removed))
    })
    .expect("snapshot");
    assert!(removed.is_err(), "the deletion committed inside the snapshot");
    assert_eq!((snippets[0].folder_id, groups.len()), (Some(work), 1));
    folders::remove_folder(&other, work, &policy).expect("after the snapshot");
    assert!(folders::folders(&conn).expect("folders").is_empty());
}

/// A storage failure while an import makes a folder (a real SQLite trigger aborts that insert) fails the whole import: nothing it added before is kept, and the failure is returned, not counted as a skipped entry.
#[test]
fn a_storage_failure_rolls_the_whole_import_back() {
    let (_dir, conn) = open_db();
    conn.execute_batch(
        "CREATE TRIGGER boom BEFORE INSERT ON folders WHEN NEW.name = 'Boom' BEGIN SELECT RAISE(ABORT, 'disk trouble'); END;",
    )
    .expect("trigger");
    let entries = vec![
        (draft("A", &[":a"], "x"), String::new()),
        (draft("B", &[":b"], "x"), "Fine".to_string()),
        (draft("C", &[":c"], "x"), "Boom".to_string()),
        (draft("D", &[":d"], "x"), String::new()),
    ];
    let Err(message) = add_all(&conn, &entries, &[], &Policy::default()) else { panic!("the import succeeded") };
    assert!(message.contains("disk trouble"), "{message}");
    assert!(store::list(&conn).expect("list").is_empty());
    assert!(folders::folders(&conn).expect("folders").is_empty());
}

/// An included text that cannot be read (the file locked by another connection) refuses the whole expansion instead of typing the include as written; a missing include stays literal text; once the file is free the include is typed.
#[test]
fn a_failed_include_read_types_nothing() {
    let (dir, conn) = open_db();
    let address = "12 Main St ".repeat(SMALL_BODY / 8);
    save(&conn, None, &draft("Address", &[":addr"], &address)).expect("address");
    let index = Index::read(&conn).expect("index");
    let quick = impatient(dir.path());
    let read = |id: i64| store::get(&quick, id).map(|s| s.map(|s| s.body)).map_err(String::from);
    let typed = |body: &str| expand_reading(&index, body, &read, |_, find| Ok(expand(body, &notepad(find)))).map(|e| e.text);

    let holder = holding_lock(dir.path());
    let refused = typed("Hello {snippet:Address}").expect_err("an unread include was typed");
    assert!(refused.starts_with("Snippets:"), "{refused}");
    holder.execute_batch("COMMIT").expect("unlock");
    assert_eq!(typed("Hello {snippet:Nobody}"), Ok("Hello {snippet:Nobody}".to_string()));
    assert_eq!(typed("Hello {snippet:Address}"), Ok(format!("Hello {address}")));
}

/// The schema refuses rows outside their domain (a switch not 0 or 1, a negative count, a blank name, a text that is not text); a row that got in anyway (checks switched off by hand) is named by its id when read, and reading it changes nothing.
#[test]
fn damaged_rows_are_refused_or_named() {
    let (dir, conn) = open_db();
    for sql in [
        "INSERT INTO snippets (name, body, enabled) VALUES ('a', 'b', 2)",
        "INSERT INTO snippets (name, body, uses) VALUES ('a', 'b', -1)",
        "INSERT INTO snippets (name, body, last_used) VALUES ('a', 'b', -5)",
        "INSERT INTO snippets (name, body) VALUES ('  ', 'b')",
        "INSERT INTO snippets (name, body) VALUES ('a', X'00')",
        "INSERT INTO folders (name, enabled) VALUES ('F', 3)",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
    conn.execute_batch("PRAGMA ignore_check_constraints = ON; INSERT INTO snippets (id, name, body) VALUES (42, X'00', 'b');")
        .expect("hand edit");
    let before = std::fs::read(dir.path().join("snippets.db")).expect("bytes");
    let message = String::from(store::list(&conn).expect_err("a damaged row was read"));
    assert!(message.contains("row 42"), "{message}");
    assert!(String::from(Index::read(&conn).err().expect("index refused")).contains("row 42"));
    assert_eq!(std::fs::read(dir.path().join("snippets.db")).expect("bytes"), before, "reading changed the file");
}
