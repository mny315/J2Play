//! Shared `Choice` flag semantics for `List` and `ChoiceGroup`.

use super::CodeAttribute;
use crate::bytecode_builder::Code;

pub(super) struct ChoiceFlags {
    pub code_name: u16,
    pub entries: u16,
    pub size: u16,
    pub entry: u16,
    pub kind: u16,
    pub cursor: u16,
    pub selected: u16,
    pub is_selected: u16,
    pub invalidate: u16,
    pub null_exception: (u16, u16),
    pub argument_exception: (u16, u16),
}

impl ChoiceFlags {
    pub(super) fn get(&self) -> CodeAttribute {
        let mut code = self.checked_array();
        // Locals: receiver, flags, index, selected count. Keep the virtual
        // isSelected call and size check so subclass dispatch remains intact.
        code.emit(&[0x03, 0x3d, 0x03, 0x3e]);
        code.label("loop")
            .emit(&[0x1c, 0x2b, 0xbe])
            .jump(0xa2, "done");
        code.emit(&[0x2b, 0x1c, 0x1c]);
        self.size(&mut code);
        code.jump(0xa2, "unselected");
        code.emit(&[0x2a, 0x1c])
            .reference(0xb6, self.is_selected)
            .jump(0x99, "unselected");
        code.emit(&[0x04]).jump(0xa7, "store");
        code.label("unselected").emit(&[0x03]);
        code.label("store")
            .emit(&[0x54, 0x2b, 0x1c, 0x33])
            .jump(0x99, "next");
        code.emit(&[0x84, 3, 1]);
        code.label("next").emit(&[0x84, 2, 1]).jump(0xa7, "loop");
        code.label("done").emit(&[0x1d, 0xac]);
        self.finish(code, 4, 4)
    }

    pub(super) fn set(&self) -> CodeAttribute {
        let mut code = self.checked_array();
        // Locals: receiver, flags, index. A valid array covers every entry;
        // excess flags never participate in selecting an entry.
        code.emit(&[0x03, 0x3d, 0x2a])
            .reference(0xb4, self.kind)
            .emit(&[0x05])
            .jump(0xa0, "single");
        code.label("multiple").emit(&[0x1c]);
        self.size(&mut code);
        code.jump(0xa2, "done");
        code.emit(&[0x2a, 0x1c]).reference(0xb7, self.entry);
        code.emit(&[0x2b, 0x1c, 0x33])
            .reference(0xb5, self.selected);
        code.emit(&[0x84, 2, 1]).jump(0xa7, "multiple");

        code.label("single").emit(&[0x1c]);
        self.size(&mut code);
        code.jump(0xa2, "default");
        code.emit(&[0x2b, 0x1c, 0x33]).jump(0x9a, "select");
        code.emit(&[0x84, 2, 1]).jump(0xa7, "single");
        code.label("default").emit(&[0x03, 0x3d]);
        code.label("select")
            .emit(&[0x2a, 0x1c])
            .reference(0xb5, self.cursor);
        code.label("done")
            .emit(&[0x2a])
            .reference(0xb6, self.invalidate)
            .emit(&[0xb1]);
        self.finish(code, 3, 3)
    }

    fn checked_array(&self) -> Code {
        let mut code = Code::default();
        code.emit(&[0x2b]).jump(0xc7, "nonnull");
        Self::throw(&mut code, self.null_exception);
        code.label("nonnull").emit(&[0x2b, 0xbe]);
        self.size(&mut code);
        code.jump(0xa2, "valid");
        Self::throw(&mut code, self.argument_exception);
        code.label("valid");
        code
    }

    fn size(&self, code: &mut Code) {
        code.emit(&[0x2a])
            .reference(0xb4, self.entries)
            .reference(0xb6, self.size);
    }

    fn throw(code: &mut Code, (class, constructor): (u16, u16)) {
        code.reference(0xbb, class)
            .emit(&[0x59])
            .reference(0xb7, constructor)
            .emit(&[0xbf]);
    }

    fn finish(&self, code: Code, max_stack: u16, max_locals: u16) -> CodeAttribute {
        CodeAttribute {
            name_index: self.code_name,
            max_stack,
            max_locals,
            code: code.finish(),
            exception_table: Vec::new(),
            attributes: Vec::new(),
        }
    }
}
