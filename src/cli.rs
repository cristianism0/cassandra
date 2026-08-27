use std::process::exit;
use crate::display::table_cli::{build_table, build_journal_table};
use crate::parsers::selector::{journal_parsed, parser_selector};

use crate::models::{AuthRecord, Finfo, FromLogEntry, JournalRecord, JournalScope, LogSource, SOURCES, SourceCandidate, SysRecord, TableMode, WtmpRecord, ParseError, TableDisplay, JOURNAL_CRITICAL, JOURNAL_MEDIUM, JOURNAL_LOW};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(version, about)]
struct ArgsC {
    #[arg(short,long, group = "display", value_delimiter = ',', global = true)]
    summary: Option<Vec<String>>,
    #[arg(short,long, group = "display", global = true)]
    compact: Option<usize>,
    #[arg(short, long, group = "display", global = true)]
    key: Option<String>,
    #[arg(long, group = "display", global = true)]
    standard: bool,

    // TODO: all this below -> lines, reverse, rows, gravity, search
    #[arg(long, value_delimiter=',', global=true)]
    rows: Option<Vec<String>>,
    #[arg(short, long, global=true)]
    lines: Option<u64>,
    #[arg(short, long, global=true)]
    reverse: Option<bool>,
    #[arg(long, global=true)] // like grep
    search: Option<String>,

    #[command(subcommand)]
    log: LogKey,
}

#[derive(Subcommand, Debug, PartialEq, Eq, Hash, Clone)]
enum LogKey {
    Sys,
    Auth,
    Wtmp,
    Journal {
        #[arg(long, default_value = "user")]
        scope: JournalScope,
	#[arg(short, long, global=true)]
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
            Some(TableMode::Summary { columns: columns.clone() })
        } else if let Some(width) = self.compact {
            Some(TableMode::Compact { max_col_width: width })
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
    let tmode = args.table_mode().unwrap_or(TableMode::Standard);

    let revs = args.reverse.unwrap_or(false);

    // TODO: all this below -> lines, reverse, rows, gravity, search

    match args.log {
        LogKey::Sys => {
            print_table::<SysRecord>(&tmode, LogSource::Sys, args.lines, revs);
        }
        LogKey::Auth => {
            print_table::<AuthRecord>(&tmode, LogSource::Auth, args.lines, revs);
        }
        LogKey::Wtmp => {
            print_table::<WtmpRecord>(&tmode, LogSource::Wtmp, args.lines, revs);
        }
        LogKey::Journal { scope, .. } => {
            let j = match journal_parsed(scope) {
                Ok(le) => le,
		Err(e) =>{
		eprintln!("Error: Cannot retrieve information from the journal.\nDetails: {e:#?}");
                exit(2);
		}
            };
            let table = build_journal_table::<JournalRecord>(&j, &scope, &tmode);
            println!("{}", table.unwrap_or_else(|| {
                eprintln!("Error: Lunete could not create the table.");
                exit(2);
            }));
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
	    eprintln!("Error: No available path.\nHint: Lunete may lack the required permissions.\
		       Try running 'cap.sh' to set binary capabilities.");
	    exit(2);
	}
    };

    // TODO: in the parser will enter the following args: lines, reverse, rows, gravity enum
    let ret = match parser_selector(vf, lines, reverse) {
	Ok(e) => e,
	Err(e) => match e {
	    ParseError::IoError(e) => {
		eprintln!("Error: An I/O error occurred while reading log files.\nHint: Ensure the\
			   binary has sufficient capabilities using 'cap.sh'.\nDetails: {e}");
		exit(2);
	    },
	    ParseError::MalformedLine(e) => {
		eprintln!("Error: Found a malformed line in the log file.\nDetails: {e}");
		exit(2);
	    },
	    ParseError::UnexpectedFormat(e) => {
		eprintln!("Error: Failed to parse log file. The format does not match the expected \
			   structure.\nDetails: {e}");
		exit(2);
	    },
	}
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
    SOURCES
        .iter()
        .filter(|sc| sc.source == lsource)
        .collect()
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
