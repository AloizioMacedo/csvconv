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

    let mut count = 0;

    loop {
        let mut buffer = String::new();

        if let Ok(0) = reader.read_line(&mut buffer) {
            break;
        }

        size_seen += buffer.len();
        pb.set_position(size_seen as u64);

        let this_count = buffer
            .graphemes(true)
            .filter(|&x| x == args.original_sep.graphemes(true).last().unwrap())
            .count();

        if count == 0 {
            count = this_count
        } else {
            if this_count != count {
                panic!()
            }
        }

        let buffer = buffer.replace(&args.original_sep, &args.new_sep);
        writer.write(&buffer.as_bytes())?;
    }

    Ok(())
}
