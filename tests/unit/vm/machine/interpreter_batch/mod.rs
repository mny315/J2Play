use super::*;

#[test]
fn batch_string_borrows_check_full_handles_and_expire_at_batch_end() {
    let first = Handle::from_raw(1 << 32);
    let collision = Handle::from_raw(17 << 32);
    let reused = Handle::from_raw((1 << 32) | 1);
    let mut strings =
        crate::machine::StringValues::from([(first, vec![65]), (collision, vec![66])]);
    {
        let mut cache = None;
        assert_eq!(cached_string(&strings, &mut cache, first), Some(&[65][..]));
        assert_eq!(cached_string(&strings, &mut cache, first), Some(&[65][..]));
        assert_eq!(cached_string(&strings, &mut cache, reused), None);
        assert_eq!(
            cached_string(&strings, &mut cache, collision),
            Some(&[66][..])
        );
        assert_eq!(cached_string(&strings, &mut cache, first), Some(&[65][..]));
    }
    strings.insert(first, vec![67]);
    strings.retain(|handle, _| *handle != collision);
    strings.insert(reused, vec![68]);
    let mut cache = None;
    assert_eq!(cached_string(&strings, &mut cache, first), Some(&[67][..]));
    assert_eq!(cached_string(&strings, &mut cache, collision), None);
    assert_eq!(cached_string(&strings, &mut cache, reused), Some(&[68][..]));
}
