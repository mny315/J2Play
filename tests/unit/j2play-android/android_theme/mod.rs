use super::*;

#[test]
fn android_12_through_14_keep_dynamic_colors_without_error_resources() {
    for api in 31..35 {
        for dark in [false, true] {
            let colors = palette(api, dark, |name| {
                if name.starts_with("error_") {
                    Err("resource unavailable")
                } else if name.starts_with("accent1_") {
                    Ok([1, 2, 3])
                } else {
                    Ok([4, 5, 6])
                }
            })
            .expect("missing error tones must not discard dynamic colors")
            .unwrap();
            assert_eq!(colors.primary, [1, 2, 3]);
            assert_eq!(colors.surface, [4, 5, 6]);
            assert_ne!(colors.error, colors.error_container);
            assert_ne!(colors.error_container, colors.on_error_container);
        }
    }
}

#[test]
fn android_15_uses_system_error_colors_and_reports_lookup_failure() {
    for dark in [false, true] {
        let colors = palette(35, dark, |name| {
            Ok::<_, ()>(if name.starts_with("error_") {
                [7, 8, 9]
            } else {
                [1, 2, 3]
            })
        })
        .unwrap()
        .unwrap();
        assert_eq!(colors.primary, [1, 2, 3]);
        assert_eq!(colors.error, [7, 8, 9]);
        assert_eq!(colors.error_container, [7, 8, 9]);
        assert_eq!(colors.on_error_container, [7, 8, 9]);
        assert_eq!(
            palette(35, dark, |_| Err("lookup failed")),
            Err("lookup failed")
        );
    }
}

#[test]
fn older_android_never_queries_dynamic_resources() {
    for api in 23..31 {
        assert!(
            palette::<()>(api, false, |_| panic!("unsupported resource lookup"))
                .unwrap()
                .is_none()
        );
    }
}
