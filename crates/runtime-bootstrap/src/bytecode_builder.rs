//! Label-based assembly for Rust-owned bootstrap bytecode.

#[derive(Default)]
pub(crate) struct Code {
    bytes: Vec<u8>,
    labels: Vec<(&'static str, usize)>,
    jumps: Vec<(usize, &'static str)>,
}

impl Code {
    pub(crate) fn emit(&mut self, bytes: &[u8]) -> &mut Self {
        self.bytes.extend_from_slice(bytes);
        self
    }

    pub(crate) fn reference(&mut self, opcode: u8, index: u16) -> &mut Self {
        self.emit(&[opcode]).emit(&index.to_be_bytes())
    }

    pub(crate) fn label(&mut self, name: &'static str) -> &mut Self {
        assert!(
            self.labels.iter().all(|(label, _)| *label != name),
            "duplicate bootstrap label: {name}"
        );
        self.labels.push((name, self.bytes.len()));
        self
    }

    pub(crate) fn jump(&mut self, opcode: u8, name: &'static str) -> &mut Self {
        self.jumps.push((self.bytes.len(), name));
        self.emit(&[opcode, 0, 0])
    }

    pub(crate) fn finish(mut self) -> Vec<u8> {
        for (position, name) in self.jumps {
            let target = self
                .labels
                .iter()
                .find(|(label, _)| *label == name)
                .unwrap_or_else(|| panic!("undefined bootstrap label: {name}"))
                .1;
            let offset = i16::try_from(
                isize::try_from(target).unwrap() - isize::try_from(position).unwrap(),
            )
            .unwrap();
            self.bytes[position + 1..position + 3].copy_from_slice(&offset.to_be_bytes());
        }
        self.bytes
    }
}
