use indicatif::{MultiProgress, ProgressBar};
use itertools::Itertools;
use std::fmt;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::thread;
use std::thread::available_parallelism;
use std::{
    fs::{read_dir, File},
    io::{BufRead, BufReader, BufWriter, Write},
};
use unicode_segmentation::UnicodeSegmentation;

use clap::Parser;

const OUTPUT_NAME: &str = "formatted.csv";
const OUTPUT_FOLDER: &str = "formatted";

#[derive(Parser)]
struct Cli {
    original_sep: String,
    new_sep: String,
    path: std::path::PathBuf,

    #[arg(short, long)]
    check: bool,

    #[arg(short, long)]
    dir: bool,
}

struct CliInfo {
    original_sep: String,
    new_sep: String,
    path: std::path::PathBuf,
    check: bool,
}

impl CliInfo {
    fn new(value: &Cli) -> Self {
        CliInfo {
            original_sep: value.original_sep.to_owned(),
            new_sep: value.new_sep.to_owned(),
            path: value.path.to_owned(),
            check: value.check,
        }
    }
}

fn main() -> Result<(), FileError> {
    let args = Cli::parse();

    if !args.dir {
        if *&args.path.is_dir() {
            return Err(FileError::FileIsDirectory(FileIsDirectoryError {}));
        }

        let pb = MultiProgress::new();
        return parse_file(&CliInfo::new(&args), PathBuf::from(OUTPUT_NAME), &pb);
    } else {
        let parent = args.path.parent().expect("Could not get parent folder.");

        create_dir_all(parent.join(OUTPUT_FOLDER))?;

        let n_parallel = match available_parallelism() {
            Ok(non_zero) => non_zero.get(),
            _ => 1,
        };

        let files_chunks = read_dir(PathBuf::from(args.path.to_owned()))?.chunks(n_parallel);
        let files_chunks = files_chunks.into_iter();

        for files in files_chunks {
            let pb = MultiProgress::new();

            thread::scope(|s| {
                for file in files {
                    s.spawn(|| {
                        process_file(&args, file, parent, &pb);
                    });
                }
            });
        }
    }

    Ok(())
}

fn process_file(
    args: &Cli,
    file: Result<std::fs::DirEntry, std::io::Error>,
    parent: &std::path::Path,
    pb: &MultiProgress,
) {
    let mut cli_info = CliInfo::new(args);
    let file = file.ok();

    if let None = file {
        println!("File couldn't be processed.");
    }

    let file = file.unwrap();
    cli_info.path = file.path();

    if let Err(error) = parse_file(
        &cli_info,
        parent.join(OUTPUT_FOLDER).join(file.file_name()),
        pb,
    ) {
        println!(
            "File {:?} couldn't be processed. {:?}.",
            file.file_name(),
            error
        );
    }
}

fn parse_file(args: &CliInfo, file_to_write: PathBuf, pb: &MultiProgress) -> Result<(), FileError> {
    if args.new_sep.graphemes(true).count() != 1 {
        return Err(FileError::Delimiter(DelimiterError {
            invalid_delimiter: args.new_sep.to_owned(),
        }));
    }

    let file_to_read = File::open(&args.path)?;

    let total_size = file_to_read.metadata()?.len();
    let pb2 = pb.add(ProgressBar::new(total_size));
    let mut size_seen = 0;

    let file_to_write = File::create(file_to_write)?;

    let mut reader = BufReader::new(file_to_read);
    let mut writer = BufWriter::new(file_to_write);

    let first_line_result = process_line(
        &mut reader,
        &mut writer,
        &mut size_seen,
        &pb2,
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
                &pb2,
                &args,
                count_from_first_line,
            )?;

            Ok(())
        }
        LineProcessingResult::Any => {
            run_lines_without_consistency_check(reader, writer, size_seen, &pb2, args)?;

            Ok(())
        }
    }
}

fn run_lines_without_consistency_check(
    mut reader: BufReader<File>,
    mut writer: BufWriter<File>,
    mut size_seen: usize,
    pb: &ProgressBar,
    args: &CliInfo,
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
    args: &CliInfo,
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
    Delimiter(DelimiterError),
    FileIsDirectory(FileIsDirectoryError),
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

#[derive(Debug)]
pub struct DelimiterError {
    invalid_delimiter: String,
}

impl fmt::Display for DelimiterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(
            &format!(
                "{} is not a valid delimiter. Please select a one-character delimiter",
                self.invalid_delimiter
            ),
            f,
        )
    }
}

impl std::error::Error for DelimiterError {}

#[derive(Debug)]
pub struct FileIsDirectoryError {}

impl fmt::Display for FileIsDirectoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt("", f)
    }
}

impl std::error::Error for FileIsDirectoryError {}
