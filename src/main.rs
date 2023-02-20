use indicatif::ProgressBar;
use std::fmt;
use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
};
use unicode_segmentation::UnicodeSegmentation;

use clap::Parser;

const OUTPUT_NAME: &str = "new.csv";

#[derive(Parser)]
struct Cli {
    original_sep: String,
    new_sep: String,
    path: std::path::PathBuf,

    #[arg(short, long)]
    check: bool,
}

fn main() -> Result<(), FileError> {
    let args = Cli::parse();

    if args.new_sep.graphemes(true).count() != 1 {
        panic!()
    }

    let file_to_read = File::open(&args.path)?;

    let total_size = file_to_read.metadata()?.len();
    let mut size_seen = 0;
    let pb = ProgressBar::new(total_size);

    let file_to_write = File::create(std::path::PathBuf::from(OUTPUT_NAME))?;

    let mut reader = BufReader::new(file_to_read);
    let mut writer = BufWriter::new(file_to_write);

    let first_line_result = process_line(
        &mut reader,
        &mut writer,
        &mut size_seen,
        &pb,
        &args.original_sep,
        &args.new_sep,
        args.check,
    )?;

    match first_line_result {
        LineProcessingResult::EndOfFile => Ok(()),
        LineProcessingResult::Some(count_from_first_line) => {
            run_lines_with_consistency_check(
                &mut reader,
                &mut writer,
                &mut size_seen,
                &pb,
                &args,
                count_from_first_line,
            )?;

            Ok(())
        }
        LineProcessingResult::Any => {
            run_lines_without_consistency_check(reader, writer, size_seen, pb, args)?;

            Ok(())
        }
    }
}

fn run_lines_without_consistency_check(
    mut reader: BufReader<File>,
    mut writer: BufWriter<File>,
    mut size_seen: usize,
    pb: ProgressBar,
    args: Cli,
) -> Result<(), std::io::Error> {
    loop {
        let next_line_result = process_line(
            &mut reader,
            &mut writer,
            &mut size_seen,
            &pb,
            &args.original_sep,
            &args.new_sep,
            args.check,
        )?;

        if let LineProcessingResult::EndOfFile = next_line_result {
            break;
        }
    }

    Ok(())
}

fn run_lines_with_consistency_check(
    reader: &mut BufReader<File>,
    writer: &mut BufWriter<File>,
    size_seen: &mut usize,
    pb: &ProgressBar,
    args: &Cli,
    number_to_compare: usize,
) -> Result<(), FileError> {
    let mut line_number = 2;

    loop {
        let next_line_result = process_line(
            reader,
            writer,
            size_seen,
            pb,
            &args.original_sep,
            &args.new_sep,
            args.check,
        )?;

        match next_line_result {
            LineProcessingResult::Some(number_in_this_line) => {
                if number_to_compare != number_in_this_line {
                    return Err(FileError::DifferentCount(CountError {
                        delimiters_at_header: number_to_compare,
                        delimiters_at_line: number_in_this_line,
                        line_number,
                    }));
                }
            }
            LineProcessingResult::EndOfFile => break,
            LineProcessingResult::Any => (),
        }

        line_number += 1;
    }

    Ok(())
}

fn get_number_of_delimiters(buffer: &str, original_sep: &str) -> usize {
    buffer
        .graphemes(true)
        .filter(|&x| x == original_sep.graphemes(true).last().unwrap())
        .count()
}

fn process_line(
    reader: &mut BufReader<File>,
    writer: &mut BufWriter<File>,
    size_seen: &mut usize,
    pb: &ProgressBar,
    original_sep: &str,
    new_sep: &str,
    check_consistency: bool,
) -> Result<LineProcessingResult, std::io::Error> {
    let mut buffer = String::new();

    if let Ok(0) = reader.read_line(&mut buffer) {
        return Ok(LineProcessingResult::EndOfFile);
    }

    if check_consistency {
        let number_of_delimiters = get_number_of_delimiters(&buffer, original_sep);

        let buffer = buffer.replace(original_sep, new_sep);
        writer.write_all(buffer.as_bytes())?;

        *size_seen += buffer.len();
        pb.set_position(*size_seen as u64);

        Ok(LineProcessingResult::Some(number_of_delimiters))
    } else {
        let buffer = buffer.replace(original_sep, new_sep);
        writer.write_all(buffer.as_bytes())?;

        *size_seen += buffer.len();
        pb.set_position(*size_seen as u64);

        Ok(LineProcessingResult::Any)
    }
}

enum LineProcessingResult {
    Some(usize),
    Any,
    EndOfFile,
}

#[derive(Debug)]
enum FileError {
    IoError(std::io::Error),
    DifferentCount(CountError),
}

impl From<std::io::Error> for FileError {
    fn from(err: std::io::Error) -> FileError {
        FileError::IoError(err)
    }
}

impl From<CountError> for FileError {
    fn from(err: CountError) -> FileError {
        FileError::DifferentCount(err)
    }
}

#[derive(Debug)]
pub struct CountError {
    delimiters_at_header: usize,
    delimiters_at_line: usize,
    line_number: usize,
}

impl fmt::Display for CountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(
            &format!(
                "{} delimiters at header, while {} at line {}.",
                self.delimiters_at_header, self.delimiters_at_line, self.line_number
            ),
            f,
        )
    }
}

impl std::error::Error for CountError {}
