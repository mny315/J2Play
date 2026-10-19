//! Project-owned OTA tones encoded from Smart Messaging section 3.6.

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum Instruction {
    Note(u8, u8, u8),
    Scale(u8),
    Style(u8),
    Tempo(u8),
    Volume(u8),
}

#[derive(Default)]
struct Bits(Vec<u8>, usize);

impl Bits {
    fn put(&mut self, value: u8, width: usize) {
        for bit in (0..width).rev() {
            if self.1.is_multiple_of(8) {
                self.0.push(0);
            }
            let last = self.0.last_mut().unwrap();
            *last |= ((value >> bit) & 1) << (7 - self.1 % 8);
            self.1 += 1;
        }
    }
    fn align(&mut self) {
        while !self.1.is_multiple_of(8) {
            self.put(0, 1);
        }
    }
}

pub fn song(basic: bool, unicode: bool, patterns: &[(u8, u8, &[Instruction])]) -> Vec<u8> {
    let mut bits = Bits::default();
    bits.put(if unicode { 3 } else { 2 }, 8);
    bits.put(0x25, 7);
    bits.align();
    if unicode {
        bits.put(0x22, 7);
        bits.align();
    }
    bits.put(0x1d, 7);
    bits.put(if basic { 1 } else { 2 }, 3);
    if basic {
        bits.put(2, 4);
        for character in *b"AB" {
            if unicode {
                bits.put(0, 8);
            }
            bits.put(character, 8);
        }
    }
    bits.put(u8::try_from(patterns.len()).unwrap(), 8);
    for &(id, repeats, instructions) in patterns {
        bits.put(0, 3);
        bits.put(id, 2);
        bits.put(repeats, 4);
        bits.put(u8::try_from(instructions.len()).unwrap(), 8);
        for instruction in instructions {
            match *instruction {
                Instruction::Note(pitch, duration, modifier) => {
                    bits.put(1, 3);
                    bits.put(pitch, 4);
                    bits.put(duration, 3);
                    bits.put(modifier, 2);
                }
                Instruction::Scale(value) => {
                    bits.put(2, 3);
                    bits.put(value, 2);
                }
                Instruction::Style(value) => {
                    bits.put(3, 3);
                    bits.put(value, 2);
                }
                Instruction::Tempo(value) => {
                    bits.put(4, 3);
                    bits.put(value, 5);
                }
                Instruction::Volume(value) => {
                    bits.put(5, 3);
                    bits.put(value, 4);
                }
            }
        }
    }
    bits.align();
    bits.put(0, 8);
    bits.0
}

pub fn short_tone() -> Vec<u8> {
    song(
        false,
        false,
        &[(
            0,
            0,
            &[
                Instruction::Tempo(12),
                Instruction::Scale(0),
                Instruction::Style(1),
                Instruction::Volume(15),
                Instruction::Note(10, 2, 0),
            ],
        )],
    )
}
