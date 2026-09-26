use super::*;

#[test]
fn parses_standard_http_date() {
    for date in [
        "Sun, 06 Nov 1994 08:49:37 GMT",
        "Sunday, 06-Nov-94 08:49:37 GMT",
        "Sun Nov  6 08:49:37 1994",
        "\tSun,  06\tNov\r\n1994 08:49:37 GMT ",
    ] {
        assert_eq!(parse_http_date(date, 0).unwrap(), 784_111_777_000);
    }
    for date in [
        "bad",
        "Sun, 06 Nov 1994 08:49:37",
        "Sun, 06 Nov 1994 08:49:37 GMT extra",
        "Sun, 06 Nov 1994 08:49 GMT",
        "Sun, 06 Nov 1994 08:49:37:00 GMT",
        "Sun, 06 Nov 1994 08:49:x GMT",
    ] {
        assert_eq!(
            parse_http_date(date, 0).unwrap_err().code(),
            "network-header"
        );
    }
    assert_eq!(
        parse_http_date("Mon, 31 Feb 2025 00:00:00 GMT", 0)
            .unwrap_err()
            .code(),
        "network-header"
    );
    assert_eq!(
        parse_http_date("Sun, 06 Nov 2147483647 08:49:37 GMT", 0)
            .unwrap_err()
            .code(),
        "network-header"
    );
}

#[test]
fn dates_before_the_epoch_and_leap_years_keep_exact_milliseconds() {
    for (date, expected) in [
        ("Wed, 31 Dec 1969 23:59:59 GMT", -1_000),
        ("Wed Dec 31 23:59:59 1969", -1_000),
        ("Monday, 01-Jan-01 00:00:00 GMT", 978_307_200_000),
        ("Mon, 01 Jan 0001 00:00:00 GMT", -62_135_596_800_000),
        ("Tue, 29 Feb 2000 00:00:00 GMT", 951_782_400_000),
        ("Wed, 31 Dec 1969 23:59:60 GMT", -1_000),
    ] {
        assert_eq!(parse_http_date(date, 0).unwrap(), expected, "{date}");
    }
    for date in [
        "Mon, 29 Feb 1900 00:00:00 GMT",
        "Mon, 29 Feb 2100 00:00:00 GMT",
        "Mon, 01 Jan 0000 00:00:00 GMT",
        "Mon, 01 Jan 2000 24:00:00 GMT",
        "Mon, 01 Jan 2000 23:60:00 GMT",
        "Mon, 01 Jan 2000 23:59:61 GMT",
    ] {
        assert_eq!(
            parse_http_date(date, 0).unwrap_err().code(),
            "network-header"
        );
    }
}

#[test]
fn two_digit_years_follow_the_clock_across_the_fifty_year_and_century_boundaries() {
    let now = 1_749_990_645_000; // 2025-06-15 12:30:45 UTC
    assert_eq!(
        parse_http_date("Saturday, 15-Jun-75 12:30:45 GMT", now).unwrap(),
        3_327_827_445_000,
    );
    assert_eq!(
        parse_http_date("Sunday, 15-Jun-75 12:30:46 GMT", now).unwrap(),
        172_067_446_000,
    );
    assert_eq!(
        parse_http_date("Friday, 01-Jan-00 00:00:00 GMT", 4_070_908_800_000).unwrap(),
        4_102_444_800_000,
    );
    for now in [i64::MIN, i64::MAX] {
        assert!(parse_http_date("Sunday, 06-Nov-94 08:49:37 GMT", now).is_err());
        assert_eq!(
            parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT", now).unwrap(),
            784_111_777_000,
        );
    }
}

#[test]
fn all_formats_reject_invalid_fields_and_trailing_tokens() {
    for date in [
        "Unknown, 06 Nov 1994 08:49:37 GMT",
        "Sunday, 06-Nov-1994 08:49:37 GMT",
        "Sunday, 06-Nov-94-extra 08:49:37 GMT",
        "Sunday, 06-Nov-94 08:49:37 UTC",
        "Sunday, 06-Nov-94 08:49:37 GMT extra",
        "Sun Nov 6 08:49:37 1994 extra",
        "Sun Nov 6 08:49:37 94",
        "Sun Nov +6 08:49:37 1994",
        "Sun Nov 6 +8:49:37 1994",
        "Sun Xxx 6 08:49:37 1994",
        "Sun Nov 0 08:49:37 1994",
    ] {
        assert_eq!(
            parse_http_date(date, 0).unwrap_err().code(),
            "network-header",
            "{date}",
        );
    }
}
