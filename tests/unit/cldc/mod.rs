use super::*;

mod numeric_format;
mod numeric_natives;

#[test]
fn float_parsers_reject_host_specific_special_spellings() {
    for text in [
        "nan",
        "NAN",
        "nAn",
        "infinity",
        "INFINITY",
        "inf",
        "NaNf",
        "NaND",
        "InfinityF",
        "-Infinityd",
        "+nan",
        "1_000",
        "1,5",
        "1e",
        ".",
        "+",
        "",
        "1 f",
        "\u{a0}1.0",
    ] {
        assert!(parse_java_f32(text).is_none(), "Float accepted {text:?}");
        assert!(parse_java_f64(text).is_none(), "Double accepted {text:?}");
    }
}

#[test]
fn float_parsers_keep_decimal_suffixes_special_values_and_ieee_limits() {
    for (text, expected) in [
        ("1", 1.0_f32),
        ("+1.", 1.0),
        (".5", 0.5),
        ("-.5f", -0.5),
        ("1.25D", 1.25),
        ("1e2f", 100.0),
        ("0.01E+2d", 1.0),
        (" \t\r\n+2.0F\0", 2.0),
    ] {
        assert_eq!(parse_java_f32(text), Some(expected), "{text:?}");
        assert_eq!(parse_java_f64(text), Some(f64::from(expected)), "{text:?}");
    }
    for text in ["NaN", "+NaN", "-NaN"] {
        assert!(parse_java_f32(text).unwrap().is_nan());
        assert!(parse_java_f64(text).unwrap().is_nan());
    }
    for text in ["Infinity", "+Infinity", "1e9999"] {
        assert_eq!(parse_java_f32(text), Some(f32::INFINITY));
        assert_eq!(parse_java_f64(text), Some(f64::INFINITY));
    }
    for text in ["-Infinity", "-1e9999"] {
        assert_eq!(parse_java_f32(text), Some(f32::NEG_INFINITY));
        assert_eq!(parse_java_f64(text), Some(f64::NEG_INFINITY));
    }
    for text in ["-0.0", "-1e-9999"] {
        assert_eq!(
            parse_java_f32(text).unwrap().to_bits(),
            (-0.0_f32).to_bits()
        );
        assert_eq!(
            parse_java_f64(text).unwrap().to_bits(),
            (-0.0_f64).to_bits()
        );
    }
}

#[test]
fn se_featurephone_profile_publishes_family_platform_and_encoding() {
    let profile = DeviceProfile::from_reader(
        include_bytes!("../../../profiles/sony-ericsson/featurephone.json").as_slice(),
    )
    .unwrap();
    let properties = SystemProperties::from_profile(&profile);
    assert_eq!(
        properties.get("microedition.platform"),
        Some("SonyEricsson")
    );
    assert_eq!(properties.get("microedition.encoding"), Some("ISO-8859-1"));
}

#[test]
fn siemens_featurephone_uses_the_cldc_default_encoding() {
    let profile = DeviceProfile::from_reader(
        include_bytes!("../../../profiles/siemens/featurephone.json").as_slice(),
    )
    .unwrap();
    let properties = SystemProperties::from_profile(&profile);
    assert_eq!(properties.get("microedition.platform"), Some("Siemens"));
    assert_eq!(properties.get("microedition.encoding"), Some("ISO-8859-1"));
}

#[test]
fn nokia_featurephone_profile_publishes_its_own_properties() {
    let profile = DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/featurephone.json").as_slice(),
    )
    .unwrap();
    let properties = SystemProperties::from_profile(&profile);
    assert_eq!(properties.get("microedition.platform"), Some("Nokia"));
    assert_eq!(properties.get("microedition.encoding"), Some("ISO-8859-1"));
    assert_eq!(properties.get("supports.mixing"), Some("false"));
    assert_eq!(properties.get("microedition.m3g.version"), Some("1.1"));
}

#[test]
fn nokia_s60_generation_profiles_publish_distinct_platform_versions() {
    let v1 = DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/s60-v1-keypad.json").as_slice(),
    )
    .unwrap();
    let v1_properties = SystemProperties::from_profile(&v1);
    assert_eq!(
        v1_properties.get("microedition.configuration"),
        Some("CLDC-1.0")
    );
    assert_eq!(v1_properties.get("microedition.profiles"), Some("MIDP-1.0"));
    assert_eq!(v1_properties.get("microedition.m3g.version"), None);

    let v2 = DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/s60-v2-keypad.json").as_slice(),
    )
    .unwrap();
    let v2_properties = SystemProperties::from_profile(&v2);
    assert_eq!(
        v2_properties.get("microedition.configuration"),
        Some("CLDC-1.1")
    );
    assert_eq!(v2_properties.get("microedition.profiles"), Some("MIDP-2.0"));
    assert_eq!(v2_properties.get("microedition.m3g.version"), Some("1.0"));
}

#[test]
fn nokia_s60_touch_profile_publishes_measured_midp21_properties() {
    let profile = DeviceProfile::from_reader(
        include_bytes!("../../../profiles/nokia/s60-touch.json").as_slice(),
    )
    .unwrap();
    let properties = SystemProperties::from_profile(&profile);
    assert_eq!(properties.get("microedition.platform"), Some("Nokia"));
    assert_eq!(
        properties.get("microedition.configuration"),
        Some("CLDC-1.1")
    );
    assert_eq!(properties.get("microedition.profiles"), Some("MIDP-2.1"));
    assert_eq!(properties.get("microedition.encoding"), Some("ISO-8859-1"));
    assert_eq!(properties.get("microedition.m3g.version"), Some("1.1"));
}
