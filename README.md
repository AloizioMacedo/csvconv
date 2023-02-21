# Introduction

CSV Delimiter Converter (csvconv) is a simple, fast CLI tool that converts csv files to different limiters.

It processes files via multithreading using [Rayon](https://github.com/rayon-rs/rayon), and can identify invalid csv files.

# Installation methods

## Binary

Download any of the binaries appropriate for your system in the releases section.

## Repository

Use the repository directly

## Crates

Install directly with

```console
~: cargo install csvconv
```

# Usage

Usage information can be seen by running the --help option:

```console
~$ csvconv -h

Usage: csvconv.exe [OPTIONS] <ORIGINAL_DELIMITER> <NEW_DELIMITER> <PATH>

Arguments:
  <ORIGINAL_DELIMITER>  Original string delimiter. Must be one character
  <NEW_DELIMITER>       New string delimiter. Must be one character
  <PATH>                File or directory path. Directory search is recursive

Options:
  -o, --output <OUTPUT>  Name of the output file (resp. directory) for the formatted result (resp. results) [default: formatted]
  -c, --check            Checks if the file is valid csv by counting delimiters in the lines
  -h, --help             Print help
```
