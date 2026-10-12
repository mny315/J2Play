//! Rectangle bounds for the eight axis-aligned Sprite transforms.

use super::CodeAttribute;
use crate::bytecode_builder::Code;

pub(super) fn rectangle_code(name_index: u16) -> CodeAttribute {
    let mut code = Code::default();
    // Arguments: world x/y, rectangle x/y/w/h, frame w/h, transform.
    // Long locals 9/11 and 13/15 hold each axis's two pixel endpoints.
    for (origin, extent, first, last) in [(2, 4, 9, 11), (3, 5, 13, 15)] {
        code.emit(&[0x15, origin, 0x85, 0x37, first]); // first = origin
        code.emit(&[
            0x16, first, 0x15, extent, 0x85, 0x61, 0x0a, 0x65, 0x37, last,
        ]);
    }

    // Public setTransform rejects invalid values. Preserve the old identity
    // default here as well, before interpreting the three transform bits.
    code.emit(&[0x15, 8]).jump(0x9b, "identity");
    code.emit(&[0x15, 8, 0x10, 7]).jump(0xa4, "reflect");
    code.label("identity").emit(&[0x03, 0x36, 8]);
    code.label("reflect");
    for (mask, frame, first, last, done) in
        [(2, 6, 9, 11, "reflected_x"), (1, 7, 13, 15, "reflected_y")]
    {
        code.emit(&[0x15, 8, 0x03 + mask, 0x7e]).jump(0x99, done);
        for endpoint in [first, last] {
            // Java computes frame - 1 as int before widening to long.
            code.emit(&[
                0x15, frame, 0x04, 0x64, 0x85, 0x16, endpoint, 0x65, 0x37, endpoint,
            ]);
        }
        code.label(done);
    }
    for (first, last, done) in [(9, 11, "ordered_x"), (13, 15, "ordered_y")] {
        // Normalize even zero/negative extents to preserve the four-corner
        // calculation's result for every possible internal input.
        code.emit(&[0x16, first, 0x16, last, 0x94]).jump(0x9e, done);
        swap_longs(&mut code, first, last);
        code.label(done);
    }
    code.emit(&[0x15, 8, 0x07, 0x7e]).jump(0x99, "world_bounds");
    swap_longs(&mut code, 9, 13);
    swap_longs(&mut code, 11, 15);
    code.label("world_bounds").emit(&[0x07, 0xbc, 0x0b]); // new long[4]
    for (index, world, bound) in [(0, 0, 9), (1, 1, 13), (2, 0, 11), (3, 1, 15)] {
        code.emit(&[0x59, 0x03 + index, 0x15, world, 0x85, 0x16, bound, 0x61]);
        if index >= 2 {
            code.emit(&[0x0a, 0x61]); // exclusive upper edge
        }
        code.emit(&[0x50]); // lastore
    }
    code.emit(&[0xb0]);
    CodeAttribute {
        name_index,
        max_stack: 7,
        max_locals: 17,
        code: code.finish(),
        exception_table: Vec::new(),
        attributes: Vec::new(),
    }
}

fn swap_longs(code: &mut Code, first: u8, second: u8) {
    code.emit(&[0x16, first, 0x16, second, 0x37, first, 0x37, second]);
}
