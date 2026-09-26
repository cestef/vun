use tracing::Level;

use crate::Filter;

#[test]
fn the_most_specific_directive_decides() {
    let filter = Filter::parse("info, vun_sema=trace, vun_sema::int=error, bogus=loud");
    assert!(filter.enabled("vun_sema::check", Level::TRACE));
    assert!(!filter.enabled("vun_sema::int", Level::WARN));
    assert!(filter.enabled("vun_syntax", Level::INFO));
    assert!(!filter.enabled("vun_syntax", Level::DEBUG));
    assert!(!Filter::default().enabled("anything", Level::ERROR), "nothing without a filter");
    assert!(
        Filter::parse("debug,warn").enabled("x", Level::WARN)
            && !Filter::parse("debug,warn").enabled("x", Level::INFO),
        "later wins"
    );
}

#[test]
fn own_time_excludes_the_activities_run_inside() {
    use std::time::Duration;

    use crate::{Activity, Timings};

    const PAUSE: Duration = Duration::from_millis(5);
    let (outer, inner) = (Activity::stage("outer"), Activity::query("inner"));
    drop(outer.start());
    assert!(Timings::report().is_none(), "off until enabled");
    Timings::enable();
    {
        let _outer = outer.start();
        std::thread::sleep(PAUSE);
        for _ in 0..2 {
            let _inner = inner.start();
            std::thread::sleep(PAUSE);
        }
    }
    let report = Timings::report().unwrap();
    let row = |activity| *report.rows.iter().find(|r| r.activity == activity).unwrap();
    let (outer, inner) = (row(outer), row(inner));
    assert_eq!((outer.runs, inner.runs), (1, 2), "runs before enabling are not counted");
    assert!(inner.own >= 2 * PAUSE && inner.own == inner.total);
    assert!(outer.own >= PAUSE && outer.own < outer.total && outer.total >= outer.own + inner.own);
}
