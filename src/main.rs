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
    let file_to_write = File::create(std::path::PathBuf::from(OUTPUT_NAME))?;

    let mut reader = BufReader::new(file_to_read);
    let mut writer = BufWriter::new(file_to_write);

    loop {
        let mut buffer = String::new();

        if let Ok(0) = reader.read_line(&mut buffer) {
            break;
        }

        let buffer = buffer.replace(&args.original_sep, &args.new_sep);
        writer.write(&buffer.as_bytes())?;
    }

    Ok(())
}
