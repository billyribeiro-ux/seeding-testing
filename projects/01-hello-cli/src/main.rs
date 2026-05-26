//! hello-cli — count lines, words, and characters in a file or stdin.
//!
//! Exit codes:
//!   0 — success
//!   1 — generic failure (I/O, encoding…)
//!   2 — bad input (missing file, invalid args)

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use hello_cli::{Counts, Selection};

/// Count lines, words, and characters.
///
/// If no flags are passed, all three are printed (like `wc`).
/// If FILE is omitted, input is read from standard input.
#[derive(Debug, Parser)]
#[command(name = "hello-cli", version, about)]
struct Cli {
    /// File to read. If omitted, read from stdin.
    file: Option<PathBuf>,

    /// Show the line count.
    #[arg(short = 'l', long)]
    lines: bool,

    /// Show the word count.
    #[arg(short = 'w', long)]
    words: bool,

    /// Show the character count (Unicode scalars).
    #[arg(short = 'c', long)]
    chars: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let input = match read_input(cli.file.as_deref()) {
        Ok(s) => s,
        Err(ReadError::NotFound(p)) => {
            eprintln!("hello-cli: {}: no such file", p.display());
            return ExitCode::from(2);
        }
        Err(ReadError::Io(e)) => {
            eprintln!("hello-cli: I/O error: {e}");
            return ExitCode::from(1);
        }
        Err(ReadError::NotUtf8) => {
            eprintln!("hello-cli: input is not valid UTF-8");
            return ExitCode::from(1);
        }
    };

    let counts = Counts::count(&input);
    let sel = Selection {
        lines: cli.lines,
        words: cli.words,
        chars: cli.chars,
    }
    .or_default();

    println!("{}", sel.render(counts));
    ExitCode::SUCCESS
}

#[derive(Debug)]
enum ReadError {
    NotFound(PathBuf),
    Io(io::Error),
    NotUtf8,
}

fn read_input(path: Option<&std::path::Path>) -> Result<String, ReadError> {
    let Some(p) = path else {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(ReadError::Io)?;
        return Ok(buf);
    };

    match fs::read(p) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| ReadError::NotUtf8),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(ReadError::NotFound(p.to_path_buf())),
        Err(e) => Err(ReadError::Io(e)),
    }
}
