//! T073 (US14) — MSSQL log (behind the `mssql` feature, FR-020): entries are persisted via the
//! background writer, mirroring `sql_log.rs`'s pattern. Gated on `DATABASE_URL_MSSQL` being set to
//! a reachable instance (CI provides one via a service container — see
//! `.github/workflows/ci.yml`'s `mssql` job); dev boxes without that service shouldn't fail the
//! suite. Unlike `sql_log.rs` (which verifies via a second `sqlx` connection to the same SQLite
//! file), verification here reconnects with a second `tiberius` client, since that's the only
//! driver this crate pulls in for MSSQL.

#![cfg(feature = "mssql")]

use std::time::Duration;

use tiberius::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;
use truefix_log::{Log, MssqlLog, MssqlLogConfig};

fn parse_url(url: &str) -> Config {
    let rest = url
        .strip_prefix("mssql://")
        .or_else(|| url.strip_prefix("sqlserver://"))
        .expect("mssql:// or sqlserver:// URL");
    let (userinfo, hostpart) = rest.split_once('@').expect("user:password@host/db");
    let (user, password) = userinfo.split_once(':').expect("user:password");
    let (hostport, database) = hostpart.split_once('/').expect("host/database");
    let (host, port) = match hostport.split_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().expect("valid port")),
        None => (hostport, 1433),
    };
    let mut config = Config::new();
    config.host(host);
    config.port(port);
    config.database(database);
    config.authentication(AuthMethod::sql_server(user, password));
    config.trust_cert();
    config
}

#[tokio::test]
async fn mssql_log_persists_messages_and_events_if_available() {
    let Ok(url) = std::env::var("DATABASE_URL_MSSQL") else {
        eprintln!("skipping: DATABASE_URL_MSSQL not set");
        return;
    };

    let config = MssqlLogConfig {
        incoming_table: "t73_log_incoming".to_owned(),
        outgoing_table: "t73_log_outgoing".to_owned(),
        event_table: "t73_log_event".to_owned(),
        ..MssqlLogConfig::new(&url)
    };
    let log = MssqlLog::connect_with_config(config).await.unwrap();
    log.on_incoming("8=FIX.4.4|35=A");
    log.on_outgoing("8=FIX.4.4|35=0");
    log.on_event("logged on");

    // Allow the background writer to flush.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let tiberius_config = parse_url(&url);
    let addr = tokio::net::lookup_host(tiberius_config.get_addr())
        .await
        .unwrap()
        .next()
        .unwrap();
    let tcp = TcpStream::connect(addr).await.unwrap();
    tcp.set_nodelay(true).unwrap();
    let mut client = Client::connect(tiberius_config, tcp.compat_write())
        .await
        .unwrap();

    let incoming: i32 = client
        .query("SELECT COUNT(*) FROM t73_log_incoming", &[])
        .await
        .unwrap()
        .into_row()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    let outgoing: i32 = client
        .query("SELECT COUNT(*) FROM t73_log_outgoing", &[])
        .await
        .unwrap()
        .into_row()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    let events: i32 = client
        .query("SELECT COUNT(*) FROM t73_log_event", &[])
        .await
        .unwrap()
        .into_row()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();

    assert_eq!(incoming, 1, "one incoming message logged");
    assert_eq!(outgoing, 1, "one outgoing message logged");
    assert_eq!(events, 1, "one event logged");
}
