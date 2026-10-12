use super::*;

#[test]
fn bounded_reader_distinguishes_exact_limit_from_oversized_input() {
    assert_eq!(read_bounded(&b"abc"[..], 3).unwrap(), b"abc");
    assert_eq!(
        read_bounded(&b"abcd"[..], 3).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}
