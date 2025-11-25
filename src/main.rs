use std::{
    fs::{OpenOptions, read_to_string},
    io::{Read, Write, stdin},
    path::{Path, PathBuf},
    process::exit,
    time::Instant,
};

use clap::Parser;

use crate::config::Config;

mod config;

/// Simple CLI utility to filter the Space Station 13 saved chat logs
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Paths to chat log files to filter
    #[arg(short, long, value_name = "FILES")]
    paths: Vec<PathBuf>,

    /// Paths to the output files. Defaults to "{out_dir}/filtered_{INPUT FILE NAME}". out_dir defaults to current working
    /// directory the program's working directory. Missing directories in the path will be created recursively. If more
    /// paths than outputs were provided, missing outputs will be set to default. If more outputs than paths
    /// were provided, excessive outputs will be ignored.
    #[arg(short, long, value_name = "FILES")]
    outputs: Vec<PathBuf>,

    /// Path to the directory, which will be considered base for default outputs. Missing directories in the path will be
    /// created recursively.
    #[arg(long, value_name = "DIR")]
    out_dir: Option<PathBuf>,

    /// Read paths from standard input, separated by whitespaces
    #[arg(long)]
    stdin: bool,

    /// Exits the program if failed to filter one or more paths
    #[arg(long)]
    strict: bool,

    /// Allow overwrite of the output file
    #[arg(long)]
    overwrite: bool,

    /// Match case
    #[arg(long)]
    match_case: bool,

    /// Treat include & exclude patterns as regexes
    #[arg(long)]
    regex: bool,

    /// Patterns that has to be included in the output
    #[arg(short, long)]
    include: Option<String>,

    /// Patterns that has to be excluded from the output
    #[arg(short, long)]
    exclude: Option<String>,

    /// Path to a config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
}

fn main() {
    let start = Instant::now();

    let mut cli = Cli::parse();

    let config: Config;

    if let Some(config_path) = cli.config {
        config = Config::load(&config_path).unwrap_or_else(|err| {
            eprintln!(
                "Failed to load config from {}: {}",
                config_path.to_string_lossy(),
                err
            );
            exit(1);
        });
    } else {
        config = Config::from_args(cli.regex, cli.include, cli.exclude, cli.match_case)
            .unwrap_or_else(|err| {
                eprintln!("Failed to parse arguments: {}", err);
                exit(1)
            });
    }

    if cli.stdin {
        let mut buf: Vec<u8> = Vec::new();
        stdin().read_to_end(&mut buf).unwrap_or_else(|err| {
            eprintln!("Failed to read from the standard input: {}", err);
            exit(1);
        });
        let mut stdin_paths: Vec<PathBuf> = String::from_utf8_lossy(&buf)
            .split_whitespace()
            .map(|path| path.into())
            .collect();
        println!(
            "Parsed {} paths from the standard input.",
            stdin_paths.len()
        );
        cli.paths.append(&mut stdin_paths);
    }

    if cli.paths.is_empty() {
        eprintln!("No valid paths were provided");
        exit(1)
    }

    for (index, log_path) in cli.paths.iter().enumerate() {
        let this_path_start = Instant::now();
        let output_path = get_path_for_output(index, &cli.outputs, log_path, &cli.out_dir);

        match process_path(log_path, &output_path, &config, cli.overwrite) {
            Ok(()) => {
                println!(
                    "Filtered chat log from {} to {} in {}ms",
                    log_path.to_string_lossy(),
                    output_path.to_string_lossy(),
                    this_path_start.elapsed().as_millis()
                );
            }
            Err(err) => {
                eprintln!("Failed to process {}: {}", log_path.to_string_lossy(), err);
                if cli.strict {
                    eprintln!("Encountered error in strict mode. Exiting...");
                    exit(1)
                } else {
                    continue;
                }
            }
        }
    }

    println!(
        "Filtered {} logs in {}ms",
        cli.paths.len(),
        start.elapsed().as_millis()
    );
}

fn get_path_for_output(
    index: usize,
    outputs: &[PathBuf],
    path: &Path,
    base_dir: &Option<PathBuf>,
) -> PathBuf {
    if let Some(output) = outputs.get(index) {
        return output.clone();
    }
    let base_dir = match &base_dir {
        Some(dir) => dir.to_string_lossy().trim_end_matches("/").to_string(),
        None => ".".to_string(),
    };
    let file_name = path
        .file_name()
        .map(|file_name| file_name.to_string_lossy())
        .unwrap_or(format!("file_name_error{}", index).into());

    PathBuf::from(format!("{}/filtered_{}", base_dir, file_name))
}

fn process_path(
    path: &PathBuf,
    output_path: &PathBuf,
    config: &Config,
    overwrite: bool,
) -> Result<(), anyhow::Error> {
    let chat_log = read_to_string(path)
        .map_err(|err| anyhow::format_err!("error while reading the input file: {}", err))?;

    let filtered_chat_log = filter_chat_log(chat_log, config).unwrap_or_else(|err| {
        eprintln!("filter error: {}", err);
        exit(1);
    });

    let mut output_file = OpenOptions::new()
        .write(true)
        .create_new(!overwrite)
        .create(overwrite)
        .truncate(overwrite)
        .open(output_path)
        .unwrap_or_else(|err| {
            eprintln!(
                "error while creating the output file in {}: {}",
                path.to_string_lossy(),
                err
            );
            exit(1);
        });

    output_file
        .write_all(filtered_chat_log.as_bytes())
        .unwrap_or_else(|err| {
            eprintln!(
                "error while writing to the output file in {}: {}",
                output_path.to_string_lossy(),
                err
            );
            exit(1);
        });

    Ok(())
}

fn filter_chat_log(chat_log: String, config: &Config) -> Result<String, anyhow::Error> {
    let mut output = String::with_capacity(chat_log.len());
    let parts: Vec<&str> = chat_log.split_inclusive("<div class=\"Chat\">").collect();
    if parts.len() != 2 {
        Err(anyhow::format_err!(
            "Expected 1 \"<div class=\"Chat\">\", but found {}",
            parts.len() - 1
        ))?
    }
    output.push_str(parts[0]);

    let chat_messages = parts[1].replace("</div>\n</body>\n</html>", "");

    for message in chat_messages.split_inclusive("<div class=\"ChatMessage\"") {
        if config.matches(message)? {
            output.push_str(message);
        }
    }

    output.push_str("</div>\n</body>\n</html>");

    Ok(output)
}
