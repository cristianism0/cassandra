use crate::display::table_cli::{build_journal_table, build_table};
use crate::parsers::selector::{journal_parsed, parser_selector};
use std::process::exit;

use crate::display::theme::rose_pine_moon;
use crate::models::{
    AuthRecord, Finfo, FromLogEntry, JOURNAL_CRITICAL, JOURNAL_LOW, JOURNAL_MEDIUM, JournalRecord,
    JournalScope, LogSource, ParseError, SOURCES, SourceCandidate, SysRecord, TableDisplay,
    TableMode, WtmpRecord,
};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(version, about, styles=rose_pine_moon())]
struct ArgsC {
    #[arg(
        short,
        long,
        group = "display",
        value_delimiter = ',',
        global = true,
        help = "Show only given columns (comma-separated) — e.g. --summary message,host,process"
    )]
    summary: Option<Vec<String>>,
    #[arg(
        short,
        long,
        group = "display",
        global = true,
        help = "Compact table: truncate each column to N chars"
    )]
    compact: Option<usize>,
    #[arg(
        short,
        long,
        group = "display",
        global = true,
        help = "Key-value view: render one entry per table"
    )]
    key: Option<String>,
    #[arg(
        long,
        group = "display",
        global = true,
        help = "Standard full table (default)"
    )]
    standard: bool,

    // TODO: still to wire -> rows, gravity, search
    #[arg(
        long,
        value_delimiter = ',',
        global = true,
        help = "Filter rows by column=value — e.g. --rows host=myhost,process=sshd (not yet wired)"
    )]
    rows: Option<Vec<String>>,
    #[arg(short, long, global = true, help = "Limit to last N lines")]
    lines: Option<u64>,
    #[arg(
        short,
        long,
        global = true,
        help = "Reverse output order — newest first"
    )]
    reverse: Option<bool>,
    #[arg(long, global = true, help = "Substring filter (not yet wired)")]
    search: Option<String>,
    // TODO: chrono like system for all parsers — common filter e.g. "15 days ago", "2024-01-01", "now-2h"
    // Parse with humantime/chrono -> SystemTime, then filter sys/auth by timestamp
    // and journal via seek_realtime_usec. Lazy to TUI: tokio stream + mpsc, render windowed.
    // #[arg(long, global=true, help = "Show entries since time — e.g. --since '15 days ago'")]
    // since: Option<String>,
    // #[arg(long, global=true, help = "Show entries until time")]
    // until: Option<String>,
    #[command(subcommand)]
    log: LogKey,
}

#[derive(Subcommand, Debug, PartialEq, Eq, Hash, Clone)]
enum LogKey {
    #[command(about = "System log — /var/log/messages or /var/log/syslog")]
    Sys,
    #[command(about = "Auth log — /var/log/secure or /var/log/auth.log")]
    Auth,
    #[command(about = "Wtmp — /var/log/wtmp (utmp)")]
    Wtmp,
    #[command(about = "Systemd journal — via sd-journal")]
    Journal {
        #[arg(long, default_value = "user", help = "Journal scope — system or user")]
        scope: JournalScope,
        #[arg(
            short,
            long,
            global = true,
            help = "Filter by gravity — critical/low/medium (maps to priority)"
        )]
        gravity: Option<GravityArgs>,
    },
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, ValueEnum)]
enum GravityArgs {
    Critical,
    Low,
    Medium,
}

impl ArgsC {
    pub fn table_mode(&self) -> Option<TableMode> {
        if let Some(columns) = &self.summary {
            Some(TableMode::Summary {
                columns: columns.clone(),
            })
        } else if let Some(width) = self.compact {
            Some(TableMode::Compact {
                max_col_width: width,
            })
        } else if self.key.is_some() {
            Some(TableMode::KeyValue)
        } else {
            Some(TableMode::Standard)
        }
    }
}

fn table_cli_args() -> ArgsC {
    ArgsC::parse()
}

pub fn run_cli() {
    let args = table_cli_args();
    let l: Option<u64> = match args.lines {
        Some(0) => {
            eprintln!("There's no line to parse, try to use a number greater than 0!");
            exit(2);
        }
        Some(n) => Some(n),
        None => None,
    };

    let tmode = args.table_mode().unwrap_or(TableMode::Standard);

    let revs = args.reverse.unwrap_or(false);

    // TODO: still to wire -> rows, gravity, search

    match args.log {
        LogKey::Sys => {
            print_table::<SysRecord>(&tmode, LogSource::Sys, l, revs);
        }
        LogKey::Auth => {
            print_table::<AuthRecord>(&tmode, LogSource::Auth, l, revs);
        }
        LogKey::Wtmp => {
            print_table::<WtmpRecord>(&tmode, LogSource::Wtmp, l, revs);
        }
        LogKey::Journal { scope, .. } => {
            let j = match journal_parsed(scope, l, revs) {
                Ok(le) => le,
                Err(e) => {
                    eprintln!(
                        "Error: Cannot retrieve information from the journal.\nDetails: {e:#?}"
                    );
                    exit(2);
                }
            };
            let table = build_journal_table::<JournalRecord>(&j, &scope, &tmode);
            println!(
                "{}",
                table.unwrap_or_else(|| {
                    eprintln!("Error: Lunete could not create the table.");
                    exit(2);
                })
            );
        }
    }
}

fn print_table<T>(mode: &TableMode, source: LogSource, lines: Option<u64>, reverse: bool)
where
    T: TableDisplay + FromLogEntry,
{
    let ps = possible_paths(source);
    let vf = match filtered_finfo(ps) {
        Some(e) => e,
        None => {
            eprintln!(
                "Error: No available path.\nHint: Lunete may lack the required permissions.\
		       Try running 'cap.sh' to set binary capabilities."
            );
            exit(2);
        }
    };

    // TODO: still to wire in the parser -> rows, gravity enum
    let ret = match parser_selector(vf, lines, reverse) {
        Ok(e) => e,
        Err(e) => match e {
            ParseError::IoError(e) => {
                eprintln!(
                    "Error: An I/O error occurred while reading log files.\nHint: Ensure the\
			   binary has sufficient capabilities using 'cap.sh'.\nDetails: {e}"
                );
                exit(2);
            }
            ParseError::MalformedLine(e) => {
                eprintln!("Error: Found a malformed line in the log file.\nDetails: {e}");
                exit(2);
            }
            ParseError::UnexpectedFormat(e) => {
                eprintln!(
                    "Error: Failed to parse log file. The format does not match the expected \
			   structure.\nDetails: {e}"
                );
                exit(2);
            }
        },
    };
    let table = match build_table::<T>(&ret, mode) {
        Some(e) => e,
        None => {
            eprintln!("Error: Lunete could not create the table.");
            exit(2);
        }
    };
    println!("{}", table);
}

fn possible_paths(lsource: LogSource) -> Vec<&'static SourceCandidate> {
    SOURCES.iter().filter(|sc| sc.source == lsource).collect()
}

fn filtered_finfo(psc: Vec<&SourceCandidate>) -> Option<Finfo> {
    for p in psc {
        let f = Finfo::gather_info(p);
        if f.path.exists() {
            return Some(f);
        }
    }
    None
}
