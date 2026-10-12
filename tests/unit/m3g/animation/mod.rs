use super::*;

#[test]
fn animation_weights_scale_contributions_without_being_normalized() {
    assert_eq!(blend_samples(&[(&[0.8], 0.25)]).unwrap(), [0.2]);
    assert_eq!(
        blend_samples(&[(&[0.2], 1.0), (&[0.4], 1.0)]).unwrap(),
        [0.6]
    );
    let weight = f32::EPSILON * 0.25;
    assert_eq!(blend_samples(&[(&[0.5], weight)]).unwrap(), [weight * 0.5]);
    assert_eq!(
        blend_samples(&[(&[0.25], f32::MAX), (&[0.25], f32::MAX)]).unwrap(),
        [f32::MAX * 0.5]
    );
}

#[test]
fn blending_validates_every_input_before_producing_output() {
    assert_eq!(
        blend_samples(&[(&[2.0, 4.0], 1.0), (&[6.0, 8.0], 3.0)]).unwrap(),
        [20.0, 28.0]
    );
    assert_eq!(
        blend_samples(&[(&[1.0][..], 1.0), (&[2.0, 3.0], 1.0)])
            .unwrap_err()
            .code(),
        "invalid-blend"
    );
}

#[test]
fn blending_cancels_large_contributions_before_rounding_to_float() {
    for weight in [1.0, 2.0, f32::MAX] {
        let samples = [
            ([f32::MAX, -f32::MAX], weight),
            ([f32::MAX, -f32::MAX], weight),
            ([-f32::MAX, f32::MAX], weight),
            ([-f32::MAX, f32::MAX], weight),
            ([7.0, -5.0], 0.5),
        ];
        assert_eq!(blend_samples(&samples).unwrap(), [3.5, -2.5]);
    }
}
