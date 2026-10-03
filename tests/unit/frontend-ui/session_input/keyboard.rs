use super::{HostAction, Key, text_edit_action_for_key};

#[test]
fn system_back_is_navigation_only_and_guest_back_keeps_an_explicit_binding() {
    assert_eq!(text_edit_action_for_key(Key::BrowserBack), None);
    assert_eq!(text_edit_action_for_key(Key::F3), Some(HostAction::Back));
}
