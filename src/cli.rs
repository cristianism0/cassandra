use chrono::{DateTime, TimeZone, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use regex::Regex;
use std::collections::VecDeque;
use std::io::{self, Write};
use std::process::exit;

use crate::display::{
    TableDisplay, TableMode,
    pager::pager_or_print,
    table_cli::{build_journal_table, build_table},
    theme::rose_pine_moon,
};

use crate::parsers::{
    ParseError,
    selector::{journal_parsed, parser_selector},
};

use crate::models::{
    Finfo, FromLogEntry, LogSource, SOURCES, SourceCandidate,
    auth::AuthRecord,
    journal::{JOURNAL_KERNEL_COL, JournalRecord, JournalScope},
    sys::SysRecord,
    wtmp::WtmpRecord,
};

use crate::utils::time::{datetime_to_micros, parse_human_time, rfc3164_to_datetime};

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
    #[arg(
        long,
        short = 'R',
        group = "display",
        global = true,
        help = "Raw output: tab-separated, no wrapping (streaming, grep-friendly; no pager)"
    )]
    raw: bool,
    #[arg(long, global = true, help = "Disable pager (print directly, no less)")]
    no_pager: bool,

    #[arg(
        long,
        short = 'F',
        value_delimiter = ',',
        global = true,
        help = "Filter rows by column=value — e.g. --rows host=myhost,process=sshd (-F)"
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
    reverse: bool,
    #[arg(
        long,
        short = 'e',
        global = true,
        help = "Regex search across all fields — e.g. --search 'error|failed' (-e)"
    )]
    search: Option<String>,
    #[arg(
        long,
        short = 'S',
        global = true,
        help = "Show entries since time — e.g. --since '2024-01-01', '15 days ago', '2h ago', 'now-2h', 'today', 'yesterday' (UTC) (-S)"
    )]
    since: Option<String>,
    #[arg(
        long,
        short = 'U',
        global = true,
        help = "Show entries until time — e.g. --until '2024-01-01' (UTC) (-U)"
    )]
    until: Option<String>,
    #[command(subcommand)]
    log: LogKey,
}

#[derive(Subcommand, Debug, PartialEq, Eq, Hash, Clone)]
enum LogKey {
    #[command(about = "System log — /var/log/messages or /var/log/syslog")]
    Sys {
        #[arg(long, help = "List available columns for this source and exit")]
        list_columns: bool,
    },
    #[command(about = "Auth log — /var/log/secure or /var/log/auth.log")]
    Auth {
        #[arg(long, help = "List available columns for this source and exit")]
        list_columns: bool,
    },
    #[command(about = "Wtmp — /var/log/wtmp (utmp)")]
    Wtmp {
        #[arg(long, help = "List available columns for this source and exit")]
        list_columns: bool,
    },
    #[command(about = "Systemd journal — via sd-journal")]
    Journal {
        #[arg(long, default_value = "user", help = "Journal scope — system or user")]
        scope: JournalScope,
        #[arg(
            long,
            short = 'g',
            value_enum,
            help = "Filter by gravity — critical, medium, low (maps to priority) (-g)"
        )]
        gravity: Option<GravityArgs>,
        #[arg(long, help = "List available columns for this source and exit")]
        list_columns: bool,
    },
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, ValueEnum)]
enum GravityArgs {
    Critical,
    Low,
    Medium,
}

impl ArgsC {
    pub fn table_mode(&self) -> TableMode {
        if self.raw {
            TableMode::Raw
        } else if let Some(columns) = &self.summary {
            TableMode::Summary {
                columns: columns.clone(),
            }
        } else if let Some(width) = self.compact {
            TableMode::Compact {
                max_col_width: width,
            }
        } else if self.key.is_some() {
            TableMode::KeyValue
        } else {
            TableMode::Standard
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
            eprintln!("Error: --lines 0 is invalid — no lines to display.");
            eprintln!(
                "Hint: Use a value greater than 0, e.g. -l 10 or omit --lines for the default (50 for journal, all for files)."
            );
            eprintln!("Details: --lines expects N > 0");
            exit(2);
        }
        Some(n) => Some(n),
        None => None,
    };

    let tmode = args.table_mode();
    let revs = args.reverse;
    let no_pager = args.no_pager;

    let search_re = match &args.search {
        Some(pat) => match Regex::new(pat) {
            Ok(re) => Some(re),
            Err(e) => {
                eprintln!("Error: Invalid search regex (--search) '{pat}': {e}");
                eprintln!("Hint: Use a valid Rust regex, e.g. --search 'error|failed' or --search 'Failed.*password' (-e).
                    Escape special chars or use --rows for exact match.");
                eprintln!("Details: regex parse error: {e}");
                exit(2);
            }
        },
        None => None,
    };

    let row_filters: Option<Vec<(String, String)>> = match &args.rows {
        Some(raw) => match parse_row_filters(raw) {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("Error: Invalid --rows filter: {e}");
                eprintln!("Hint: Use --rows col=value[,col2=value2] — e.g. --rows host=myhost,process=sshd (-F).
                    Quote values with spaces.");
                eprintln!("Details: expected col=value, got '{raw:?}'");
                exit(2);
            }
        },
        None => None,
    };

    let since_dt: Option<DateTime<Utc>> = match &args.since {
        Some(s) => match parse_human_time(s) {
            Ok(dt) => Some(dt),
            Err(e) => {
                eprintln!("Error: Invalid --since '{s}': {e}");
                eprintln!(
                    "Hint: Try --since '2024-01-01', '2024-01-01T10:00:00', '15 days ago',
                    '2h ago', 'now-2h', 'today', 'yesterday' (UTC). Short: -S"
                );
                eprintln!("Details: {e}");
                exit(2);
            }
        },
        None => None,
    };
    let until_dt: Option<DateTime<Utc>> = match &args.until {
        Some(s) => match parse_human_time(s) {
            Ok(dt) => Some(dt),
            Err(e) => {
                eprintln!("Error: Invalid --until '{s}': {e}");
                eprintln!("Hint: Try --until '2024-01-01' or 'today' (UTC). Short: -U");
                eprintln!("Details: {e}");
                exit(2);
            }
        },
        None => None,
    };
    if let (Some(since), Some(until)) = (&since_dt, &until_dt)
        && since > until
    {
        eprintln!("Error: --since time is after --until time");
        eprintln!(
            "Hint: Swap --since and --until or use --since
            '2024-01-01' --until '2024-12-31' with since < until."
        );
        eprintln!("Details: since={since} (UTC) > until={until} (UTC)");
        exit(2);
    }

    let is_wtmp = matches!(args.log, LogKey::Wtmp { .. });
    if is_wtmp && (since_dt.is_some() || until_dt.is_some()) {}

    let is_raw = matches!(tmode, TableMode::Raw);
    match args.log {
        LogKey::Sys { list_columns } => {
            if list_columns {
                print_list_columns::<SysRecord>();
            }
            if is_raw {
                print_raw_file::<SysRecord>(
                    LogSource::Sys,
                    l,
                    revs,
                    search_re.as_ref(),
                    row_filters.as_deref(),
                    since_dt.as_ref(),
                    until_dt.as_ref(),
                );
            } else {
                let lines_for_parser = if since_dt.is_some() || until_dt.is_some() {
                    None
                } else {
                    l
                };
                print_table::<SysRecord>(
                    &tmode,
                    LogSource::Sys,
                    lines_for_parser,
                    l,
                    revs,
                    search_re.as_ref(),
                    row_filters.as_deref(),
                    since_dt.as_ref(),
                    until_dt.as_ref(),
                    no_pager,
                );
            }
        }
        LogKey::Auth { list_columns } => {
            if list_columns {
                print_list_columns::<AuthRecord>();
            }
            if is_raw {
                print_raw_file::<AuthRecord>(
                    LogSource::Auth,
                    l,
                    revs,
                    search_re.as_ref(),
                    row_filters.as_deref(),
                    since_dt.as_ref(),
                    until_dt.as_ref(),
                );
            } else {
                let lines_for_parser = if since_dt.is_some() || until_dt.is_some() {
                    None
                } else {
                    l
                };
                print_table::<AuthRecord>(
                    &tmode,
                    LogSource::Auth,
                    lines_for_parser,
                    l,
                    revs,
                    search_re.as_ref(),
                    row_filters.as_deref(),
                    since_dt.as_ref(),
                    until_dt.as_ref(),
                    no_pager,
                );
            }
        }
        LogKey::Wtmp { list_columns } => {
            if list_columns {
                print_list_columns::<WtmpRecord>();
            }
            if is_raw {
                print_raw_file::<WtmpRecord>(
                    LogSource::Wtmp,
                    l,
                    revs,
                    search_re.as_ref(),
                    row_filters.as_deref(),
                    None,
                    None,
                );
            } else {
                print_table::<WtmpRecord>(
                    &tmode,
                    LogSource::Wtmp,
                    l,
                    l,
                    revs,
                    search_re.as_ref(),
                    row_filters.as_deref(),
                    None,
                    None,
                    no_pager,
                );
            }
        }
        LogKey::Journal {
            scope,
            gravity,
            list_columns,
        } => {
            if list_columns {
                match scope {
                    JournalScope::System => {
                        let headers = JournalRecord::headers();
                        for h in headers {
                            if !JOURNAL_KERNEL_COL.iter().any(|c| c.eq_ignore_ascii_case(h)) {
                                println!("{h}");
                            }
                        }
                        std::process::exit(0);
                    }
                    JournalScope::User => {
                        print_list_columns::<JournalRecord>();
                    }
                }
            }
            let since_usec = since_dt.as_ref().map(|dt| datetime_to_micros(*dt));
            let until_usec = until_dt.as_ref().map(|dt| datetime_to_micros(*dt));
            if is_raw {
                print_raw_journal(
                    scope,
                    gravity,
                    since_usec,
                    until_usec,
                    l,
                    revs,
                    search_re.as_ref(),
                    row_filters.as_deref(),
                );
            } else {
                let gravity_dbg = gravity.clone();
                let mut j = match journal_parsed(scope, l, revs, since_usec, until_usec) {
                    Ok(le) => le,
                    Err(e) => {
                        eprintln!(
                            "Error: Cannot retrieve entries from journal (scope={scope:?}) — journal unavailable or no permission."
                        );
                        eprintln!(
                            "Hint: Check that systemd-journald is running, 
                            try --scope user vs --scope system, and ensure read access — try 'sudo ./cap.sh' or 'journalctl --verify'."
                        );
                        eprintln!("Details: {e:#?}");
                        exit(2);
                    }
                };

                if let Some(g) = &gravity {
                    j = apply_gravity_filter(j, g);
                }
                if let Some(filters) = row_filters.as_deref() {
                    let validated = validate_row_columns::<JournalRecord>(filters);
                    if let Err(e) = validated {
                        eprintln!("Error: Invalid --rows filter for journal: {e}");
                        eprintln!(
                            "Hint: Use -F col=value — e.g. -F systemd_unit=sshd.service. Check --list-columns for journal (scope={scope:?})."
                        );
                        eprintln!("Details: {e}");
                        exit(2);
                    }
                    j = apply_rows_filter::<JournalRecord>(j, filters);
                }
                if let Some(re) = search_re.as_ref() {
                    j = apply_search_filter::<JournalRecord>(j, re);
                }

                let table = build_journal_table::<JournalRecord>(&j, &scope, &tmode);
                let output = table.unwrap_or_else(|| {
                    eprintln!("Error: No entries to display for journal (scope={scope:?}) — table empty after filtering (0 rows).");
                    eprintln!("Hint: Try relaxing filters: remove -e/--search, -F/--rows, widen -S/--since/-U/--until, increase -l, or use --raw.
                        Check --list-columns and try without gravity.");
                    eprintln!("Details: 
                        scope={scope:?}, gravity={gravity_dbg:?}, 
                        search={}, rows={}, since={}, until={}, 
                        lines={:?}, reverse={}; result 0 
                        rows (journal empty or all filtered)", 
                        search_re.is_some(),
                        row_filters.is_some(),
                        since_usec.is_some(),
                        until_usec.is_some(),
                        l, revs);
                    exit(2);
                });
                let output = format!("{output}\n");
                pager_or_print(&output, no_pager);
            }
        }
    }
}

fn print_list_columns<T: TableDisplay>() -> ! {
    for col in T::headers() {
        println!("{col}");
    }
    exit(0);
}

fn parse_row_filters(raw: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::with_capacity(raw.len());
    for s in raw {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err("empty filter".to_string());
        }
        let (k, v) = trimmed
            .split_once('=')
            .ok_or_else(|| format!("missing '=' in '{s}'"))?;
        let k = k.trim();
        let v = v.trim();
        if k.is_empty() || v.is_empty() {
            return Err(format!("empty column or value in '{s}'"));
        }
        out.push((k.to_string(), v.to_string()));
    }
    Ok(out)
}

fn validate_row_columns<T: TableDisplay>(filters: &[(String, String)]) -> Result<(), String> {
    let headers = T::headers();
    for (col, _) in filters {
        if !headers.iter().any(|h| h.eq_ignore_ascii_case(col)) {
            return Err(format!(
                "unknown column '{col}' — available: {}",
                headers.join(", ")
            ));
        }
    }
    Ok(())
}

fn apply_rows_filter<T: TableDisplay + FromLogEntry>(
    entries: Vec<crate::models::LogEntry>,
    filters: &[(String, String)],
) -> Vec<crate::models::LogEntry> {
    if filters.is_empty() {
        return entries;
    }
    let headers = T::headers();
    let idx_vals: Vec<(usize, &String)> = filters
        .iter()
        .filter_map(|(col, val)| {
            headers
                .iter()
                .position(|h| h.eq_ignore_ascii_case(col))
                .map(|idx| (idx, val))
        })
        .collect();

    entries
        .into_iter()
        .filter(|e| {
            if let Some(r) = T::from_entry(e) {
                let fields = r.fields();
                idx_vals.iter().all(|(idx, val)| fields[*idx] == **val)
            } else {
                false
            }
        })
        .collect()
}

fn apply_search_filter<T: TableDisplay + FromLogEntry>(
    entries: Vec<crate::models::LogEntry>,
    regex: &Regex,
) -> Vec<crate::models::LogEntry> {
    entries
        .into_iter()
        .filter(|e| {
            if let Some(r) = T::from_entry(e) {
                r.fields().iter().any(|f| regex.is_match(f))
            } else {
                false
            }
        })
        .collect()
}

fn apply_gravity_filter(
    entries: Vec<crate::models::LogEntry>,
    gravity: &GravityArgs,
) -> Vec<crate::models::LogEntry> {
    entries
        .into_iter()
        .filter(|e| matches_gravity(e, gravity))
        .collect()
}

fn matches_gravity(entry: &crate::models::LogEntry, gravity: &GravityArgs) -> bool {
    let pri_opt = match entry {
        crate::models::LogEntry::Journal(j) => j.priority.as_deref(),
        crate::models::LogEntry::Sys(s) => s.priority.as_deref(),
        crate::models::LogEntry::Auth(a) => a.priority.as_deref(),
        crate::models::LogEntry::Wtmp(_) => return false,
    };
    let pri_str = match pri_opt {
        Some(s) => s,
        None => return false,
    };
    let pri: u8 = match pri_str.parse() {
        Ok(n) => n,
        Err(_) => return false,
    };
    match gravity {
        GravityArgs::Critical => pri <= 3,
        GravityArgs::Medium => (4..=5).contains(&pri),
        GravityArgs::Low => (6..=7).contains(&pri),
    }
}

fn apply_time_filter(
    entries: Vec<crate::models::LogEntry>,
    since: Option<&DateTime<Utc>>,
    until: Option<&DateTime<Utc>>,
) -> Vec<crate::models::LogEntry> {
    apply_time_filter_with_now(entries, since, until, Utc::now())
}

fn apply_time_filter_with_now(
    entries: Vec<crate::models::LogEntry>,
    since: Option<&DateTime<Utc>>,
    until: Option<&DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Vec<crate::models::LogEntry> {
    if since.is_none() && until.is_none() {
        return entries;
    }
    entries
        .into_iter()
        .filter(|e| {
            let dt_opt = match e {
                crate::models::LogEntry::Sys(r) => rfc3164_to_datetime(&r.timestamp, now).ok(),
                crate::models::LogEntry::Auth(r) => rfc3164_to_datetime(&r.timestamp, now).ok(),
                crate::models::LogEntry::Journal(j) => {
                    if let Some(ts) = &j.source_realtime_timestamp {
                        if let Ok(micros) = ts.parse::<i64>() {
                            Some(
                                Utc.timestamp_micros(micros)
                                    .single()
                                    .expect("valid timestamp"),
                            )
                        } else {
                            None
                        }
                    } else if let Some(ts) = &j.source_boottime_timestamp {
                        if let Ok(micros) = ts.parse::<i64>() {
                            Some(
                                Utc.timestamp_micros(micros)
                                    .single()
                                    .expect("valid timestamp"),
                            )
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                crate::models::LogEntry::Wtmp(_) => return true, // ignore for wtmp
            };
            if let Some(dt) = dt_opt {
                if let Some(since_dt) = since
                    && dt < *since_dt
                {
                    return false;
                }
                if let Some(until_dt) = until
                    && dt > *until_dt
                {
                    return false;
                }
                true
            } else {
                false
            }
        })
        .collect()
}

fn apply_lines_limit(
    entries: Vec<crate::models::LogEntry>,
    lines: Option<u64>,
    reverse: bool,
) -> Vec<crate::models::LogEntry> {
    if let Some(n) = lines {
        let n_usize = n as usize;
        let len = entries.len();
        if len <= n_usize {
            if reverse {
                let mut rev = entries;
                rev.reverse();
                return rev;
            } else {
                return entries;
            }
        }
        if reverse {
            let mut filtered = entries;
            filtered.reverse();
            filtered.truncate(n_usize);
            filtered
        } else {
            entries.into_iter().skip(len - n_usize).collect()
        }
    } else if reverse {
        let mut rev = entries;
        rev.reverse();
        rev
    } else {
        entries
    }
}

fn print_table<T>(
    mode: &TableMode,
    source: LogSource,
    lines_for_parser: Option<u64>,
    lines_outer: Option<u64>,
    reverse: bool,
    search_re: Option<&Regex>,
    row_filters: Option<&[(String, String)]>,
    since: Option<&DateTime<Utc>>,
    until: Option<&DateTime<Utc>>,
    no_pager: bool,
) where
    T: TableDisplay + FromLogEntry,
{
    if let Some(filters) = row_filters
        && let Err(e) = validate_row_columns::<T>(filters)
    {
        eprintln!("Error: Invalid --rows filter: {e}");
        eprintln!(
            "Hint: Use --rows col=value[,col2=value2]
            — e.g. --rows host=myhost,process=sshd (-F).
            Check --list-columns for {source:?} available columns."
        );
        eprintln!("Details: {e}");
        exit(2);
    }

    let ps = possible_paths(source);
    let attempted: Vec<String> = ps.iter().map(|p| p.path.to_string()).collect();
    let vf = match filtered_finfo(ps) {
        Some(e) => e,
        None => {
            eprintln!("Error: No readable log file found for {source:?}.");
            eprintln!(
                "Hint: Check that log files exist ({})
                and that Cassandra has read access — try 'sudo ./cap.sh' or run with sudo.
                See 'cassandra {} --help' for expected paths.",
                attempted.join(", "),
                format!("{source:?}").to_lowercase()
            );
            eprintln!("Details: attempted paths: {}", attempted.join(", "));
            exit(2);
        }
    };

    let has_time_filter = since.is_some() || until.is_some();
    let parser_reverse = if has_time_filter { false } else { reverse };
    let mut ret = match parser_selector(&vf, lines_for_parser, parser_reverse) {
        Ok(e) => e,
        Err(e) => match e {
            ParseError::IoError(e) => {
                eprintln!(
                    "Error: I/O error reading {} at {}: {e}",
                    format!("{source:?}").to_lowercase(),
                    vf.path.display()
                );
                eprintln!(
                    "Hint: Ensure the binary has read access — try 'sudo ./cap.sh', check file permissions, or run with sudo."
                );
                eprintln!("Details: {e}");
                exit(2);
            }
            ParseError::MalformedLine(e) => {
                eprintln!(
                    "Error: Malformed line in {} at {} — not RFC 3164.",
                    format!("{source:?}").to_lowercase(),
                    vf.path.display()
                );
                eprintln!(
                    "Hint: Check the file with --raw or 'head {}' — RFC 3164 expects 
                    '<pri>Mon DD HH:MM:SS host process: msg'. Use --raw to skip malformed lines with a warning.",
                    vf.path.display()
                );
                eprintln!("Details: {e}");
                exit(2);
            }
            ParseError::UnexpectedFormat(e) => {
                eprintln!(
                    "Error: Failed to parse {} at {} — format does not match RFC 3164.",
                    format!("{source:?}").to_lowercase(),
                    vf.path.display()
                );
                eprintln!(
                    "Hint: Verify log format (RFC 3164) or try --raw to stream raw lines.
                    Check 'cassandra {} --list-columns' for expected fields.",
                    format!("{source:?}").to_lowercase()
                );
                eprintln!("Details: {e}");
                exit(2);
            }
        },
    };

    if has_time_filter {
        ret = apply_time_filter(ret, since, until);
    }

    if let Some(filters) = row_filters {
        ret = apply_rows_filter::<T>(ret, filters);
    }
    if let Some(re) = search_re {
        ret = apply_search_filter::<T>(ret, re);
    }

    if has_time_filter {
        ret = apply_lines_limit(ret, lines_outer, reverse);
    }

    let table = match build_table::<T>(&ret, mode) {
        Some(e) => e,
        None => {
            eprintln!(
                "Error: No entries to display for {} — table empty after filtering (0 rows).",
                format!("{source:?}").to_lowercase()
            );
            eprintln!(
                "Hint: Try relaxing filters: remove --search/--rows,
                widen --since/--until, increase -l, or use --raw for tab-separated output.
                Check --list-columns for {} and try without filters.",
                format!("{source:?}").to_lowercase()
            );
            eprintln!(
                "Details: source={source:?} at {}, filters: search={}, rows={}, since={},
                until={}, lines={:?}, reverse={}, mode={:?}; result 0 rows (maybe file empty or all filtered out)",
                vf.path.display(),
                search_re.is_some(),
                row_filters.is_some(),
                since.is_some(),
                until.is_some(),
                lines_outer,
                reverse,
                mode
            );
            exit(2);
        }
    };
    // For table output, use pager (less -S -R) when stdout is a TTY and --no-pager not set.
    // Raw output (tab-separated) never uses pager — use --raw for pipe/grep.
    let output = format!("{table}\n");
    pager_or_print(&output, no_pager);
}

fn print_raw_file<T>(
    source: LogSource,
    lines: Option<u64>,
    reverse: bool,
    search_re: Option<&Regex>,
    row_filters: Option<&[(String, String)]>,
    since: Option<&DateTime<Utc>>,
    until: Option<&DateTime<Utc>>,
) where
    T: TableDisplay + FromLogEntry,
{
    if let Some(filters) = row_filters
        && let Err(e) = validate_row_columns::<T>(filters)
    {
        eprintln!("Error: Invalid --rows filter: {e}");
        eprintln!(
            "Hint: Use --rows col=value[,col2=value2] — e.g. --rows host=myhost,process=sshd (-F). Check --list-columns for {source:?} available columns."
        );
        eprintln!("Details: {e}");
        exit(2);
    }

    let ps = possible_paths(source);
    let attempted: Vec<String> = ps.iter().map(|p| p.path.to_string()).collect();
    let vf = match filtered_finfo(ps) {
        Some(e) => e,
        None => {
            eprintln!("Error: No readable log file found for {source:?}.");
            eprintln!(
                "Hint: Check that log files exist ({}) and that Cassandra has read access — 
                try 'sudo ./cap.sh' or run with sudo. See 'cassandra {} --help' for expected paths.",
                attempted.join(", "),
                format!("{source:?}").to_lowercase()
            );
            eprintln!("Details: attempted paths: {}", attempted.join(", "));
            exit(2);
        }
    };

    use crate::parsers::selector::LogParser;
    use crate::parsers::{auth::AuthLog, sys::SysLog, wtmp::WtmpLog};

    let boxed_iter: Box<dyn Iterator<Item = Result<crate::models::LogEntry, ParseError>>> =
        match source {
            LogSource::Sys => {
                let p = SysLog;
                match p.try_iter(&vf.path) {
                    Ok(it) => it,
                    Err(e) => {
                        eprintln!(
                            "Error: I/O error opening {} at {}: {e:?}",
                            format!("{source:?}").to_lowercase(),
                            vf.path.display()
                        );
                        eprintln!(
                            "Hint: Ensure the binary has read access — try 'sudo ./cap.sh', check permissions, or run with sudo."
                        );
                        eprintln!("Details: {e:?}");
                        exit(2);
                    }
                }
            }
            LogSource::Auth => {
                let p = AuthLog;
                match p.try_iter(&vf.path) {
                    Ok(it) => it,
                    Err(e) => {
                        eprintln!(
                            "Error: I/O error opening {} at {}: {e:?}",
                            format!("{source:?}").to_lowercase(),
                            vf.path.display()
                        );
                        eprintln!(
                            "Hint: Ensure the binary has read access — try 'sudo ./cap.sh', check permissions, or run with sudo."
                        );
                        eprintln!("Details: {e:?}");
                        exit(2);
                    }
                }
            }
            LogSource::Wtmp => {
                let p = WtmpLog;
                match p.try_iter(&vf.path) {
                    Ok(it) => it,
                    Err(e) => {
                        eprintln!(
                            "Error: I/O error opening wtmp at {}: {e:?}",
                            vf.path.display()
                        );
                        eprintln!(
                            "Hint: Check /var/log/wtmp exists and has read access — try 'sudo ./cap.sh'."
                        );
                        eprintln!("Details: {e:?}");
                        exit(2);
                    }
                }
            }
        };

    let has_time = since.is_some() || until.is_some();
    let now = Utc::now();

    let matches_time = |entry: &crate::models::LogEntry| -> bool {
        if !has_time {
            return true;
        }
        if matches!(entry, crate::models::LogEntry::Wtmp(_)) {
            return true;
        }
        let dt_opt = match entry {
            crate::models::LogEntry::Sys(r) => rfc3164_to_datetime(&r.timestamp, now).ok(),
            crate::models::LogEntry::Auth(r) => rfc3164_to_datetime(&r.timestamp, now).ok(),
            crate::models::LogEntry::Journal(j) => {
                if let Some(ts) = &j.source_realtime_timestamp {
                    if let Ok(micros) = ts.parse::<i64>() {
                        Some(
                            Utc.timestamp_micros(micros)
                                .single()
                                .expect("valid timestamp"),
                        )
                    } else {
                        None
                    }
                } else if let Some(ts) = &j.source_boottime_timestamp {
                    if let Ok(micros) = ts.parse::<i64>() {
                        Some(
                            Utc.timestamp_micros(micros)
                                .single()
                                .expect("valid timestamp"),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            crate::models::LogEntry::Wtmp(_) => None,
        };
        if let Some(dt) = dt_opt {
            if let Some(since_dt) = since
                && dt < *since_dt
            {
                return false;
            }
            if let Some(until_dt) = until
                && dt > *until_dt
            {
                return false;
            }
            true
        } else {
            false
        }
    };

    let matches_rows = |entry: &crate::models::LogEntry| -> bool {
        if let Some(filters) = row_filters {
            if filters.is_empty() {
                return true;
            }
            if let Some(rec) = T::from_entry(entry) {
                let fields = rec.fields();
                let headers = T::headers();
                for (col, val) in filters {
                    if let Some(idx) = headers.iter().position(|h| h.eq_ignore_ascii_case(col))
                        && fields[idx] != *val
                    {
                        return false;
                    }
                }
                true
            } else {
                false
            }
        } else {
            true
        }
    };

    let matches_search = |entry: &crate::models::LogEntry| -> bool {
        if let Some(re) = search_re {
            if let Some(rec) = T::from_entry(entry) {
                rec.fields().iter().any(|f| re.is_match(f))
            } else {
                false
            }
        } else {
            true
        }
    };

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    let headers = T::headers();
    if writeln!(handle, "{}", headers.join("\t")).is_err() {
        exit(1);
    }

    if let Some(n) = lines {
        let n_usize = n as usize;
        if n_usize == 0 {
            let _ = handle.flush();
            return;
        }
        let mut deque: VecDeque<crate::models::LogEntry> = VecDeque::with_capacity(n_usize);
        for res in boxed_iter {
            let entry = match res {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Warning: skipping malformed line: {e:?}");
                    continue;
                }
            };
            if !matches_time(&entry) || !matches_rows(&entry) || !matches_search(&entry) {
                continue;
            }
            if deque.len() == n_usize {
                deque.pop_front();
            }
            deque.push_back(entry);
        }
        if reverse {
            for entry in deque.into_iter().rev() {
                if let Some(rec) = T::from_entry(&entry) {
                    let line = rec.fields().join("\t");
                    if writeln!(handle, "{line}").is_err() {
                        break;
                    }
                }
            }
        } else {
            for entry in deque {
                if let Some(rec) = T::from_entry(&entry) {
                    let line = rec.fields().join("\t");
                    if writeln!(handle, "{line}").is_err() {
                        break;
                    }
                }
            }
        }
        let _ = handle.flush();
    } else if reverse {
        let mut filtered: Vec<crate::models::LogEntry> = Vec::new();
        for res in boxed_iter {
            let entry = match res {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Warning: skipping malformed line: {e:?}");
                    continue;
                }
            };
            if !matches_time(&entry) || !matches_rows(&entry) || !matches_search(&entry) {
                continue;
            }
            filtered.push(entry);
        }
        filtered.reverse();
        for entry in filtered {
            if let Some(rec) = T::from_entry(&entry) {
                let line = rec.fields().join("\t");
                if writeln!(handle, "{line}").is_err() {
                    break;
                }
            }
        }
        let _ = handle.flush();
    } else {
        let mut count = 0usize;
        for res in boxed_iter {
            let entry = match res {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Warning: skipping malformed line: {e:?}");
                    continue;
                }
            };
            if !matches_time(&entry) || !matches_rows(&entry) || !matches_search(&entry) {
                continue;
            }
            if let Some(rec) = T::from_entry(&entry) {
                let line = rec.fields().join("\t");
                if writeln!(handle, "{line}").is_err() {
                    break;
                }
                count += 1;
                if count % 100 == 0 {
                    let _ = handle.flush();
                }
            }
        }
        let _ = handle.flush();
    }
}

fn print_raw_journal(
    scope: JournalScope,
    gravity: Option<GravityArgs>,
    since_usec: Option<u64>,
    until_usec: Option<u64>,
    lines: Option<u64>,
    reverse: bool,
    search_re: Option<&Regex>,
    row_filters: Option<&[(String, String)]>,
) {
    if let Some(filters) = row_filters
        && let Err(e) = validate_row_columns::<JournalRecord>(filters)
    {
        eprintln!("Error: Invalid --rows filter for journal: {e}");
        eprintln!(
            "Hint: Use -F col=value — e.g. -F systemd_unit=sshd.service. Check --list-columns for journal (scope={scope:?})."
        );
        eprintln!("Details: {e}");
        exit(2);
    }

    use crate::parsers::journal::JournalLog;
    use crate::parsers::selector::JournalParser;

    let jlog = JournalLog;
    let mut journal = match jlog.connect(scope.clone()) {
        Ok(j) => j,
        Err(e) => {
            eprintln!(
                "Error: Cannot retrieve entries from journal (scope={scope:?}) — journal unavailable or no permission."
            );
            eprintln!(
                "Hint: Check that systemd-journald is running,
                try --scope user vs --scope system, and ensure read access — try 'sudo ./cap.sh' or 'journalctl --verify'."
            );
            eprintln!("Details: {e:#?}");
            exit(2);
        }
    };

    // for no `since` we use efficient `previous_skip(lines.unwrap_or(50))` inside `try_iter`.
    let lines_for_iter = if since_usec.is_some() { None } else { lines };
    let iter = match jlog.try_iter(&mut journal, lines_for_iter, since_usec, until_usec) {
        Ok(it) => it,
        Err(e) => {
            eprintln!(
                "Error: Cannot read journal (scope={scope:?}) at realtime {}: {e:#?}",
                since_usec
                    .map(|u| u.to_string())
                    .unwrap_or_else(|| "tail".to_string())
            );
            eprintln!(
                "Hint: Try without --since/--until, check journalctl, or try --scope system vs user."
            );
            eprintln!("Details: {e:#?}");
            exit(2);
        }
    };

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let headers = JournalRecord::headers();
    if writeln!(handle, "{}", headers.join("\t")).is_err() {
        exit(1);
    }

    let mut filtered_deque: Option<VecDeque<crate::models::LogEntry>> =
        if since_usec.is_some() && lines.is_some() {
            Some(VecDeque::with_capacity(lines.unwrap() as usize))
        } else {
            None
        };
    let mut filtered_vec: Vec<crate::models::LogEntry> = Vec::new();

    if filtered_deque.is_some() {
        let n_usize = lines.unwrap() as usize;
        for res in iter {
            let entry = match res {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Warning: skipping journal entry: {e:?}");
                    continue;
                }
            };
            if let Some(g) = &gravity
                && !matches_gravity(&entry, g)
            {
                continue;
            }
            if let Some(filters) = row_filters
                && !{
                    if let Some(rec) = JournalRecord::from_entry(&entry) {
                        let fields = rec.fields();
                        let headers = JournalRecord::headers();
                        let mut ok = true;
                        for (col, val) in filters {
                            if let Some(idx) =
                                headers.iter().position(|h| h.eq_ignore_ascii_case(col))
                                && fields[idx] != *val
                            {
                                ok = false;
                                break;
                            }
                        }
                        ok
                    } else {
                        false
                    }
                }
            {
                continue;
            }
            if let Some(re) = search_re
                && !{
                    if let Some(rec) = JournalRecord::from_entry(&entry) {
                        rec.fields().iter().any(|f| re.is_match(f))
                    } else {
                        false
                    }
                }
            {
                continue;
            }
            let deque = filtered_deque.as_mut().unwrap();
            if deque.len() == n_usize {
                deque.pop_front();
            }
            deque.push_back(entry);
        }
        let deque = filtered_deque.unwrap();
        if reverse {
            for entry in deque.into_iter().rev() {
                if let Some(rec) = JournalRecord::from_entry(&entry) {
                    let _ = writeln!(handle, "{}", rec.fields().join("\t"));
                }
            }
        } else {
            for entry in deque {
                if let Some(rec) = JournalRecord::from_entry(&entry) {
                    let _ = writeln!(handle, "{}", rec.fields().join("\t"));
                }
            }
        }
        let _ = handle.flush();
        return;
    }

    if reverse && filtered_deque.is_none() {
        for res in iter {
            let entry = match res {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Warning: skipping journal entry: {e:?}");
                    continue;
                }
            };
            if let Some(g) = &gravity
                && !matches_gravity(&entry, g)
            {
                continue;
            }
            if let Some(filters) = row_filters {
                let ok = if let Some(rec) = JournalRecord::from_entry(&entry) {
                    let fields = rec.fields();
                    let headers = JournalRecord::headers();
                    let mut ok = true;
                    for (col, val) in filters {
                        if let Some(idx) = headers.iter().position(|h| h.eq_ignore_ascii_case(col))
                            && fields[idx] != *val
                        {
                            ok = false;
                            break;
                        }
                    }
                    ok
                } else {
                    false
                };
                if !ok {
                    continue;
                }
            }
            if let Some(re) = search_re {
                let ok = if let Some(rec) = JournalRecord::from_entry(&entry) {
                    rec.fields().iter().any(|f| re.is_match(f))
                } else {
                    false
                };
                if !ok {
                    continue;
                }
            }
            filtered_vec.push(entry);
        }
        filtered_vec.reverse();
        for entry in filtered_vec {
            if let Some(rec) = JournalRecord::from_entry(&entry) {
                let _ = writeln!(handle, "{}", rec.fields().join("\t"));
            }
        }
        let _ = handle.flush();
        return;
    }

    // already handled via `previous_skip` in `try_iter`, so we can stream directly.
    let mut count = 0usize;
    for res in iter {
        let entry = match res {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Warning: skipping journal entry: {e:?}");
                continue;
            }
        };
        if let Some(g) = &gravity
            && !matches_gravity(&entry, g)
        {
            continue;
        }
        if let Some(filters) = row_filters {
            let ok = if let Some(rec) = JournalRecord::from_entry(&entry) {
                let fields = rec.fields();
                let headers = JournalRecord::headers();
                let mut ok = true;
                for (col, val) in filters {
                    if let Some(idx) = headers.iter().position(|h| h.eq_ignore_ascii_case(col))
                        && fields[idx] != *val
                    {
                        ok = false;
                        break;
                    }
                }
                ok
            } else {
                false
            };
            if !ok {
                continue;
            }
        }
        if let Some(re) = search_re {
            let ok = if let Some(rec) = JournalRecord::from_entry(&entry) {
                rec.fields().iter().any(|f| re.is_match(f))
            } else {
                false
            };
            if !ok {
                continue;
            }
        }
        if let Some(rec) = JournalRecord::from_entry(&entry) {
            let _ = writeln!(handle, "{}", rec.fields().join("\t"));
            count += 1;
            if count % 100 == 0 {
                let _ = handle.flush();
            }
        }
    }
    let _ = handle.flush();
}

fn possible_paths(lsource: LogSource) -> Vec<&'static SourceCandidate> {
    SOURCES.iter().filter(|sc| sc.source == lsource).collect()
}

fn filtered_finfo(psc: Vec<&SourceCandidate>) -> Option<Finfo> {
    for p in psc {
        let f = Finfo::gather_info(p)
            .map_err(|e| format!("An error ocurred during the Finfo construction: {e:?}"))
            .ok()?;
        if f.path.exists() {
            return Some(f);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LogEntry, journal::JournalRecord, sys::SysRecord};
    use chrono::TimeZone;

    fn args(
        summary: Option<Vec<String>>,
        compact: Option<usize>,
        key: Option<String>,
        standard: bool,
    ) -> ArgsC {
        ArgsC {
            summary,
            compact,
            key,
            standard,
            raw: false,
            no_pager: false,
            rows: None,
            lines: None,
            reverse: false,
            search: None,
            since: None,
            until: None,
            log: LogKey::Sys {
                list_columns: false,
            },
        }
    }

    #[test]
    fn table_mode_defaults_to_standard() {
        let mode = args(None, None, None, false).table_mode();
        assert!(matches!(mode, TableMode::Standard));
    }

    #[test]
    fn table_mode_summary_wins_over_compact_and_key() {
        let mode = args(
            Some(vec!["message".to_string()]),
            Some(10),
            Some("message".to_string()),
            false,
        )
        .table_mode();
        assert!(matches!(mode, TableMode::Summary { .. }));
    }

    #[test]
    fn table_mode_compact_wins_over_key() {
        let mode = args(None, Some(20), Some("message".to_string()), false).table_mode();
        assert!(matches!(mode, TableMode::Compact { max_col_width: 20 }));
    }

    #[test]
    fn table_mode_key_maps_to_keyvalue() {
        let mode = args(None, None, Some("message".to_string()), false).table_mode();
        assert!(matches!(mode, TableMode::KeyValue));
    }

    #[test]
    fn possible_paths_filters_by_source() {
        let sys = possible_paths(LogSource::Sys);
        assert!(!sys.is_empty());
        assert!(sys.iter().all(|s| s.source == LogSource::Sys));

        let auth = possible_paths(LogSource::Auth);
        assert!(!auth.is_empty());
        assert!(auth.iter().all(|s| s.source == LogSource::Auth));
    }

    #[test]
    fn filtered_finfo_returns_none_when_nothing_exists() {
        let missing = SourceCandidate {
            source: LogSource::Sys,
            path: "/definitely/missing/cassandra-test-cli.log",
        };
        assert!(filtered_finfo(vec![&missing]).is_none());
    }

    #[test]
    fn parse_row_filters_ok() {
        let raw = vec!["host=myhost".to_string(), "process=sshd".to_string()];
        let got = parse_row_filters(&raw).expect("parse ok");
        assert_eq!(
            got,
            vec![
                ("host".to_string(), "myhost".to_string()),
                ("process".to_string(), "sshd".to_string())
            ]
        );
    }

    #[test]
    fn parse_row_filters_rejects_missing_eq() {
        let raw = vec!["hostmyhost".to_string()];
        assert!(parse_row_filters(&raw).is_err());
    }

    #[test]
    fn parse_row_filters_rejects_empty() {
        let raw = vec!["host=".to_string()];
        assert!(parse_row_filters(&raw).is_err());
    }

    #[test]
    fn validate_row_columns_ok_and_err() {
        assert!(
            validate_row_columns::<SysRecord>(&[("host".to_string(), "x".to_string())]).is_ok()
        );
        assert!(
            validate_row_columns::<SysRecord>(&[("HOST".to_string(), "x".to_string())]).is_ok()
        );
        assert!(
            validate_row_columns::<SysRecord>(&[("nope".to_string(), "x".to_string())]).is_err()
        );
    }

    #[test]
    fn apply_rows_filter_keeps_matching() {
        let entries = vec![
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "t".to_string(),
                host: "myhost".to_string(),
                process: "sshd".to_string(),
                message: "m1".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "t".to_string(),
                host: "other".to_string(),
                process: "sshd".to_string(),
                message: "m2".to_string(),
            }),
        ];
        let filters = vec![("host".to_string(), "myhost".to_string())];
        let out = apply_rows_filter::<SysRecord>(entries, &filters);
        assert_eq!(out.len(), 1);
        match &out[0] {
            LogEntry::Sys(r) => assert_eq!(r.host, "myhost"),
            _ => panic!("wrong"),
        }
    }

    #[test]
    fn apply_search_filter_regex() {
        let entries = vec![
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "t".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "error failed".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "t".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "all good".to_string(),
            }),
        ];
        let re = Regex::new("error").unwrap();
        let out = apply_search_filter::<SysRecord>(entries, &re);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn apply_search_filter_matches_any_field() {
        let entries = vec![
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "t".to_string(),
                host: "myhost".to_string(),
                process: "p".to_string(),
                message: "m".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "t".to_string(),
                host: "other".to_string(),
                process: "p".to_string(),
                message: "m".to_string(),
            }),
        ];
        let re = Regex::new("myhost").unwrap();
        let out = apply_search_filter::<SysRecord>(entries, &re);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn gravity_filter_critical_keeps_low_priority_numbers() {
        let entries = vec![
            LogEntry::Journal(Box::new(JournalRecord {
                message: "m".to_string(),
                priority: Some("2".to_string()),
                code_file: None,
                code_func: None,
                code_line: None,
                syslog_facility: None,
                syslog_identifier: None,
                tid: None,
                audit_loginuid: None,
                audit_session: None,
                boot_id: None,
                gid: None,
                hostname: None,
                machine_id: None,
                pid: None,
                runtime_scope: None,
                selinux_context: None,
                source_monotonic_timestamp: None,
                source_boottime_timestamp: None,
                source_realtime_timestamp: None,
                systemd_cgroup: None,
                systemd_owner_uid: None,
                systemd_slice: None,
                systemd_unit: None,
                systemd_user_slice: None,
                transport: None,
                uid: None,
            })),
            LogEntry::Journal(Box::new(JournalRecord {
                message: "m".to_string(),
                priority: Some("6".to_string()),
                code_file: None,
                code_func: None,
                code_line: None,
                syslog_facility: None,
                syslog_identifier: None,
                tid: None,
                audit_loginuid: None,
                audit_session: None,
                boot_id: None,
                gid: None,
                hostname: None,
                machine_id: None,
                pid: None,
                runtime_scope: None,
                selinux_context: None,
                source_monotonic_timestamp: None,
                source_boottime_timestamp: None,
                source_realtime_timestamp: None,
                systemd_cgroup: None,
                systemd_owner_uid: None,
                systemd_slice: None,
                systemd_unit: None,
                systemd_user_slice: None,
                transport: None,
                uid: None,
            })),
        ];
        let out = apply_gravity_filter(entries, &GravityArgs::Critical);
        assert_eq!(out.len(), 1);
        match &out[0] {
            LogEntry::Journal(j) => assert_eq!(j.priority.as_deref(), Some("2")),
            _ => panic!("wrong"),
        }
    }

    #[test]
    fn gravity_filter_none_priority_is_filtered_out() {
        let entries = vec![LogEntry::Journal(Box::new(JournalRecord {
            message: "m".to_string(),
            priority: None,
            code_file: None,
            code_func: None,
            code_line: None,
            syslog_facility: None,
            syslog_identifier: None,
            tid: None,
            audit_loginuid: None,
            audit_session: None,
            boot_id: None,
            gid: None,
            hostname: None,
            machine_id: None,
            pid: None,
            runtime_scope: None,
            selinux_context: None,
            source_monotonic_timestamp: None,
            source_boottime_timestamp: None,
            source_realtime_timestamp: None,
            systemd_cgroup: None,
            systemd_owner_uid: None,
            systemd_slice: None,
            systemd_unit: None,
            systemd_user_slice: None,
            transport: None,
            uid: None,
        }))];
        let out = apply_gravity_filter(entries, &GravityArgs::Low);
        assert!(out.is_empty());
    }

    #[test]
    fn parse_row_filters_trims_spaces() {
        let raw = vec![" host = myhost ".to_string(), "process = sshd ".to_string()];
        let got = parse_row_filters(&raw).expect("parse ok");
        assert_eq!(
            got,
            vec![
                ("host".to_string(), "myhost".to_string()),
                ("process".to_string(), "sshd".to_string())
            ]
        );
    }

    #[test]
    fn apply_rows_filter_multiple_and() {
        let entries = vec![
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "t".to_string(),
                host: "myhost".to_string(),
                process: "sshd".to_string(),
                message: "m1".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "t".to_string(),
                host: "myhost".to_string(),
                process: "cron".to_string(),
                message: "m2".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "t".to_string(),
                host: "other".to_string(),
                process: "sshd".to_string(),
                message: "m3".to_string(),
            }),
        ];
        let filters = vec![
            ("host".to_string(), "myhost".to_string()),
            ("process".to_string(), "sshd".to_string()),
        ];
        let out = apply_rows_filter::<SysRecord>(entries, &filters);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn apply_rows_filter_case_insensitive_column() {
        let entries = vec![LogEntry::Sys(SysRecord {
            priority: None,
            timestamp: "t".to_string(),
            host: "myhost".to_string(),
            process: "p".to_string(),
            message: "m".to_string(),
        })];
        let filters = vec![("HOST".to_string(), "myhost".to_string())];
        let out = apply_rows_filter::<SysRecord>(entries, &filters);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn gravity_filter_medium_and_low() {
        let mk = |p: &str| {
            LogEntry::Journal(Box::new(JournalRecord {
                message: "m".to_string(),
                priority: Some(p.to_string()),
                code_file: None,
                code_func: None,
                code_line: None,
                syslog_facility: None,
                syslog_identifier: None,
                tid: None,
                audit_loginuid: None,
                audit_session: None,
                boot_id: None,
                gid: None,
                hostname: None,
                machine_id: None,
                pid: None,
                runtime_scope: None,
                selinux_context: None,
                source_monotonic_timestamp: None,
                source_boottime_timestamp: None,
                source_realtime_timestamp: None,
                systemd_cgroup: None,
                systemd_owner_uid: None,
                systemd_slice: None,
                systemd_unit: None,
                systemd_user_slice: None,
                transport: None,
                uid: None,
            }))
        };
        let entries_med = vec![mk("4"), mk("5"), mk("6"), mk("7"), mk("2")];
        let med = apply_gravity_filter(entries_med, &GravityArgs::Medium);
        assert_eq!(med.len(), 2);
        let entries_low = vec![mk("4"), mk("5"), mk("6"), mk("7"), mk("2")];
        let low = apply_gravity_filter(entries_low, &GravityArgs::Low);
        assert_eq!(low.len(), 2);
    }

    #[test]
    fn apply_search_filter_wtmp() {
        use crate::models::wtmp::WtmpRecord;
        let entries = vec![
            LogEntry::Wtmp(WtmpRecord {
                ut_type: 7,
                ut_pid: 1,
                ut_dname: "pts/0".to_string(),
                ut_id: "01".to_string(),
                ut_user: "alice".to_string(),
                ut_host: "myhost".to_string(),
                e_termination: 0,
                e_exit: 0,
            }),
            LogEntry::Wtmp(WtmpRecord {
                ut_type: 7,
                ut_pid: 2,
                ut_dname: "pts/1".to_string(),
                ut_id: "02".to_string(),
                ut_user: "bob".to_string(),
                ut_host: "other".to_string(),
                e_termination: 0,
                e_exit: 0,
            }),
        ];
        let re = Regex::new("alice").unwrap();
        let out = apply_search_filter::<WtmpRecord>(entries, &re);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn apply_rows_filter_wtmp() {
        use crate::models::wtmp::WtmpRecord;
        let entries = vec![
            LogEntry::Wtmp(WtmpRecord {
                ut_type: 7,
                ut_pid: 1,
                ut_dname: "pts/0".to_string(),
                ut_id: "01".to_string(),
                ut_user: "alice".to_string(),
                ut_host: "h1".to_string(),
                e_termination: 0,
                e_exit: 0,
            }),
            LogEntry::Wtmp(WtmpRecord {
                ut_type: 7,
                ut_pid: 2,
                ut_dname: "pts/1".to_string(),
                ut_id: "02".to_string(),
                ut_user: "bob".to_string(),
                ut_host: "h2".to_string(),
                e_termination: 0,
                e_exit: 0,
            }),
        ];
        let filters = vec![("ut_user".to_string(), "bob".to_string())];
        let out = apply_rows_filter::<WtmpRecord>(entries, &filters);
        assert_eq!(out.len(), 1);
        match &out[0] {
            LogEntry::Wtmp(r) => assert_eq!(r.ut_user, "bob"),
            _ => panic!("wrong"),
        }
    }

    #[test]
    fn apply_search_filter_regex_special() {
        let entries = vec![
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "t".to_string(),
                host: "h".to_string(),
                process: "sshd".to_string(),
                message: "Failed password".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "t".to_string(),
                host: "h".to_string(),
                process: "sshd".to_string(),
                message: "Accepted".to_string(),
            }),
        ];
        let re = Regex::new("Failed.*password").unwrap();
        let out = apply_search_filter::<SysRecord>(entries, &re);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn apply_time_filter_sys_since_until() {
        let entries = vec![
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 11 22:14:15".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "m1".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 12 10:00:00".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "m2".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 13 01:00:00".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "m3".to_string(),
            }),
        ];
        let since = Utc.with_ymd_and_hms(2024, 10, 12, 0, 0, 0).unwrap();
        let until = Utc.with_ymd_and_hms(2024, 10, 12, 23, 59, 59).unwrap();
        let now = Utc.with_ymd_and_hms(2024, 10, 14, 0, 0, 0).unwrap();
        let filtered: Vec<_> = entries
            .into_iter()
            .filter(|e| {
                if let crate::models::LogEntry::Sys(r) = e {
                    if let Ok(dt) = rfc3164_to_datetime(&r.timestamp, now) {
                        dt >= since && dt <= until
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .collect();
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn apply_lines_limit_keeps_last_n() {
        let entries = vec![
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 11 22:14:15".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "1".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 11 22:14:16".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "2".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 11 22:14:17".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "3".to_string(),
            }),
        ];
        let out = apply_lines_limit(entries, Some(2), false);
        assert_eq!(out.len(), 2);
        match (&out[0], &out[1]) {
            (LogEntry::Sys(a), LogEntry::Sys(b)) => {
                assert_eq!(a.message, "2");
                assert_eq!(b.message, "3");
            }
            _ => panic!("wrong"),
        }
    }

    #[test]
    fn apply_lines_limit_reverse() {
        let entries = vec![
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 11 22:14:15".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "1".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 11 22:14:16".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "2".to_string(),
            }),
        ];
        let out = apply_lines_limit(entries, Some(2), true);
        assert_eq!(out.len(), 2);
        match (&out[0], &out[1]) {
            (LogEntry::Sys(a), LogEntry::Sys(b)) => {
                assert_eq!(a.message, "2");
                assert_eq!(b.message, "1");
            }
            _ => panic!("wrong"),
        }
    }

    #[test]
    fn apply_time_filter_with_fixed_now() {
        let entries = vec![
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 11 22:14:15".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "m1".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 12 10:00:00".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "m2".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 13 01:00:00".to_string(),
                host: "h".to_string(),
                process: "p".to_string(),
                message: "m3".to_string(),
            }),
        ];
        let since = Utc.with_ymd_and_hms(2024, 10, 12, 0, 0, 0).unwrap();
        let until = Utc.with_ymd_and_hms(2024, 10, 12, 23, 59, 59).unwrap();
        let now = Utc.with_ymd_and_hms(2024, 10, 14, 0, 0, 0).unwrap();
        let out = apply_time_filter_with_now(entries, Some(&since), Some(&until), now);
        assert_eq!(out.len(), 1);
        match &out[0] {
            LogEntry::Sys(r) => assert_eq!(r.message, "m2"),
            _ => panic!("wrong"),
        }
    }

    #[test]
    fn apply_time_filter_journal() {
        let micros = Utc
            .with_ymd_and_hms(2024, 10, 12, 10, 0, 0)
            .unwrap()
            .timestamp_micros();
        let entries = vec![
            LogEntry::Journal(Box::new(JournalRecord {
                message: "m1".to_string(),
                priority: None,
                code_file: None,
                code_func: None,
                code_line: None,
                syslog_facility: None,
                syslog_identifier: None,
                tid: None,
                audit_loginuid: None,
                audit_session: None,
                boot_id: None,
                gid: None,
                hostname: None,
                machine_id: None,
                pid: None,
                runtime_scope: None,
                selinux_context: None,
                source_monotonic_timestamp: None,
                source_boottime_timestamp: None,
                source_realtime_timestamp: Some((micros - 1_000_000).to_string()),
                systemd_cgroup: None,
                systemd_owner_uid: None,
                systemd_slice: None,
                systemd_unit: None,
                systemd_user_slice: None,
                transport: None,
                uid: None,
            })),
            LogEntry::Journal(Box::new(JournalRecord {
                message: "m2".to_string(),
                priority: None,
                code_file: None,
                code_func: None,
                code_line: None,
                syslog_facility: None,
                syslog_identifier: None,
                tid: None,
                audit_loginuid: None,
                audit_session: None,
                boot_id: None,
                gid: None,
                hostname: None,
                machine_id: None,
                pid: None,
                runtime_scope: None,
                selinux_context: None,
                source_monotonic_timestamp: None,
                source_boottime_timestamp: None,
                source_realtime_timestamp: Some(micros.to_string()),
                systemd_cgroup: None,
                systemd_owner_uid: None,
                systemd_slice: None,
                systemd_unit: None,
                systemd_user_slice: None,
                transport: None,
                uid: None,
            })),
            LogEntry::Journal(Box::new(JournalRecord {
                message: "m3".to_string(),
                priority: None,
                code_file: None,
                code_func: None,
                code_line: None,
                syslog_facility: None,
                syslog_identifier: None,
                tid: None,
                audit_loginuid: None,
                audit_session: None,
                boot_id: None,
                gid: None,
                hostname: None,
                machine_id: None,
                pid: None,
                runtime_scope: None,
                selinux_context: None,
                source_monotonic_timestamp: None,
                source_boottime_timestamp: None,
                source_realtime_timestamp: Some((micros + 7_200_000_000).to_string()),
                systemd_cgroup: None,
                systemd_owner_uid: None,
                systemd_slice: None,
                systemd_unit: None,
                systemd_user_slice: None,
                transport: None,
                uid: None,
            })),
        ];
        let since = Utc.with_ymd_and_hms(2024, 10, 12, 9, 0, 0).unwrap();
        let until = Utc.with_ymd_and_hms(2024, 10, 12, 11, 0, 0).unwrap();
        let now = Utc.with_ymd_and_hms(2024, 10, 14, 0, 0, 0).unwrap();
        let out = apply_time_filter_with_now(entries, Some(&since), Some(&until), now);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn apply_time_filter_wtmp_ignores() {
        use crate::models::wtmp::WtmpRecord;
        let entries = vec![LogEntry::Wtmp(WtmpRecord {
            ut_type: 7,
            ut_pid: 1,
            ut_dname: "pts/0".to_string(),
            ut_id: "01".to_string(),
            ut_user: "alice".to_string(),
            ut_host: "h".to_string(),
            e_termination: 0,
            e_exit: 0,
        })];
        let since = Utc.with_ymd_and_hms(2024, 10, 12, 0, 0, 0).unwrap();
        let now = Utc.with_ymd_and_hms(2024, 10, 14, 0, 0, 0).unwrap();
        let out = apply_time_filter_with_now(entries, Some(&since), None, now);
        assert_eq!(out.len(), 1);
    }
}
