//! HTTP date parsing shared by the connection's date-valued headers.

use crate::api_error;
use diagnostics::EmuError;

const DAY_MILLIS: i64 = 86_400_000;
const SHORT_WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const LONG_WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

pub(super) fn parse_http_date(value: &str, now_millis: i64) -> Result<i64, EmuError> {
    parse(value, now_millis)
        .ok_or_else(|| api_error("network-header", "invalid or unsupported HTTP date"))
}

fn parse(value: &str, now_millis: i64) -> Option<i64> {
    // RFC 2616 section 3.3.1 requires all three formats. Borrow a fixed number
    // of fields, preserving the existing tolerance for whitespace without an
    // allocation proportional to the number of tokens in an invalid header.
    let mut fields = value.split_ascii_whitespace();
    let fields: [_; 7] = std::array::from_fn(|_| fields.next());
    let (day, month, year, clock, short_year) = match fields {
        [
            Some(weekday),
            Some(day),
            Some(month),
            Some(year),
            Some(clock),
            Some("GMT"),
            None,
        ] if SHORT_WEEKDAYS.contains(&weekday.strip_suffix(',')?) => {
            (day, month, year, clock, false)
        }
        [
            Some(weekday),
            Some(date),
            Some(clock),
            Some("GMT"),
            None,
            None,
            None,
        ] if LONG_WEEKDAYS.contains(&weekday.strip_suffix(',')?) => {
            let mut parts = date.split('-');
            let [Some(day), Some(month), Some(year), None] = std::array::from_fn(|_| parts.next())
            else {
                return None;
            };
            (day, month, year, clock, true)
        }
        [
            Some(weekday),
            Some(month),
            Some(day),
            Some(clock),
            Some(year),
            None,
            None,
        ] if SHORT_WEEKDAYS.contains(&weekday) => (day, month, year, clock, false),
        _ => return None,
    };
    let day = decimal(day, 1, 2)?;
    let month = i64::try_from(
        [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]
        .iter()
        .position(|candidate| *candidate == month)?,
    )
    .ok()?
        + 1;
    let width = if short_year { 2 } else { 4 };
    let mut year = decimal(year, width, width)?;
    let mut clock = clock.split(':');
    let [Some(hour), Some(minute), Some(second), None] = std::array::from_fn(|_| clock.next())
    else {
        return None;
    };
    let (hour, minute, second) = (
        decimal(hour, 1, 2)?,
        decimal(minute, 1, 2)?,
        decimal(second, 1, 2)?,
    );
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let time = (hour * 3_600 + minute * 60 + second.min(59)) * 1_000;
    if short_year {
        // RFC 2616 section 19.3: a date over 50 years in the future belongs
        // to the preceding century. Compare calendar fields at the boundary
        // rather than approximating a year with a fixed number of seconds.
        let (current_year, current_month, current_day) =
            civil_from_days(now_millis.div_euclid(DAY_MILLIS));
        let last_year = current_year + 50;
        year += last_year.div_euclid(100) * 100;
        if (year, month, day, time)
            > (
                last_year,
                current_month,
                current_day,
                now_millis.rem_euclid(DAY_MILLIS),
            )
        {
            year -= 100;
        }
    }
    if !(1..=9_999).contains(&year) || !(1..=days_in_month(year, month)).contains(&day) {
        return None;
    }
    // Four-digit years fit comfortably in a Java long, including dates
    // before the Unix epoch. No saturating arithmetic is necessary here.
    Some(days_from_civil(year, month, day) * DAY_MILLIS + time)
}

fn decimal(value: &str, minimum: usize, maximum: usize) -> Option<i64> {
    ((minimum..=maximum).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        _ => 31,
    }
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = shifted_month + if shifted_month < 10 { 3 } else { -9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
#[path = "../../../../tests/unit/gcf/http_date.rs"]
mod tests;
