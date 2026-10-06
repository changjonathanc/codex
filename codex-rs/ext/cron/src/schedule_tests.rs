use super::Schedule;
use chrono::DateTime;
use chrono::Utc;
use pretty_assertions::assert_eq;

fn time(value: &str) -> DateTime<Utc> {
    value.parse().unwrap()
}

#[test]
fn ranges_steps_lists_and_sunday_alias() {
    for (expression, after, expected) in [
        (
            "*/5 * * * *",
            "2026-10-05T12:05:00Z",
            "2026-10-05T12:10:00Z",
        ),
        (
            "2,17 9-11/2 * * *",
            "2026-10-05T09:18:00Z",
            "2026-10-05T11:02:00Z",
        ),
        ("0 9 * * 7", "2026-10-05T00:00:00Z", "2026-10-11T09:00:00Z"),
        (
            "0 9 * * 1-5",
            "2026-10-09T09:00:00Z",
            "2026-10-12T09:00:00Z",
        ),
        ("0 0 29 2 *", "2096-03-01T00:00:00Z", "2104-02-29T00:00:00Z"),
        // Cron uses OR when both day fields are constrained.
        ("0 9 15 * 1", "2026-10-12T09:00:00Z", "2026-10-15T09:00:00Z"),
    ] {
        assert_eq!(
            Schedule::parse(expression).unwrap().next(time(after)),
            Some(time(expected))
        );
    }
}

#[test]
fn invalid_and_impossible_schedules() {
    for expression in [
        "* * *",
        "60 * * * *",
        "*/0 * * * *",
        "5-2 * * * *",
        "0 0 * * MON",
        "0,,2 * * * *",
    ] {
        assert!(Schedule::parse(expression).is_err(), "{expression}");
    }
    assert_eq!(
        Schedule::parse("0 0 30 2 *")
            .unwrap()
            .next(time("2026-01-01T00:00:00Z")),
        None
    );
}
