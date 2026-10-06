use std::collections::BTreeSet;

use chrono::DateTime;
use chrono::Datelike;
use chrono::Days;
use chrono::TimeZone;
use chrono::Utc;

#[derive(Clone)]
pub(crate) struct Schedule {
    fields: [BTreeSet<u32>; 5],
    day_wildcard: bool,
    weekday_wildcard: bool,
}

impl Schedule {
    pub(crate) fn parse(expression: &str) -> Result<Self, String> {
        let fields: Vec<_> = expression.split_whitespace().collect();
        if fields.len() != 5 || expression.len() > 100 {
            return Err(
                "Use five cron fields: minute hour day-of-month month day-of-week (UTC).".into(),
            );
        }
        let mut parsed = Self {
            fields: [
                parse_field(fields[0], /*min*/ 0, /*max*/ 59)?,
                parse_field(fields[1], /*min*/ 0, /*max*/ 23)?,
                parse_field(fields[2], /*min*/ 1, /*max*/ 31)?,
                parse_field(fields[3], /*min*/ 1, /*max*/ 12)?,
                parse_field(fields[4], /*min*/ 0, /*max*/ 7)?,
            ],
            day_wildcard: fields[2].starts_with('*'),
            weekday_wildcard: fields[4].starts_with('*'),
        };
        parsed.fields[4] = parsed.fields[4].iter().map(|day| day % 7).collect();
        Ok(parsed)
    }

    /// Find a strictly later UTC minute. Eight years cover the Gregorian leap gap.
    pub(crate) fn next(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let mut day = after.date_naive();
        for _ in 0..=366 * 8 {
            let day_matches = self.fields[2].contains(&day.day());
            let weekday_matches = self.fields[4].contains(&day.weekday().num_days_from_sunday());
            let matches = if self.day_wildcard || self.weekday_wildcard {
                day_matches && weekday_matches
            } else {
                day_matches || weekday_matches
            };
            if matches && self.fields[3].contains(&day.month()) {
                for hour in &self.fields[1] {
                    for minute in &self.fields[0] {
                        let candidate = Utc.from_utc_datetime(&day.and_hms_opt(*hour, *minute, 0)?);
                        if candidate > after {
                            return Some(candidate);
                        }
                    }
                }
            }
            day = day.checked_add_days(Days::new(1))?;
        }
        None
    }
}

fn parse_field(field: &str, min: u32, max: u32) -> Result<BTreeSet<u32>, String> {
    let invalid = || {
        format!(
            "Invalid cron field {field:?}; use numbers {min}-{max}, *, lists, ranges, or steps."
        )
    };
    let mut values = BTreeSet::new();
    for part in field.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => (range, step.parse::<u32>().map_err(|_| invalid())?),
            None => (part, 1),
        };
        if step == 0 {
            return Err(invalid());
        }
        let (start, end) = if range == "*" {
            (min, max)
        } else if let Some((start, end)) = range.split_once('-') {
            (
                start.parse().map_err(|_| invalid())?,
                end.parse().map_err(|_| invalid())?,
            )
        } else {
            let start = range.parse().map_err(|_| invalid())?;
            (start, if part.contains('/') { max } else { start })
        };
        if start < min || end > max || start > end {
            return Err(invalid());
        }
        values.extend((start..=end).step_by(step as usize));
    }
    Ok(values)
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;
