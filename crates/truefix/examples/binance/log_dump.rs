//! `--dump-log`: prints every entry recorded in a redb log database, oldest first.

use anyhow::{Context, Result};

/// Prints every entry recorded in the redb log database at `path`, oldest first. Reads the same
/// three tables `truefix_log::RedbLog` writes: key -> (unix seconds, session, text).
pub fn dump_log_db(path: &std::path::Path) -> Result<()> {
    use redb::{ReadableDatabase, ReadableTable, TableDefinition};

    let db = redb::Database::open(path).with_context(|| {
        format!(
            "opening log database {} (does it exist? is a session still running?)",
            path.display()
        )
    })?;
    let txn = db.begin_read()?;

    // (unix seconds, per-table counter, direction, session, text) -- sorted for a merged timeline.
    let mut rows: Vec<(i64, u64, &'static str, String, String)> = Vec::new();
    for (table_name, label) in [
        ("log_incoming", "RECV"),
        ("log_outgoing", "SENT"),
        ("log_event", "EVENT"),
    ] {
        let def = TableDefinition::<u64, (i64, &str, &str)>::new(table_name);
        let table = txn.open_table(def)?;
        for entry in table.iter()? {
            let (key, value) = entry?;
            let (ts, session, text) = value.value();
            rows.push((
                ts,
                key.value(),
                label,
                session.to_owned(),
                text.replace('\u{1}', "|"),
            ));
        }
    }
    rows.sort();

    for (ts, _, label, session, text) in &rows {
        let when = time::OffsetDateTime::from_unix_timestamp(*ts)
            .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
            .format(&time::format_description::well_known::Rfc3339)?;
        println!("{when} {label:5} [{session}] {text}");
    }
    eprintln!("{} entries", rows.len());
    Ok(())
}
