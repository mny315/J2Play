//! Rebuild the enumeration's ID sequence, with optional filtering and ordering.

use super::CodeAttribute;
use crate::bytecode_builder::Code;

const STORE: u16 = 7;
const FILTER: u16 = 13;
const COMPARATOR: u16 = 17;
const IDS: u16 = 40;
const INITIAL: u16 = 55;
const CURSOR: u16 = 58;
const VERSION: u16 = 74;

pub(super) fn rebuild() -> CodeAttribute {
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb4, STORE)
        .reference(0xb6, 82); // ensureOpen
    code.emit(&[0x2a])
        .reference(0xb4, STORE)
        .reference(0xb6, 85) // recordIds
        .emit(&[0x4c]); // local 1: source IDs

    // With no filter, every returned ID belongs in the enumeration. Keep this
    // array directly; only the optional sorting below needs record payloads.
    code.emit(&[0x2a])
        .reference(0xb4, FILTER)
        .jump(0xc7, "filter");
    code.emit(&[0x2a, 0x2b])
        .reference(0xb5, IDS)
        .jump(0xa7, "sort");

    // Compact matching IDs in the temporary array, then publish its prefix.
    code.label("filter").emit(&[0x03, 0x3d, 0x03, 0x3e]); // count, index
    code.label("filter_loop")
        .emit(&[0x1d, 0x2b, 0xbe])
        .jump(0xa2, "filtered");
    code.emit(&[0x2a])
        .reference(0xb4, STORE)
        .emit(&[0x2b, 0x1d, 0x2e])
        .reference(0xb6, 48) // getRecord
        .emit(&[0x3a, 4]);
    code.emit(&[0x2a])
        .reference(0xb4, FILTER)
        .jump(0xc6, "accept");
    code.emit(&[0x2a])
        .reference(0xb4, FILTER)
        .emit(&[0x19, 4])
        .reference(0xb9, 89)
        .emit(&[2, 0]) // matches(byte[])
        .jump(0x99, "filter_next");
    code.label("accept")
        .emit(&[0x2b, 0x1c, 0x84, 2, 1, 0x2b, 0x1d, 0x2e, 0x4f]);
    code.label("filter_next")
        .emit(&[0x84, 3, 1])
        .jump(0xa7, "filter_loop");
    code.label("filtered")
        .emit(&[0x2a, 0x1c, 0xbc, 10])
        .reference(0xb5, IDS)
        .emit(&[0x03, 0x3e]);
    code.label("copy_loop")
        .emit(&[0x1d, 0x1c])
        .jump(0xa2, "sort");
    code.emit(&[0x2a])
        .reference(0xb4, IDS)
        .emit(&[0x1d, 0x2b, 0x1d, 0x2e, 0x4f, 0x84, 3, 1])
        .jump(0xa7, "copy_loop");

    // Preserve stable insertion order for equivalent records and the existing
    // callback/read order. A comparator may execute arbitrary guest code.
    code.label("sort")
        .emit(&[0x2a])
        .reference(0xb4, COMPARATOR)
        .jump(0xc6, "finish");
    code.emit(&[0x04, 0x3e]);
    code.label("sort_loop").emit(&[0x1d, 0x2a]);
    code.reference(0xb4, IDS).emit(&[0xbe]).jump(0xa2, "finish");
    code.emit(&[0x2a])
        .reference(0xb4, IDS)
        .emit(&[0x1d, 0x2e, 0x36, 4]); // selected ID
    code.emit(&[0x2a])
        .reference(0xb4, STORE)
        .emit(&[0x15, 4])
        .reference(0xb6, 48)
        .emit(&[0x3a, 5, 0x1d, 0x36, 6]); // selected data, insertion index
    code.label("insert_loop")
        .emit(&[0x15, 6])
        .jump(0x9e, "insert");
    code.emit(&[0x2a]).reference(0xb4, COMPARATOR);
    code.emit(&[0x2a]).reference(0xb4, STORE);
    code.emit(&[0x2a])
        .reference(0xb4, IDS)
        .emit(&[0x15, 6, 0x04, 0x64, 0x2e])
        .reference(0xb6, 48);
    code.emit(&[0x19, 5])
        .reference(0xb9, 95)
        .emit(&[3, 0, 0x04]) // compare(previous, selected) == FOLLOWS
        .jump(0xa0, "insert");
    code.emit(&[0x2a]).reference(0xb4, IDS).emit(&[0x15, 6]);
    code.emit(&[0x2a])
        .reference(0xb4, IDS)
        .emit(&[0x15, 6, 0x04, 0x64, 0x2e, 0x4f, 0x84, 6, 0xff])
        .jump(0xa7, "insert_loop");
    code.label("insert")
        .emit(&[0x2a])
        .reference(0xb4, IDS)
        .emit(&[0x15, 6, 0x15, 4, 0x4f, 0x84, 3, 1])
        .jump(0xa7, "sort_loop");

    code.label("finish").emit(&[0x2a, 0x2a]);
    code.reference(0xb4, STORE)
        .reference(0xb6, 71)
        .reference(0xb5, VERSION);
    code.emit(&[0x2a, 0x03]).reference(0xb5, CURSOR);
    code.emit(&[0x2a, 0x04])
        .reference(0xb5, INITIAL)
        .emit(&[0xb1]);
    CodeAttribute {
        name_index: 104,
        max_stack: 5,
        max_locals: 7,
        code: code.finish(),
        exception_table: Vec::new(),
        attributes: Vec::new(),
    }
}
