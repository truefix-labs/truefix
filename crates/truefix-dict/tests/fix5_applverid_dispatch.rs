//! T045/T046 (US1, feature 009, `NEW-06`): the generated `crack_fix50`/`crack_fix50sp1`/
//! `crack_fix50sp2` dispatchers all guarded solely on `BeginString == "FIXT.1.1"` -- since FIX
//! 5.0/SP1/SP2 all carry that same BeginString on the wire, distinguished only by
//! `ApplVerID(1128)`, a FIX 5.0SP2 message would match `crack_fix50` (and every other FIX-5.x
//! `crack_*`) too.

use truefix_core::{Field, Message};
use truefix_dict::fix50::{FIX50MessageHandler, crack_fix50};
use truefix_dict::fix50sp1::{FIX50SP1MessageHandler, crack_fix50sp1};
use truefix_dict::fix50sp2::{FIX50SP2MessageHandler, crack_fix50sp2};

struct NoopHandler;
impl FIX50MessageHandler for NoopHandler {}
impl FIX50SP1MessageHandler for NoopHandler {}
impl FIX50SP2MessageHandler for NoopHandler {}

/// An ExecutionReport (MsgType=8, common across FIX 5.0/SP1/SP2) carrying `BeginString=FIXT.1.1`
/// and the given `ApplVerID`.
fn execution_report(appl_ver_id: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIXT.1.1"));
    m.header.set(Field::string(35, "8"));
    m.header.set(Field::string(1128, appl_ver_id));
    m
}

#[test]
fn crack_fix50_only_dispatches_applverid_7() {
    let mut h = NoopHandler;
    assert!(
        crack_fix50(&execution_report("7"), &mut h),
        "FIX50 (ApplVerID=7) should dispatch"
    );
    assert!(
        !crack_fix50(&execution_report("8"), &mut h),
        "FIX50SP1 (ApplVerID=8) must NOT dispatch via crack_fix50 (NEW-06)"
    );
    assert!(
        !crack_fix50(&execution_report("9"), &mut h),
        "FIX50SP2 (ApplVerID=9) must NOT dispatch via crack_fix50 (NEW-06)"
    );
}

#[test]
fn crack_fix50sp1_only_dispatches_applverid_8() {
    let mut h = NoopHandler;
    assert!(
        !crack_fix50sp1(&execution_report("7"), &mut h),
        "FIX50 (ApplVerID=7) must NOT dispatch via crack_fix50sp1 (NEW-06)"
    );
    assert!(
        crack_fix50sp1(&execution_report("8"), &mut h),
        "FIX50SP1 (ApplVerID=8) should dispatch"
    );
    assert!(
        !crack_fix50sp1(&execution_report("9"), &mut h),
        "FIX50SP2 (ApplVerID=9) must NOT dispatch via crack_fix50sp1 (NEW-06)"
    );
}

#[test]
fn crack_fix50sp2_only_dispatches_applverid_9() {
    let mut h = NoopHandler;
    assert!(
        !crack_fix50sp2(&execution_report("7"), &mut h),
        "FIX50 (ApplVerID=7) must NOT dispatch via crack_fix50sp2 (NEW-06)"
    );
    assert!(
        !crack_fix50sp2(&execution_report("8"), &mut h),
        "FIX50SP1 (ApplVerID=8) must NOT dispatch via crack_fix50sp2 (NEW-06)"
    );
    assert!(
        crack_fix50sp2(&execution_report("9"), &mut h),
        "FIX50SP2 (ApplVerID=9) should dispatch"
    );
}
