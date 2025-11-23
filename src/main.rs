use std::{
    fs::{OpenOptions, read_to_string},
    io::Write,
    path::PathBuf,
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
    /// Path to chat file to filter
    #[arg(short, long, value_name = "FILE")]
    path: PathBuf,

    /// Path to the output file. Defaults to "./filtered_{INPUT FILE NAME}". Missing directories in the path will be created recursively.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Allow overwrite of the output file
    #[arg(long)]
    overwrite: bool,

    /// Match case
    #[arg(long)]
    match_case: bool,

    /// Treat include & exclude input as regexes
    #[arg(long)]
    regex: bool,

    /// Patterns that has to be included in the output
    #[arg(short, long)]
    include: Option<String>,

    /// Patterns that has to be excluded from the output
    #[arg(short, long)]
    exclude: Option<String>,

    /// Path to config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
}

fn main() {
    let start = Instant::now();

    let cli = Cli::parse();

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
                exit(1);
            });
    }

    let chat_log = read_to_string(&cli.path).unwrap_or_else(|err| {
        eprintln!(
            "Failed to read the input file from {}: {}",
            cli.path.to_string_lossy(),
            err
        );
        exit(1);
    });

    let filtered_chat_log = filter_chat_log(chat_log, config).unwrap_or_else(|err| {
        eprintln!(
            "Failed to filter the chat log from {}: {}",
            cli.path.to_string_lossy(),
            err
        );
        exit(1);
    });

    let output_path = if let Some(output_path) = cli.output {
        output_path
    } else {
        PathBuf::from(format!(
            "./filtered_{}",
            // it is unlikely that we can get error there after successfully reading this file
            cli.path.file_name().unwrap().to_string_lossy()
        ))
    };

    let mut output_file = OpenOptions::new()
        .write(true)
        .create_new(!cli.overwrite)
        .truncate(cli.overwrite)
        .open(&output_path)
        .unwrap_or_else(|err| {
            eprintln!(
                "Failed to create the output file in {}: {}",
                cli.path.to_string_lossy(),
                err
            );
            exit(1);
        });

    output_file
        .write_all(filtered_chat_log.as_bytes())
        .unwrap_or_else(|err| {
            eprintln!(
                "Failed to write to the output file in {}: {}",
                cli.path.to_string_lossy(),
                err
            );
            exit(1);
        });

    println!(
        "Filtered chat log from {} to {} in {}ms",
        cli.path.to_string_lossy(),
        output_path.to_string_lossy(),
        start.elapsed().as_millis()
    );
}

fn filter_chat_log(chat_log: String, config: Config) -> Result<String, anyhow::Error> {
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
