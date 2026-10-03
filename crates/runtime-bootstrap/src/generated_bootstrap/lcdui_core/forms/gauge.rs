//! Numeric state and drawing for the bootstrap `Gauge` class.

use super::CodeAttribute;
use crate::bytecode_builder::Code;

const INTERACTIVE: u16 = 14;
const MAXIMUM: u16 = 18;
const VALUE: u16 = 26;

pub(super) fn set_maximum() -> CodeAttribute {
    let mut code = Code::default();
    code.emit(&[0x1b]).jump(0x9d, "valid");
    code.emit(&[0x1b, 0x02]).jump(0xa0, "invalid");
    code.emit(&[0x2a])
        .reference(0xb4, INTERACTIVE)
        .jump(0x99, "valid");
    code.label("invalid")
        .reference(0xbb, 9)
        .emit(&[0x59])
        .reference(0xb7, 11)
        .emit(&[0xbf]);

    // A transition between definite and indefinite range resets the value.
    // Staying indefinite preserves its activity state; definite values clamp.
    code.label("valid")
        .emit(&[0x2a])
        .reference(0xb4, MAXIMUM)
        .emit(&[0x1b])
        .jump(0x9f, "invalidate");
    code.emit(&[0x1b]).jump(0x9b, "reset");
    code.emit(&[0x2a])
        .reference(0xb4, MAXIMUM)
        .jump(0x9b, "reset");
    code.emit(&[0x2a])
        .reference(0xb4, VALUE)
        .emit(&[0x1b])
        .jump(0xa4, "maximum");
    code.emit(&[0x2a, 0x1b])
        .reference(0xb5, VALUE)
        .jump(0xa7, "maximum");
    code.label("reset")
        .emit(&[0x2a, 0x03])
        .reference(0xb5, VALUE);
    code.label("maximum")
        .emit(&[0x2a, 0x1b])
        .reference(0xb5, MAXIMUM);
    code.label("invalidate")
        .emit(&[0x2a])
        .reference(0xb6, 29)
        .emit(&[0xb1]);
    finish(code, 2, 2)
}

pub(super) fn key() -> CodeAttribute {
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb4, INTERACTIVE)
        .jump(0x99, "done");
    code.emit(&[0x2a]).reference(0xb4, VALUE).emit(&[0x3d]); // old value
    code.emit(&[0x1b, 0x10, 0xfd]).jump(0x9f, "left");
    code.emit(&[0x1b, 0x10, 52]).jump(0xa0, "right_key");
    code.label("left")
        .emit(&[0x2a, 0x1c, 0x04, 0x64])
        .jump(0xa7, "apply");
    code.label("right_key")
        .emit(&[0x1b, 0x10, 0xfc])
        .jump(0x9f, "right");
    code.emit(&[0x1b, 0x10, 54]).jump(0xa0, "done");
    // Values are nonnegative. A negative increment is integer overflow;
    // retain the old value and still call the virtual setValue as before.
    code.label("right")
        .emit(&[0x2a, 0x1c, 0x04, 0x60, 0x59])
        .jump(0x9c, "apply");
    code.emit(&[0x57, 0x1c]);
    code.label("apply").reference(0xb6, 22);
    code.emit(&[0x2a])
        .reference(0xb4, VALUE)
        .emit(&[0x1c])
        .jump(0x9f, "done");
    code.emit(&[0x2a]).reference(0xb6, 46);
    code.label("done").emit(&[0xb1]);
    finish(code, 3, 3)
}

pub(super) fn paint() -> CodeAttribute {
    let mut code = Code::default();
    // Arguments: receiver, graphics, x, y, width, focused. Local 6: fill width.
    code.emit(&[0x2b, 0x03]).reference(0xb6, 49); // black label/border
    code.emit(&[0x2a]).reference(0xb6, 32).jump(0xc6, "border");
    code.emit(&[0x2b, 0x2a])
        .reference(0xb6, 32)
        .emit(&[0x1c, 0x1d, 0x10, 20])
        .reference(0xb6, 54);
    code.emit(&[0x1d])
        .reference(0xb8, 36)
        .reference(0xb6, 42)
        .emit(&[0x04, 0x60, 0x60, 0x3e]);
    code.label("border")
        .emit(&[0x2b, 0x1c, 0x1d, 0x15, 4, 0x04, 0x64, 0x10, 9])
        .reference(0xb6, 58);
    code.emit(&[0x2a])
        .reference(0xb4, MAXIMUM)
        .emit(&[0x02])
        .jump(0xa0, "definite");
    code.emit(&[0x2a])
        .reference(0xb4, VALUE)
        .emit(&[0x05])
        .jump(0x9f, "active");
    code.emit(&[0x2a])
        .reference(0xb4, VALUE)
        .emit(&[0x06])
        .jump(0xa0, "idle");
    code.label("active")
        .emit(&[0x15, 4, 0x05, 0x6c])
        .jump(0xa7, "fill_width");
    code.label("idle").emit(&[0x03]).jump(0xa7, "fill_width");
    // (width - 2) * value / maximum, widening before the product.
    code.label("definite")
        .emit(&[0x15, 4, 0x05, 0x64, 0x85, 0x2a])
        .reference(0xb4, VALUE);
    code.emit(&[0x85, 0x69, 0x2a])
        .reference(0xb4, MAXIMUM)
        .emit(&[0x85, 0x6d, 0x88]);
    code.label("fill_width")
        .emit(&[0x36, 6, 0x2b, 0x15, 5])
        .jump(0x99, "unfocused");
    code.emit(&[0x12, 62]).jump(0xa7, "color");
    code.label("unfocused").emit(&[0x12, 63]);
    code.label("color").reference(0xb6, 49);
    code.emit(&[0x2b, 0x1c, 0x04, 0x60, 0x1d, 0x04, 0x60, 0x15, 6, 0x10, 8])
        .reference(0xb6, 64)
        .emit(&[0xb1]);
    finish(code, 5, 7)
}

fn finish(code: Code, max_stack: u16, max_locals: u16) -> CodeAttribute {
    CodeAttribute {
        name_index: 79,
        max_stack,
        max_locals,
        code: code.finish(),
        exception_table: Vec::new(),
        attributes: Vec::new(),
    }
}
