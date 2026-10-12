use super::*;

#[test]
fn invalid_options_cannot_silently_run_a_different_parser_or_iteration_count() {
    assert_eq!(options(&[]).unwrap(), ("all", 1));
    for mode in ["amr", "all", "classfile", "m3g-object"] {
        let args = [mode.to_owned(), "100".to_owned()];
        assert_eq!(options(&args).unwrap(), (mode, 100));
    }
    for arguments in [
        vec!["amrr"],
        vec!["amr", "0"],
        vec!["amr", "ten"],
        vec!["amr", "1", "extra"],
    ] {
        let arguments = arguments.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            options(&arguments).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
    }
}

#[test]
fn amr_seed_reaches_native_decoding_for_every_supported_frame_type() {
    let mut runtime =
        mmapi::Runtime::new(mmapi::NullAudioSink::default(), mmapi::Limits::default());
    let handle = runtime
        .create_from_bytes("audio/amr", &valid_amr_file())
        .unwrap();
    runtime.realize(handle).unwrap();
    assert_eq!(runtime.duration(handle).unwrap(), 400_000);
}

#[test]
fn micro3d_default_seeds_reach_valid_decoded_resources() {
    let limits = micro3d::LoaderLimits::default();
    let figure =
        micro3d::FigureData::parse(&valid_micro3d_resource("micro3d-figure"), limits).unwrap();
    assert_eq!(figure.vertices.len(), 1);
    let actions =
        micro3d::ActionTableData::parse(&valid_micro3d_resource("micro3d-action"), limits).unwrap();
    assert_eq!(actions.frame_counts, [6, 17]);
    let texture =
        micro3d::TextureData::parse(&valid_micro3d_resource("micro3d-texture"), true, limits)
            .unwrap();
    assert_eq!(texture.pixels.as_ref(), [0xff00_00ff]);
}
