use crate::bytecode_builder::Code;

/// Match a filename with constant stack space. A mismatch extends only the
/// latest star; earlier stars never need to retry an already matched prefix.
/// This avoids the recursive matcher enumerating every partition of the name.
pub(super) fn wildcard_code(length: u16, char_at: u16) -> Vec<u8> {
    // Locals: this, name, name index, pattern, pattern index,
    // latest star, star's name endpoint, name length, pattern length, character.
    let mut code = Code::default();
    code.emit(&[0x2b])
        .reference(0xb6, length)
        .emit(&[0x36, 7, 0x2d])
        .reference(0xb6, length)
        .emit(&[0x36, 8, 0x02, 0x36, 5]);

    code.label("scan").emit(&[0x1c, 0x15, 7]).jump(0xa2, "tail");
    code.emit(&[0x15, 4, 0x15, 8]).jump(0xa2, "retry");
    code.emit(&[0x2d, 0x15, 4])
        .reference(0xb6, char_at)
        .emit(&[0x36, 9, 0x15, 9, 0x10, b'*'])
        .jump(0x9f, "star");
    code.emit(&[0x15, 9, 0x10, b'?']).jump(0x9f, "advance");
    code.emit(&[0x15, 9, 0x2b, 0x1c])
        .reference(0xb6, char_at)
        .jump(0x9f, "advance");

    code.label("retry").emit(&[0x15, 5]).jump(0x9b, "fail");
    code.emit(&[0x84, 6, 1, 0x15, 6, 0x3d, 0x15, 5, 0x04, 0x60, 0x36, 4])
        .jump(0xa7, "scan");

    code.label("star")
        .emit(&[0x15, 4, 0x36, 5, 0x1c, 0x36, 6, 0x84, 4, 1])
        .jump(0xa7, "scan");
    code.label("advance")
        .emit(&[0x84, 2, 1, 0x84, 4, 1])
        .jump(0xa7, "scan");

    code.label("tail")
        .emit(&[0x15, 4, 0x15, 8])
        .jump(0xa2, "match");
    code.emit(&[0x2d, 0x15, 4])
        .reference(0xb6, char_at)
        .emit(&[0x10, b'*'])
        .jump(0xa0, "fail");
    code.emit(&[0x84, 4, 1]).jump(0xa7, "tail");
    code.label("fail").emit(&[0x03, 0xac]);
    code.label("match").emit(&[0x04, 0xac]);
    code.finish()
}
