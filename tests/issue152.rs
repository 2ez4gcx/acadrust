use acadrust::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
use acadrust::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use acadrust::io::dwg::dwg_stream_writers::bit_writer::DwgBitWriter;
use acadrust::io::dwg::DwgVersion;
use acadrust::{Color, DxfVersion};

#[test]
fn r2007_cm_color_names_use_the_text_stream_and_keep_main_aligned() {
    let version = DxfVersion::AC1021;
    let mut main = DwgBitWriter::new(DwgVersion::AC21, version);
    main.write_bit_short(0);
    main.write_bit_long(0xC20D_1823u32 as i32);
    main.write_byte(3);
    main.write_bit_short(42);

    let mut text = DwgBitWriter::new(DwgVersion::AC21, version);
    text.write_variable_text("COLOR");
    text.write_variable_text("PANTONE+ CMYK Uncoated");

    let main = DwgBitReader::new(main.into_bytes(), DwgVersion::AC21, version);
    let text = DwgBitReader::new(text.into_bytes(), DwgVersion::AC21, version);
    let mut reader = DwgMergedReader::from_readers(main, Some(text), None, version);

    assert_eq!(reader.read_cm_color(), Color::from_rgb(13, 24, 35));
    assert_eq!(reader.read_bit_short(), 42);
}
