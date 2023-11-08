use std::collections::HashSet;

#[derive(PartialEq, Eq, Hash)]
enum CronToken<'a> {
    Numeric(u64),
    Alphabetic(&'a str),
}

const MONTHS: &[&str] = &[
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

const DAYS: &[&str] = &["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

fn value_is_allowed(value: &str, allowed_values: &HashSet<CronToken>) -> bool {
    match value.parse::<u64>() {
        Ok(numeric_value) => allowed_values.contains(&CronToken::Numeric(numeric_value)),
        Err(_) => allowed_values.contains(&CronToken::Alphabetic(&value.to_lowercase())),
    }
}

fn validate_range(range: &str, allowed_values: &HashSet<CronToken>) -> bool {
    range == "*"
        || range // TODO: Validate that the last range bound is after the previous one.
            .splitn(2, "-")
            .all(|bound| value_is_allowed(bound, allowed_values))
}

/// A valid step is None or Some positive value
fn validate_step(step: &Option<&str>) -> bool {
    match *step {
        Some(value) => match value.parse::<u64>() {
            Ok(value) => value > 0,
            Err(_) => false,
        },
        None => true,
    }
}

fn validate_steprange(steprange: &str, allowed_values: &HashSet<CronToken>) -> bool {
    let mut steprange_split = steprange.splitn(2, "/");
    let range = match steprange_split.next() {
        Some(range) => range,
        None => {
            return false;
        }
    };
    let range_is_valid = validate_range(range, allowed_values);
    let step = steprange_split.next();

    range_is_valid && validate_step(&step)
}

fn validate_segment(segment: &str, allowed_values: &HashSet<CronToken>) -> bool {
    segment
        .split(",")
        .all(|steprange| validate_steprange(steprange, &allowed_values))
}

pub fn validate(crontab: &str) -> bool {
    let allowed_values = vec![
        (0..60).map(CronToken::Numeric).collect(),
        (0..24).map(CronToken::Numeric).collect(),
        (1..32).map(CronToken::Numeric).collect(),
        (1..13)
            .map(CronToken::Numeric)
            .chain(MONTHS.iter().map(|&month| CronToken::Alphabetic(month)))
            .collect(),
        (0..8)
            .map(CronToken::Numeric)
            .chain(DAYS.iter().map(|&day| CronToken::Alphabetic(day)))
            .collect(),
    ];

    crontab
        .split_whitespace()
        .zip(allowed_values)
        .all(|(segment, allowed_values)| validate_segment(segment, &allowed_values))
}
