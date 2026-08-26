use std::collections::HashMap;
use std::process::exit;
use tabled::Tabled;
use crate::display::table_cli::{build_table, build_journal_table};
use crate::parsers::selector::{journal_parsed, parser_selector};

use crate::models::{AuthRecord, Finfo, FromLogEntry, JournalRecord, JournalScope, LogSource, SOURCES, SourceCandidate, SysRecord, TableKey, TableMode, WtmpRecord};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about)]
struct ArgsC {
    #[arg(long, group = "display", value_delimiter = ',', global = true)]
    summary: Option<Vec<String>>,
    #[arg(long, group = "display", global = true)]
    compact: Option<usize>,
    #[arg(long, group = "display", global = true)]
    standard: bool, //help display only (default behavior)
    #[arg(short, long, group = "display", global = true)]
    key: Option<String>,
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
    },
}

impl ArgsC {
    pub fn table_mode(&self) -> Option<TableMode> {
        if self.summary.is_some() {
            let k = get_key(&self.log);
            let mut hash = HashMap::new();
            hash.insert(k, self.summary.clone()?);
            Some(TableMode::Summary { columns: hash })
        } else if self.compact.is_some() {
            Some(TableMode::Compact {
                max_col_width: self.compact.unwrap(),
            })
        } else if self.key.is_some() {
            Some(TableMode::KeyValue)
        } else {
            Some(TableMode::Standard)
        }
    }
}

fn get_key(l: &LogKey) -> TableKey {
    match l {
        LogKey::Sys => TableKey::Sys,
        LogKey::Auth => TableKey::Auth,
        LogKey::Wtmp => TableKey::Wtmp,
        LogKey::Journal { scope: _ } => TableKey::Journal,
    }
}

fn table_cli_args() -> ArgsC {
    ArgsC::parse()
}

pub fn run_cli() {
    let args = table_cli_args();
    let tmode = args.table_mode().unwrap_or(TableMode::Standard);

    match args.log {
	LogKey::Sys => {
	    print_table::<SysRecord>(&tmode, TableKey::Sys, LogSource::Sys);
	}
	LogKey::Auth => {
	    print_table::<AuthRecord>(&tmode, TableKey::Auth, LogSource::Auth);
	}
	LogKey::Wtmp => {
	    print_table::<WtmpRecord>(&tmode, TableKey::Wtmp, LogSource::Wtmp);
	}
	LogKey::Journal { scope } => {
	    let j = journal_parsed(scope).unwrap_or_else(|e| {
		eprintln!("The journal was not parsed due to error: {e:#?}");
		eprintln!("Lunete was terminated.");
		exit(2);
	    });
	    let table = build_journal_table::<JournalRecord>(&j, &scope, &tmode);
	    println!("{}", table.unwrap_or_else(|| {
		// TODO: better error handling
		eprintln!("Lunete could not display the result.");
		exit(2);
	    }));
	}
    }
}

fn print_table<T>(mode: &TableMode, key: TableKey, source: LogSource)
where T: Tabled + FromLogEntry
{
    // TODO: handle unwraps
    let ps = possible_paths(source);
    let vf = filtered_finfo(ps).unwrap();
    let ret = parser_selector(vf).unwrap();
    let table = build_table::<T>(&ret, mode, key);
    println!("{}", table.unwrap());
}

fn possible_paths(lsource: LogSource) -> Vec<&'static SourceCandidate>{
    SOURCES.iter().filter(|sc| sc.source == lsource).collect()
}

fn filtered_finfo(psc: Vec<&SourceCandidate>) -> Option<Finfo> {
    for p in psc {
	let f = Finfo::gather_info(p);
	if f.path.exists() {
	   return Some(f)
	}
    }
    None
}
