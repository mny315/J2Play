use super::*;

#[test]
fn gaming_window_is_mapped_before_rendering_without_changing_desktop_startup() {
    let gaming = options(true);
    assert_eq!(gaming.viewport.visible, Some(true));
    assert_eq!(gaming.viewport.app_id.as_deref(), Some(APP_ID));
    assert_eq!(options(false).viewport.visible, None);
}
