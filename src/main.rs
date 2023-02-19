use indicatif::ProgressBar;
use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
};

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

    let file_to_read = File::open(&args.path)?;

    let total_size = file_to_read.metadata()?.len();
    let mut size_seen = 0;
    let pb = ProgressBar::new(total_size);

    let file_to_write = File::create(std::path::PathBuf::from(OUTPUT_NAME))?;

    let mut reader = BufReader::new(file_to_read);
    let mut writer = BufWriter::new(file_to_write);

    loop {
        let mut buffer = String::new();

        if let Ok(0) = reader.read_line(&mut buffer) {
            break;
        }

        size_seen += buffer.len();
        pb.set_position(size_seen as u64);

        let buffer = buffer.replace(&args.original_sep, &args.new_sep);
        writer.write(&buffer.as_bytes())?;
    }

    Ok(())
}
