use crate::support::guest::{utf8, write_fixture};

#[test]
fn input_stream_reader_inherits_reader_skip_and_advances_by_characters() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "input_stream_reader_inherits_reader_skip_and_advances_by_characters"
    )) {
        return;
    }
    let scratch = crate::support::Scratch::new();
    let directory = &scratch.0;
    let jar_path = directory.join("fixture.jar");
    write_fixture(
        &jar_path,
        b"Manifest-Version: 1.0\r\n\r\n",
        "ReaderSkipFixture.class",
        &reader_skip_fixture_class(),
    );

    let output =
        crate::support::Fixture::new(&jar_path).run_static("ReaderSkipFixture", "main", "()I");
    assert!(output.success(), "{}", output.diagnostics);
    let diagnostics = &output.diagnostics;
    // UTF-8 bytes encode 'A', '©', 'B'. Skipping two Reader characters
    // must report two and leave 'B' as the next decoded character.
    assert_eq!(output.int_value(), Some(266), "{diagnostics}");
}

#[test]
fn data_input_stream_instance_read_utf_accepts_literal_nul() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "data_input_stream_instance_read_utf_accepts_literal_nul"
    )) {
        return;
    }
    let scratch = crate::support::Scratch::new();
    let directory = &scratch.0;
    let jar_path = directory.join("fixture.jar");
    write_fixture(
        &jar_path,
        b"Manifest-Version: 1.0\r\n\r\n",
        "ReadUtfFixture.class",
        &read_utf_fixture_class(),
    );

    let output =
        crate::support::Fixture::new(&jar_path).run_static("ReadUtfFixture", "main", "()I");
    assert!(output.success(), "{}", output.diagnostics);
    let diagnostics = &output.diagnostics;
    // The three decoded UTF-16 units are 'A', literal NUL and 'Z'.
    assert_eq!(output.int_value(), Some(390), "{diagnostics}");
}

fn reader_skip_fixture_class() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0xcafe_babe_u32.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&50_u16.to_be_bytes());
    bytes.extend_from_slice(&28_u16.to_be_bytes());
    utf8(&mut bytes, "ReaderSkipFixture"); // #1
    bytes.extend_from_slice(&[7, 0, 1]); // #2 Class ReaderSkipFixture
    utf8(&mut bytes, "java/lang/Object"); // #3
    bytes.extend_from_slice(&[7, 0, 3]); // #4 Class Object
    utf8(&mut bytes, "main"); // #5
    utf8(&mut bytes, "()I"); // #6
    utf8(&mut bytes, "Code"); // #7
    utf8(&mut bytes, "java/io/ByteArrayInputStream"); // #8
    bytes.extend_from_slice(&[7, 0, 8]); // #9 Class ByteArrayInputStream
    utf8(&mut bytes, "<init>"); // #10
    utf8(&mut bytes, "([B)V"); // #11
    bytes.extend_from_slice(&[12, 0, 10, 0, 11]); // #12 name and type
    bytes.extend_from_slice(&[10, 0, 9, 0, 12]); // #13 ByteArrayInputStream.<init>
    utf8(&mut bytes, "java/io/InputStreamReader"); // #14
    bytes.extend_from_slice(&[7, 0, 14]); // #15 Class InputStreamReader
    utf8(&mut bytes, "(Ljava/io/InputStream;Ljava/lang/String;)V"); // #16
    bytes.extend_from_slice(&[12, 0, 10, 0, 16]); // #17 name and type
    bytes.extend_from_slice(&[10, 0, 15, 0, 17]); // #18 InputStreamReader.<init>
    utf8(&mut bytes, "UTF-8"); // #19
    bytes.extend_from_slice(&[8, 0, 19]); // #20 String UTF-8
    utf8(&mut bytes, "skip"); // #21
    utf8(&mut bytes, "(J)J"); // #22
    bytes.extend_from_slice(&[12, 0, 21, 0, 22]); // #23 name and type
    bytes.extend_from_slice(&[10, 0, 15, 0, 23]); // #24 InputStreamReader.skip
    utf8(&mut bytes, "read"); // #25
    bytes.extend_from_slice(&[12, 0, 25, 0, 6]); // #26 name and type
    bytes.extend_from_slice(&[10, 0, 15, 0, 26]); // #27 InputStreamReader.read

    bytes.extend_from_slice(&0x0021_u16.to_be_bytes());
    bytes.extend_from_slice(&2_u16.to_be_bytes());
    bytes.extend_from_slice(&4_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&0x0009_u16.to_be_bytes());
    bytes.extend_from_slice(&5_u16.to_be_bytes());
    bytes.extend_from_slice(&6_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&7_u16.to_be_bytes());
    bytes.extend_from_slice(&70_u32.to_be_bytes());
    bytes.extend_from_slice(&5_u16.to_be_bytes());
    bytes.extend_from_slice(&2_u16.to_be_bytes());
    bytes.extend_from_slice(&58_u32.to_be_bytes());
    bytes.extend_from_slice(&[
        0x07, 0xbc, 0x08, 0x4b, // byte[] bytes = new byte[4]
        0x2a, 0x03, 0x10, 0x41, 0x54, // bytes[0] = 'A'
        0x2a, 0x04, 0x10, 0xc2, 0x54, // bytes[1] = 0xc2
        0x2a, 0x05, 0x10, 0xa9, 0x54, // bytes[2] = 0xa9
        0x2a, 0x06, 0x10, 0x42, 0x54, // bytes[3] = 'B'
        0xbb, 0x00, 0x0f, 0x59, // new InputStreamReader, dup
        0xbb, 0x00, 0x09, 0x59, 0x2a, 0xb7, 0x00, 0x0d, // new byte stream
        0x12, 0x14, 0xb7, 0x00, 0x12, 0x4c, // UTF-8 reader
        0x2b, 0x05, 0x85, 0xb6, 0x00, 0x18, // reader.skip(2)
        0x88, 0x10, 0x64, 0x68, // (int) skipped * 100
        0x2b, 0xb6, 0x00, 0x1b, 0x60, 0xac, // + reader.read()
    ]);
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes
}

fn read_utf_fixture_class() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0xcafe_babe_u32.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&50_u16.to_be_bytes());
    bytes.extend_from_slice(&32_u16.to_be_bytes());
    utf8(&mut bytes, "ReadUtfFixture"); // #1
    bytes.extend_from_slice(&[7, 0, 1]); // #2 Class ReadUtfFixture
    utf8(&mut bytes, "java/lang/Object"); // #3
    bytes.extend_from_slice(&[7, 0, 3]); // #4 Class Object
    utf8(&mut bytes, "main"); // #5
    utf8(&mut bytes, "()I"); // #6
    utf8(&mut bytes, "Code"); // #7
    utf8(&mut bytes, "java/io/ByteArrayInputStream"); // #8
    bytes.extend_from_slice(&[7, 0, 8]); // #9 Class ByteArrayInputStream
    utf8(&mut bytes, "<init>"); // #10
    utf8(&mut bytes, "([B)V"); // #11
    bytes.extend_from_slice(&[12, 0, 10, 0, 11]); // #12 name and type
    bytes.extend_from_slice(&[10, 0, 9, 0, 12]); // #13 ByteArrayInputStream.<init>
    utf8(&mut bytes, "java/io/DataInputStream"); // #14
    bytes.extend_from_slice(&[7, 0, 14]); // #15 Class DataInputStream
    utf8(&mut bytes, "(Ljava/io/InputStream;)V"); // #16
    bytes.extend_from_slice(&[12, 0, 10, 0, 16]); // #17 name and type
    bytes.extend_from_slice(&[10, 0, 15, 0, 17]); // #18 DataInputStream.<init>
    utf8(&mut bytes, "readUTF"); // #19
    utf8(&mut bytes, "()Ljava/lang/String;"); // #20
    bytes.extend_from_slice(&[12, 0, 19, 0, 20]); // #21 name and type
    bytes.extend_from_slice(&[10, 0, 15, 0, 21]); // #22 DataInputStream.readUTF
    utf8(&mut bytes, "length"); // #23
    bytes.extend_from_slice(&[12, 0, 23, 0, 6]); // #24 name and type
    utf8(&mut bytes, "java/lang/String"); // #25
    bytes.extend_from_slice(&[7, 0, 25]); // #26 Class String
    bytes.extend_from_slice(&[10, 0, 26, 0, 24]); // #27 String.length
    utf8(&mut bytes, "charAt"); // #28
    utf8(&mut bytes, "(I)C"); // #29
    bytes.extend_from_slice(&[12, 0, 28, 0, 29]); // #30 name and type
    bytes.extend_from_slice(&[10, 0, 26, 0, 30]); // #31 String.charAt

    bytes.extend_from_slice(&0x0021_u16.to_be_bytes());
    bytes.extend_from_slice(&2_u16.to_be_bytes());
    bytes.extend_from_slice(&4_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&0x0009_u16.to_be_bytes());
    bytes.extend_from_slice(&5_u16.to_be_bytes());
    bytes.extend_from_slice(&6_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&7_u16.to_be_bytes());
    let code = [
        0x08, 0xbc, 0x08, 0x4b, // byte[] bytes = new byte[5]
        0x2a, 0x03, 0x03, 0x54, // bytes[0] = 0 (UTF byte length high)
        0x2a, 0x04, 0x06, 0x54, // bytes[1] = 3 (UTF byte length low)
        0x2a, 0x05, 0x10, 0x41, 0x54, // bytes[2] = 'A'
        0x2a, 0x06, 0x03, 0x54, // bytes[3] = literal NUL
        0x2a, 0x07, 0x10, 0x5a, 0x54, // bytes[4] = 'Z'
        0xbb, 0x00, 0x0f, 0x59, // new DataInputStream, dup
        0xbb, 0x00, 0x09, 0x59, 0x2a, 0xb7, 0x00, 0x0d, // new byte stream
        0xb7, 0x00, 0x12, // DataInputStream.<init>(InputStream)
        0xb6, 0x00, 0x16, 0x4c, // String text = input.readUTF()
        0x2b, 0xb6, 0x00, 0x1b, 0x10, 0x64, 0x68, // text.length() * 100
        0x2b, 0x04, 0xb6, 0x00, 0x1f, 0x60, // + text.charAt(1)
        0x2b, 0x05, 0xb6, 0x00, 0x1f, 0x60, 0xac, // + text.charAt(2)
    ];
    bytes.extend_from_slice(&(12_u32 + u32::try_from(code.len()).unwrap()).to_be_bytes());
    bytes.extend_from_slice(&5_u16.to_be_bytes());
    bytes.extend_from_slice(&2_u16.to_be_bytes());
    bytes.extend_from_slice(&u32::try_from(code.len()).unwrap().to_be_bytes());
    bytes.extend_from_slice(&code);
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes
}
