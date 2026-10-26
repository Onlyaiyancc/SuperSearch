use super::ConversionOptions;
use encoding_rs::{GB18030, UTF_16BE, UTF_16LE};
use serde::Serialize;
use std::{
    borrow::Cow,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

pub(super) const CSV_INPUT_LIMIT: u64 = 32 * 1024 * 1024;
const CSV_OUTPUT_LIMIT: u64 = 128 * 1024 * 1024;
const ROW_LIMIT: usize = 100_000;
const COLUMN_LIMIT: usize = 1024;
const FIELD_LIMIT: usize = 1024 * 1024;
const PREVIEW_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvPreview {
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

pub(super) fn validate_selection(delimiter: &str, encoding: &str) -> Result<u8, String> {
    if !matches!(encoding, "utf-8" | "gb18030" | "utf-16le" | "utf-16be") {
        return Err("Conversion CSV invalid encoding selection".into());
    }
    match delimiter {
        "," => Ok(b','),
        ";" => Ok(b';'),
        "\t" => Ok(b'\t'),
        _ => Err("Conversion CSV invalid delimiter selection".into()),
    }
}

fn read_bounded(file: &mut File) -> Result<Vec<u8>, String> {
    let size = file
        .metadata()
        .map_err(|_| "Conversion CSV source unavailable")?
        .len();
    if size > CSV_INPUT_LIMIT {
        return Err("Conversion CSV input size limit exceeded".into());
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(CSV_INPUT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Conversion CSV source unavailable")?;
    if bytes.len() as u64 > CSV_INPUT_LIMIT {
        return Err("Conversion CSV input size limit exceeded".into());
    }
    Ok(bytes)
}

fn decode<'a>(bytes: &'a [u8], options: &ConversionOptions) -> Result<Cow<'a, str>, String> {
    validate_selection(&options.csv_delimiter, &options.csv_encoding)?;
    if bytes.len() as u64 > CSV_INPUT_LIMIT {
        return Err("Conversion CSV input size limit exceeded".into());
    }
    let (bom, bom_encoding) = if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        (3, Some("utf-8"))
    } else if bytes.starts_with(&[0xff, 0xfe]) {
        (2, Some("utf-16le"))
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        (2, Some("utf-16be"))
    } else {
        (0, None)
    };
    if bom_encoding.is_some_and(|encoding| encoding != options.csv_encoding) {
        return Err("Conversion CSV encoding does not match byte order mark".into());
    }
    let bytes = &bytes[bom..];
    let text = match options.csv_encoding.as_str() {
        "utf-8" => Cow::Borrowed(
            std::str::from_utf8(bytes).map_err(|_| "Conversion CSV invalid text encoding")?,
        ),
        "gb18030" => GB18030
            .decode_without_bom_handling_and_without_replacement(bytes)
            .ok_or("Conversion CSV invalid text encoding")?,
        "utf-16le" => UTF_16LE
            .decode_without_bom_handling_and_without_replacement(bytes)
            .ok_or("Conversion CSV invalid text encoding")?,
        "utf-16be" => UTF_16BE
            .decode_without_bom_handling_and_without_replacement(bytes)
            .ok_or("Conversion CSV invalid text encoding")?,
        _ => return Err("Conversion CSV invalid encoding selection".into()),
    };
    if text
        .chars()
        .any(|value| value.is_control() && !matches!(value, '\t' | '\r' | '\n'))
    {
        return Err("Conversion CSV invalid text encoding or control character".into());
    }
    Ok(text)
}

fn validate_quotes(text: &str, delimiter: u8) -> Result<(), String> {
    // csv 库有意容忍损坏引号；导入文档前额外拒绝歧义，防止截断字段被当作正常数据。
    let mut state = 0u8;
    for byte in text.bytes() {
        state = match state {
            0 => {
                if byte == b'"' {
                    2
                } else if byte == delimiter || matches!(byte, b'\r' | b'\n') {
                    0
                } else {
                    1
                }
            }
            1 => {
                if byte == b'"' {
                    return Err("Conversion CSV invalid quoting".into());
                } else if byte == delimiter || matches!(byte, b'\r' | b'\n') {
                    0
                } else {
                    1
                }
            }
            2 => {
                if byte == b'"' {
                    3
                } else {
                    2
                }
            }
            _ => {
                if byte == b'"' {
                    2
                } else if byte == delimiter || matches!(byte, b'\r' | b'\n') {
                    0
                } else {
                    return Err("Conversion CSV invalid quoting".into());
                }
            }
        };
    }
    if state == 2 {
        return Err("Conversion CSV unterminated quoted field".into());
    }
    Ok(())
}

fn records(
    text: &str,
    delimiter: u8,
    mut visit: impl FnMut(&::csv::StringRecord) -> Result<(), String>,
) -> Result<(), String> {
    validate_quotes(text, delimiter)?;
    let mut reader = ::csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(false)
        .from_reader(text.as_bytes());
    let mut count = 0usize;
    for record in reader.records() {
        let record =
            record.map_err(|_| "Conversion CSV malformed record or inconsistent column count")?;
        count += 1;
        if count > ROW_LIMIT
            || record.len() > COLUMN_LIMIT
            || record.iter().any(|cell| cell.len() > FIELD_LIMIT)
        {
            return Err("Conversion CSV row column or field size limit exceeded".into());
        }
        visit(&record)?;
    }
    if count == 0 {
        return Err("Conversion CSV empty input".into());
    }
    Ok(())
}

fn preview_bytes(bytes: &[u8], options: &ConversionOptions) -> Result<CsvPreview, String> {
    let delimiter = validate_selection(&options.csv_delimiter, &options.csv_encoding)?;
    let text = decode(bytes, options)?;
    let mut preview = CsvPreview {
        rows: Vec::new(),
        truncated: false,
    };
    let mut budget = PREVIEW_BYTES;
    records(&text, delimiter, |record| {
        if preview.rows.len() >= 20 {
            preview.truncated = true;
            return Ok(());
        }
        let mut row = Vec::new();
        for cell in record.iter().take(20) {
            let end = if cell.len() <= budget {
                cell.len()
            } else {
                preview.truncated = true;
                cell.char_indices()
                    .map(|(index, value)| index + value.len_utf8())
                    .take_while(|end| *end <= budget)
                    .last()
                    .unwrap_or(0)
            };
            row.push(cell[..end].to_owned());
            budget -= end;
        }
        if record.len() > 20 {
            preview.truncated = true;
        }
        preview.rows.push(row);
        Ok(())
    })?;
    Ok(preview)
}

pub(super) fn preview_file(
    file: &mut File,
    options: &ConversionOptions,
) -> Result<CsvPreview, String> {
    preview_bytes(&read_bounded(file)?, options)
}

struct OutputBudget {
    inner: File,
    bytes: u64,
}
impl Write for OutputBudget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.saturating_add(bytes.len() as u64) > CSV_OUTPUT_LIMIT {
            return Err(io::Error::other(
                "CSV normalized output size limit exceeded",
            ));
        }
        let count = self.inner.write(bytes)?;
        self.bytes += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(super) fn normalize(
    source: &Path,
    output: &Path,
    options: &ConversionOptions,
) -> Result<(), String> {
    options.validate()?;
    let mut source = File::open(source).map_err(|_| "Conversion CSV source unavailable")?;
    let bytes = read_bounded(&mut source)?;
    let text = decode(&bytes, options)?;
    let delimiter = validate_selection(&options.csv_delimiter, &options.csv_encoding)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|_| "Conversion CSV output unavailable")?;
    let result = (|| {
        // 与原生端固定的“引号字段作为文本、禁止公式与特殊数值推断”导入参数配合。
        let mut writer = ::csv::WriterBuilder::new()
            .delimiter(b',')
            .quote_style(::csv::QuoteStyle::Always)
            .terminator(::csv::Terminator::CRLF)
            .from_writer(OutputBudget {
                inner: file,
                bytes: 0,
            });
        records(&text, delimiter, |record| {
            writer
                .write_record(record)
                .map_err(|_| "Conversion CSV normalized output size limit or write failure".into())
        })?;
        writer
            .flush()
            .map_err(|_| "Conversion CSV output write failed")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(output);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion::ConversionOptions;
    use std::fs;

    fn directory() -> std::path::PathBuf {
        let path = std::env::current_dir()
            .unwrap()
            .join("../test-results")
            .join(format!("conversion-csv-{}", crate::conversion::unique_id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn gb18030_semicolon_and_quoted_newlines_preserve_literal_field_values() {
        let root = directory();
        let source = root.join("source.csv");
        let output = root.join("normalized.csv");
        let input = "名称;备注;公式\r\n中文;\"第一行\n第二行;\"\"引用\"\"\";=1+2\r\n";
        let (bytes, _, errors) = encoding_rs::GB18030.encode(input);
        assert!(!errors);
        fs::write(&source, &bytes).unwrap();
        let options = ConversionOptions {
            csv_delimiter: ";".into(),
            csv_encoding: "gb18030".into(),
            ..Default::default()
        };
        normalize(&source, &output, &options).unwrap();
        let normalized = fs::read_to_string(&output).unwrap();
        assert!(normalized.contains("\"=1+2\""));
        let rows = ::csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(normalized.as_bytes())
            .records()
            .map(|record| {
                record
                    .unwrap()
                    .iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(rows[1], vec!["中文", "第一行\n第二行;\"引用\"", "=1+2"]);
        assert_eq!(fs::read(&source).unwrap(), bytes.as_ref());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_utf16_endianness_and_tab_delimiter_decode_without_replacement() {
        let text = "名称\t值\r\n中文\t00123\r\n";
        for (encoding, little, bom) in [
            ("utf-16le", true, [0xff, 0xfe]),
            ("utf-16be", false, [0xfe, 0xff]),
        ] {
            let mut bytes = bom.to_vec();
            for value in text.encode_utf16() {
                bytes.extend_from_slice(&if little {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                });
            }
            let options = ConversionOptions {
                csv_delimiter: "\t".into(),
                csv_encoding: encoding.into(),
                ..Default::default()
            };
            let preview = preview_bytes(&bytes, &options).unwrap();
            assert_eq!(preview.rows[1], vec!["中文", "00123"]);
            assert!(!preview.truncated);
            assert!(preview_bytes(&bytes, &ConversionOptions::default()).is_err());
        }
    }

    #[test]
    fn invalid_encoding_bad_selection_and_broken_csv_are_rejected() {
        assert!(preview_bytes(
            &[0xff, 0xfe, 0x41],
            &ConversionOptions {
                csv_encoding: "utf-16le".into(),
                ..Default::default()
            }
        )
        .is_err());
        assert!(preview_bytes(&[0xc3, 0x28], &ConversionOptions::default()).is_err());
        assert!(preview_bytes(b"a,b\n1,\"unterminated", &ConversionOptions::default()).is_err());
        assert!(preview_bytes(b"a,b\n1", &ConversionOptions::default()).is_err());
        for options in [
            ConversionOptions {
                csv_delimiter: "|".into(),
                ..Default::default()
            },
            ConversionOptions {
                csv_encoding: "system".into(),
                ..Default::default()
            },
        ] {
            assert!(preview_bytes(b"a,b", &options).is_err());
        }
    }

    #[test]
    fn preview_bounds_rows_columns_utf8_bytes_and_validates_remaining_rows() {
        let options = ConversionOptions::default();
        let text = (0..25)
            .map(|_| {
                (0..25)
                    .map(|_| "中文".repeat(800))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let preview = preview_bytes(text.as_bytes(), &options).unwrap();
        assert_eq!(preview.rows.len(), 20);
        assert!(preview.rows.iter().all(|row| row.len() == 20));
        assert!(preview.truncated);
        assert!(
            preview
                .rows
                .iter()
                .flatten()
                .map(String::len)
                .sum::<usize>()
                <= 64 * 1024
        );
        let malformed = format!(
            "{}\nwrong,column,count",
            (0..21).map(|_| "a,b").collect::<Vec<_>>().join("\n")
        );
        assert!(preview_bytes(malformed.as_bytes(), &options).is_err());
        assert!(preview_bytes(&vec![b'a'; CSV_INPUT_LIMIT as usize + 1], &options).is_err());
        assert!(preview_bytes("a,".repeat(1024).as_bytes(), &options).is_err());
    }

    #[test]
    fn full_import_rejects_oversized_fields_rows_and_removes_partial_output() {
        let options = ConversionOptions::default();
        assert!(preview_bytes(&vec![b'a'; FIELD_LIMIT + 1], &options).is_err());
        assert!(preview_bytes("a\n".repeat(ROW_LIMIT + 1).as_bytes(), &options).is_err());
        let root = directory();
        let source = root.join("bad.csv");
        let output = root.join("out.csv");
        fs::write(&source, "a,b\n1,2\nlast,extra,column").unwrap();
        assert!(normalize(&source, &output, &options).is_err());
        assert!(!output.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
