use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::iter::ParallelIterator;
use rayon::prelude::*;
use std::fmt;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::{
    fs::{read_dir, File},
    io::{BufRead, BufReader, BufWriter, Write},
};
use unicode_segmentation::UnicodeSegmentation;

use clap::Parser;

const DEFAULT_NAME: &str = "formatted";

/// CSV Delimiter Converter.
#[derive(Parser, Debug)]
struct Cli {
    /// Original string delimiter. Must be one character.
    original_delimiter: String,

    /// New string delimiter. Must be one character.
    new_delimiter: String,

    /// File or directory path. Directory search is recursive.
    path: std::path::PathBuf,

    /// Name of the output file (resp. directory) for the formatted result (resp. results).
    #[arg(short, long, default_value_t = String::from(DEFAULT_NAME))]
    output: String,

    /// Checks if the file is valid csv by counting delimiters in the lines.
    #[arg(short, long)]
    check: bool,
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
            original_sep: value.original_delimiter.to_owned(),
            new_sep: value.new_delimiter.to_owned(),
            path: value.path.to_owned(),
            check: value.check,
        }
    }
}

fn main() -> Result<(), FileError> {
    let mut args = Cli::parse();

    if !args.path.is_dir() && args.output == DEFAULT_NAME {
        args.output = String::from(format!("{}.csv", DEFAULT_NAME))
    }

    let args = args;

    if let Some(input_name) = args.path.to_str() {
        if input_name == args.output {
            return Err(FileError::OutputWithSameName(OutputWithSameNameError {}));
        }
    }

    if !args.path.is_dir() {
        let pb = MultiProgress::new();
        return parse_file(&CliInfo::new(&args), PathBuf::from(args.output), &pb);
    } else {
        let parent = args.path.parent().expect("Could not get parent folder.");

        create_dir_all(parent.join(&args.output))?;

        let pb = MultiProgress::new();

        visit_dirs(&args.path, &args, parent, &pb)?
    }

    Ok(())
}

fn process_file(
    args: &Cli,
    file: &Result<std::fs::DirEntry, std::io::Error>,
    parent: &std::path::Path,
    pb: &MultiProgress,
) {
    let mut cli_info = CliInfo::new(args);
    let file = file.as_ref().ok();

    if file.is_none() {
        println!("File couldn't be processed.");
    }

    let file = file.unwrap();
    cli_info.path = file.path();

    if let Err(error) = parse_file(&cli_info, parent.join(&args.output).join(file.path()), pb) {
        println!("{:?}", cli_info.path);
        println!("{:?}", parent.join(&args.output).join(file.path()));
        println!(
            "File {:?} couldn't be processed. {:?}.",
            file.file_name(),
            error
        );
    }
}

fn parse_file(args: &CliInfo, file_to_write: PathBuf, pb: &MultiProgress) -> Result<(), FileError> {
    create_dir_all(
        file_to_write
            .parent()
            .expect("Could not get parent folder."),
    )?;

    if args.new_sep.graphemes(true).count() != 1 {
        return Err(FileError::Delimiter(DelimiterError {
            invalid_delimiter: args.new_sep.to_owned(),
        }));
    }

    let file_to_read = File::open(&args.path)?;

    let total_size = file_to_read.metadata()?.len();
    let pb2 = pb.add(ProgressBar::new(total_size));

    let sty = ProgressStyle::with_template(
        "{spinner:.green} [{msg} {wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
    )
    .unwrap()
    .progress_chars("#>-");

    pb2.set_style(sty);

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
        LineProcessingResult::EndOfFile => {
            pb2.finish_with_message(format!("File {:?} done!", &args.path.file_name().unwrap()));
            Ok(())
        }
        LineProcessingResult::Some(count_from_first_line) => run_lines_with_consistency_check(
            &mut reader,
            &mut writer,
            &mut size_seen,
            &pb2,
            args,
            count_from_first_line,
        ),
        LineProcessingResult::Any => {
            run_lines_without_consistency_check(reader, writer, size_seen, &pb2, args)
        }
    }
}

fn run_lines_without_consistency_check(
    mut reader: BufReader<File>,
    mut writer: BufWriter<File>,
    mut size_seen: usize,
    pb: &ProgressBar,
    args: &CliInfo,
) -> Result<(), FileError> {
    loop {
        let next_line_result = process_line(
            &mut reader,
            &mut writer,
            &mut size_seen,
            pb,
            &args.original_sep,
            &args.new_sep,
            args.check,
        )?;

        if let LineProcessingResult::EndOfFile = next_line_result {
            pb.finish_and_clear();

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
                    let error = CountError {
                        delimiters_at_header: number_to_compare,
                        delimiters_at_line: number_in_this_line,
                        line_number,
                    };
                    pb.finish_with_message(format!(
                        "File {:?}: {error:?}",
                        args.path.file_name().unwrap()
                    ));
                    break;
                }
            }
            LineProcessingResult::EndOfFile => {
                pb.finish_with_message(format!("File {:?} done!", args.path.file_name().unwrap()));
                break;
            }
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

fn visit_dirs(
    dir: &PathBuf,
    args: &Cli,
    parent: &std::path::Path,
    pb: &MultiProgress,
) -> Result<(), FileError> {
    if dir.is_dir() {
        read_dir(dir)?.par_bridge().for_each(|file| {
            if let Ok(x) = file {
                let path = x.path();
                if path.is_dir() {
                    visit_dirs(&path, args, parent, pb)
                        .expect("Unexpected non-directory inside visit_dirs loop");
                } else {
                    process_file(args, &Ok(x), parent, pb);
                }
            }
        });

        Ok(())
    } else {
        Err(FileError::FileIsDirectory(FileIsDirectoryError {}))
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
    OutputWithSameName(OutputWithSameNameError),
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

#[derive(Debug)]
pub struct OutputWithSameNameError {}

impl fmt::Display for OutputWithSameNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt("", f)
    }
}

impl std::error::Error for OutputWithSameNameError {}
