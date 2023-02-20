use indicatif::ProgressBar;
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

fn main() -> std::io::Result<()> {
    let args = Cli::parse();

    if !(args.new_sep.graphemes(true).count() == 1) {
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
        LineProcessingResult::Some(x) => {
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

                match next_line_result {
                    LineProcessingResult::Some(y) => {
                        if x != y {
                            panic!()
                        }
                    }
                    LineProcessingResult::EndOfFile => break,
                    LineProcessingResult::Any => (),
                }
            }

            Ok(())
        }
        LineProcessingResult::Any => {
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

                match next_line_result {
                    LineProcessingResult::EndOfFile => break,
                    _ => (),
                }
            }

            Ok(())
        }
    }
}

fn get_number_of_delimiters(buffer: &String, original_sep: &String) -> usize {
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
    original_sep: &String,
    new_sep: &String,
    check_consistency: bool,
) -> Result<LineProcessingResult, std::io::Error> {
    let mut buffer = String::new();

    if let Ok(0) = reader.read_line(&mut buffer) {
        return Ok(LineProcessingResult::EndOfFile);
    }

    if check_consistency {
        let number_of_delimiters = get_number_of_delimiters(&buffer, original_sep);

        let buffer = buffer.replace(original_sep, new_sep);
        writer.write(&buffer.as_bytes())?;

        *size_seen += buffer.len();
        pb.set_position(*size_seen as u64);

        Ok(LineProcessingResult::Some(number_of_delimiters))
    } else {
        let buffer = buffer.replace(original_sep, new_sep);
        writer.write(&buffer.as_bytes())?;

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
