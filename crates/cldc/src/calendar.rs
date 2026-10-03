use super::{EmuError, cldc_error};

pub(super) fn calendar_field(millis: i64, field: i32) -> Result<i32, EmuError> {
    const DAY_MILLIS: i64 = 86_400_000;
    let days = millis.div_euclid(DAY_MILLIS);
    let within = millis.rem_euclid(DAY_MILLIS);
    let date = || civil_from_days(days);
    let hour = (within / 3_600_000) as i32;
    match field {
        0 => Ok(i32::from(date().0 >= 1)),
        1 => {
            let year = date().0;
            Ok(if year >= 1 { year } else { 1 - year })
        }
        2 => Ok(date().1 - 1),
        3 | 6 => {
            let day_of_year = (days - days_from_civil(date().0, 1, 1) + 1) as i32;
            Ok(if field == 3 {
                (day_of_year - 1) / 7 + 1
            } else {
                day_of_year
            })
        }
        4 | 8 => Ok((date().2 - 1) / 7 + 1),
        5 => Ok(date().2),
        7 => Ok((days + 4).rem_euclid(7) as i32 + 1),
        9 => Ok(hour / 12),
        10 => Ok(hour % 12),
        11 => Ok(hour),
        12 => Ok((within / 60_000 % 60) as i32),
        13 => Ok((within / 1_000 % 60) as i32),
        14 => Ok((within % 1_000) as i32),
        _ => Err(cldc_error("illegal-argument", "invalid Calendar field")),
    }
}

pub(super) fn calendar_replace_field(millis: i64, field: i32, value: i32) -> Result<i64, EmuError> {
    const DAY_MILLIS: i64 = 86_400_000;
    let days = millis.div_euclid(DAY_MILLIS);
    let within = millis.rem_euclid(DAY_MILLIS);
    let (mut year, mut month, mut day) = civil_from_days(days);
    let mut hour = (within / 3_600_000) as i32;
    let mut minute = (within / 60_000 % 60) as i32;
    let mut second = (within / 1_000 % 60) as i32;
    let mut millisecond = (within % 1_000) as i32;
    match field {
        0 if (0..=1).contains(&value) => {
            let displayed_year = if year >= 1 { year } else { 1 - year };
            year = if value == 1 {
                displayed_year
            } else {
                1_i32
                    .checked_sub(displayed_year)
                    .ok_or_else(|| cldc_error("calendar-overflow", "Calendar era overflow"))?
            };
        }
        1 if value >= 1 => year = if year >= 1 { value } else { 1 - value },
        2 if (0..=11).contains(&value) => month = value + 1,
        3 if value >= 1 => {
            let current = calendar_field(millis, 3)?;
            return replace_calendar_day(millis, i64::from(value - current) * 7);
        }
        4 | 8 if value >= 1 => {
            let weekday = calendar_field(millis, 7)?;
            let first_days = days_from_civil(year, month, 1);
            let first_weekday = (first_days + 4).rem_euclid(7) as i32 + 1;
            let first_match = 1 + (weekday - first_weekday).rem_euclid(7);
            let target = i64::from(first_match) + i64::from(value - 1) * 7;
            if target > i64::from(days_in_month(year, month)) {
                return Err(cldc_error(
                    "illegal-argument",
                    if field == 4 {
                        "invalid week of month"
                    } else {
                        "invalid day of week in month"
                    },
                ));
            }
            day = target as i32;
        }
        5 if value >= 1 => day = value,
        6 if value >= 1 && value <= days_in_year(year) => {
            let target_days = days_from_civil(year, 1, 1) + i64::from(value - 1);
            let (_, target_month, target_day) = civil_from_days(target_days);
            month = target_month;
            day = target_day;
        }
        7 if (1..=7).contains(&value) => {
            let current = calendar_field(millis, 7)?;
            return replace_calendar_day(millis, i64::from(value - current));
        }
        9 if (0..=1).contains(&value) => hour = hour % 12 + value * 12,
        10 if (0..=11).contains(&value) => hour = hour / 12 * 12 + value,
        11 if (0..=23).contains(&value) => hour = value,
        12 if (0..=59).contains(&value) => minute = value,
        13 if (0..=59).contains(&value) => second = value,
        14 if (0..=999).contains(&value) => millisecond = value,
        _ => {
            return Err(cldc_error(
                "illegal-argument",
                "invalid Calendar field value",
            ));
        }
    }
    if day > days_in_month(year, month) {
        return Err(cldc_error("illegal-argument", "invalid day of month"));
    }
    compose_millis(year, month, day, hour, minute, second, millisecond)
}

pub(super) fn calendar_replace_date_time(
    millis: i64,
    year: i32,
    month: i32,
    day: i32,
    hour: i32,
    minute: i32,
    second: i32,
) -> Result<i64, EmuError> {
    if year < 1 {
        return Err(cldc_error("illegal-argument", "invalid Calendar year"));
    }
    let current_year = civil_from_days(millis.div_euclid(86_400_000)).0;
    let target_year = if current_year >= 1 { year } else { 1 - year };
    if !(0..=11).contains(&month)
        || day < 1
        || day > days_in_month(target_year, month + 1)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
    {
        return Err(cldc_error(
            "illegal-argument",
            "invalid Calendar date/time value",
        ));
    }
    compose_millis(
        target_year,
        month + 1,
        day,
        hour,
        minute,
        second,
        (millis.rem_euclid(1_000)) as i32,
    )
}

pub(super) fn replace_calendar_day(millis: i64, day_delta: i64) -> Result<i64, EmuError> {
    day_delta
        .checked_mul(86_400_000)
        .and_then(|delta| millis.checked_add(delta))
        .ok_or_else(|| cldc_error("calendar-overflow", "Calendar day replacement overflow"))
}

pub(super) fn calendar_add_field(millis: i64, field: i32, amount: i32) -> Result<i64, EmuError> {
    const DAY_MILLIS: i64 = 86_400_000;
    if amount == 0 {
        return Ok(millis);
    }
    if field == 1 || field == 2 {
        let days = millis.div_euclid(DAY_MILLIS);
        let within = millis.rem_euclid(DAY_MILLIS);
        let (mut year, mut month, mut day) = civil_from_days(days);
        if field == 1 {
            year = year
                .checked_add(amount)
                .ok_or_else(|| cldc_error("calendar-overflow", "year overflow"))?;
        } else {
            let zero_month = i64::from(year) * 12 + i64::from(month - 1) + i64::from(amount);
            year = i32::try_from(zero_month.div_euclid(12))
                .map_err(|_| cldc_error("calendar-overflow", "year overflow"))?;
            month = zero_month.rem_euclid(12) as i32 + 1;
        }
        day = day.min(days_in_month(year, month));
        return compose_millis(
            year,
            month,
            day,
            (within / 3_600_000) as i32,
            (within / 60_000 % 60) as i32,
            (within / 1_000 % 60) as i32,
            (within % 1_000) as i32,
        );
    }
    let unit = match field {
        3 | 4 | 8 => DAY_MILLIS * 7,
        5..=7 => DAY_MILLIS,
        9 => 43_200_000,
        10 | 11 => 3_600_000,
        12 => 60_000,
        13 => 1_000,
        14 => 1,
        _ => return Err(cldc_error("illegal-argument", "unsupported Calendar field")),
    };
    millis
        .checked_add(
            i64::from(amount)
                .checked_mul(unit)
                .ok_or_else(|| cldc_error("calendar-overflow", "Calendar addition overflow"))?,
        )
        .ok_or_else(|| cldc_error("calendar-overflow", "Calendar addition overflow"))
}

pub(super) fn calendar_roll_field(millis: i64, field: i32, amount: i32) -> Result<i64, EmuError> {
    const DAY_MILLIS: i64 = 86_400_000;
    if field == 1 {
        return calendar_add_field(millis, field, amount);
    }
    let days = millis.div_euclid(DAY_MILLIS);
    let within = millis.rem_euclid(DAY_MILLIS);
    let (mut year, mut month, mut day) = civil_from_days(days);
    let mut hour = (within / 3_600_000) as i32;
    let mut minute = (within / 60_000 % 60) as i32;
    let mut second = (within / 1_000 % 60) as i32;
    let mut millisecond = (within % 1_000) as i32;
    let wrap = |value: i32, minimum: i32, maximum: i32| {
        let width = i64::from(maximum - minimum + 1);
        (i64::from(minimum)
            + (i64::from(value) - i64::from(minimum) + i64::from(amount)).rem_euclid(width))
            as i32
    };
    match field {
        2 => {
            month = wrap(month, 1, 12);
            day = day.min(days_in_month(year, month));
        }
        3 | 6 => {
            let year_start = days_from_civil(year, 1, 1);
            let day_of_year = (days - year_start + 1) as i32;
            let maximum_day = days_in_year(year);
            let target_day_of_year = if field == 3 {
                let week = (day_of_year - 1) / 7 + 1;
                let day_in_week = (day_of_year - 1) % 7;
                let maximum_week = (maximum_day + 6) / 7;
                ((wrap(week, 1, maximum_week) - 1) * 7 + day_in_week + 1).min(maximum_day)
            } else {
                wrap(day_of_year, 1, maximum_day)
            };
            (year, month, day) = civil_from_days(year_start + i64::from(target_day_of_year - 1));
        }
        4 => {
            let week = (day - 1) / 7 + 1;
            let day_in_week = (day - 1) % 7;
            let maximum_day = days_in_month(year, month);
            let maximum_week = (maximum_day + 6) / 7;
            day = ((wrap(week, 1, maximum_week) - 1) * 7 + day_in_week + 1).min(maximum_day);
        }
        5 => day = wrap(day, 1, days_in_month(year, month)),
        7 => {
            let day_of_week = calendar_field(millis, 7)?;
            let week_start = days - i64::from(day_of_week - 1);
            (year, month, day) =
                civil_from_days(week_start + i64::from(wrap(day_of_week, 1, 7) - 1));
        }
        8 => {
            let ordinal = (day - 1) / 7 + 1;
            let first_occurrence = day - (ordinal - 1) * 7;
            let maximum_ordinal = (days_in_month(year, month) - first_occurrence) / 7 + 1;
            day = first_occurrence + (wrap(ordinal, 1, maximum_ordinal) - 1) * 7;
        }
        9 => hour = hour % 12 + wrap(hour / 12, 0, 1) * 12,
        10 => hour = hour / 12 * 12 + wrap(hour % 12, 0, 11),
        11 => hour = wrap(hour, 0, 23),
        12 => minute = wrap(minute, 0, 59),
        13 => second = wrap(second, 0, 59),
        14 => millisecond = wrap(millisecond, 0, 999),
        _ => {
            return Err(cldc_error(
                "illegal-argument",
                "unsupported Calendar roll field",
            ));
        }
    }
    compose_millis(year, month, day, hour, minute, second, millisecond)
}

pub(super) fn compose_millis(
    year: i32,
    month: i32,
    day: i32,
    hour: i32,
    minute: i32,
    second: i32,
    millisecond: i32,
) -> Result<i64, EmuError> {
    let value = i128::from(days_from_civil(year, month, day)) * 86_400_000
        + i128::from(hour) * 3_600_000
        + i128::from(minute) * 60_000
        + i128::from(second) * 1_000
        + i128::from(millisecond);
    i64::try_from(value).map_err(|_| cldc_error("calendar-overflow", "Calendar value overflow"))
}

pub(super) fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

pub(super) fn days_in_year(year: i32) -> i32 {
    if days_in_month(year, 2) == 29 {
        366
    } else {
        365
    }
}

pub(super) fn civil_from_days(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as i32, day as i32)
}

pub(super) fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let adjusted_year = i64::from(year) - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = i64::from(month + if month > 2 { -3 } else { 9 });
    let day_of_year = (153 * adjusted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
#[path = "../../../tests/unit/cldc/calendar.rs"]
mod tests;
