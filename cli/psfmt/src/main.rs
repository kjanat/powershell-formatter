//! `psfmt` — native PowerShell formatter.
//!
//! Baseline contract: `psfmt < in.ps1 > out.ps1`. Formatted source is the
//! only thing written to stdout; diagnostics go to stderr.
//!
//! Exit codes:
//!   0  success (or `--check` with nothing to change)
//!   1  `--check` found files that would change
//!   2  usage error
//!   3  I/O error
//!   4  input could not be formatted safely (preserved unchanged)

use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pwsh_formatter::{
    Diagnostic, FormatOptions, FormatRange, FormatResult, JsonCatalog, format, format_range,
    format_range_with_catalog, format_with_catalog,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
psfmt — fast, standalone PowerShell formatter

USAGE:
    psfmt [OPTIONS] [FILES...]

With no files, reads stdin and writes formatted source to stdout.

OPTIONS:
    -w, --write             Rewrite files in place (atomic)
        --check             Exit 1 if any input would be reformatted
        --config <PATH>     JSON configuration (camelCase FormatOptions);
                            mutually exclusive with --preset
        --preset <NAME>     default | otbs | allman | stroustrup
        --catalog <PATH>    JSON command catalog for command/parameter casing
        --range <RANGE>     Format only startLine,startCol,endLine,endCol
                            (1-based, like Invoke-Formatter -Range)
        --stdin-filepath <PATH>  File name used in diagnostics for stdin
    -h, --help              Print help
    -V, --version           Print version
";

struct Cli {
    files: Vec<PathBuf>,
    write: bool,
    check: bool,
    options: FormatOptions,
    catalog: Option<JsonCatalog>,
    range: Option<FormatRange>,
    stdin_name: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("psfmt: {err}");
            ExitCode::from(err.code)
        }
    }
}

struct CliError {
    code: u8,
    message: String,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

fn usage_err(message: impl Into<String>) -> CliError {
    CliError {
        code: 2,
        message: message.into(),
    }
}

fn io_err(context: &str, err: &io::Error) -> CliError {
    CliError {
        code: 3,
        message: format!("{context}: {err}"),
    }
}

fn parse_args() -> Result<Option<Cli>, CliError> {
    let mut cli = Cli {
        files: Vec::new(),
        write: false,
        check: false,
        options: FormatOptions::default(),
        catalog: None,
        range: None,
        stdin_name: "<stdin>".to_owned(),
    };
    let mut config_path: Option<PathBuf> = None;
    let mut preset: Option<String> = None;

    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        let Some(arg_str) = arg.to_str() else {
            cli.files.push(PathBuf::from(arg));
            continue;
        };
        let mut next_value = |name: &str| -> Result<String, CliError> {
            args.next()
                .and_then(|v| v.to_str().map(str::to_owned))
                .ok_or_else(|| usage_err(format!("{name} requires a value")))
        };
        match arg_str {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("psfmt {VERSION}");
                return Ok(None);
            }
            "-w" | "--write" => cli.write = true,
            "--check" => cli.check = true,
            "--config" => config_path = Some(PathBuf::from(next_value("--config")?)),
            "--preset" => preset = Some(next_value("--preset")?),
            "--catalog" => {
                let path = next_value("--catalog")?;
                let json = std::fs::read_to_string(&path)
                    .map_err(|e| io_err(&format!("reading catalog {path}"), &e))?;
                cli.catalog = Some(
                    JsonCatalog::from_json(&json)
                        .map_err(|e| usage_err(format!("invalid catalog {path}: {e}")))?,
                );
            }
            "--range" => {
                let raw = next_value("--range")?;
                let parts: Vec<u32> = raw
                    .split([',', ':', '-'])
                    .map(|p| p.trim().parse::<u32>())
                    .collect::<Result<_, _>>()
                    .map_err(|_| {
                        usage_err(format!(
                            "invalid --range {raw:?}; expected startLine,startCol,endLine,endCol"
                        ))
                    })?;
                let [sl, sc, el, ec] = parts.as_slice() else {
                    return Err(usage_err("--range needs exactly four numbers"));
                };
                cli.range = Some(FormatRange {
                    start_line: *sl,
                    start_column: *sc,
                    end_line: *el,
                    end_column: *ec,
                });
            }
            "--stdin-filepath" => cli.stdin_name = next_value("--stdin-filepath")?,
            "--" => {
                cli.files.extend(args.by_ref().map(PathBuf::from));
                break;
            }
            s if s.starts_with('-') && s.len() > 1 => {
                return Err(usage_err(format!("unknown option {s:?} (see --help)")));
            }
            _ => cli.files.push(PathBuf::from(arg_str)),
        }
    }

    // A full-struct config replaces every field, so combining it with a
    // preset would silently discard the preset. Refuse rather than guess.
    if preset.is_some() && config_path.is_some() {
        return Err(usage_err("--preset and --config are mutually exclusive"));
    }
    if let Some(name) = preset {
        cli.options = match name.as_str() {
            "default" | "stroustrup" => FormatOptions::stroustrup(),
            "otbs" => FormatOptions::otbs(),
            "allman" => FormatOptions::allman(),
            other => {
                return Err(usage_err(format!(
                    "unknown preset {other:?}; expected default|otbs|allman|stroustrup"
                )));
            }
        };
    }
    if let Some(path) = config_path {
        let json = std::fs::read_to_string(&path)
            .map_err(|e| io_err(&format!("reading config {}", path.display()), &e))?;
        cli.options = serde_json::from_str(&json)
            .map_err(|e| usage_err(format!("invalid config {}: {e}", path.display())))?;
    }
    if cli.write && cli.check {
        return Err(usage_err("--write and --check are mutually exclusive"));
    }
    if cli.write && cli.files.is_empty() {
        return Err(usage_err("--write requires file arguments"));
    }
    Ok(Some(cli))
}

fn run_format(cli: &Cli, source: &str) -> FormatResult {
    match (&cli.catalog, cli.range) {
        (Some(cat), Some(range)) => format_range_with_catalog(source, &cli.options, cat, range),
        (Some(cat), None) => format_with_catalog(source, &cli.options, cat),
        (None, Some(range)) => format_range(source, &cli.options, range),
        (None, None) => format(source, &cli.options),
    }
}

fn report_diagnostics(name: &str, diagnostics: &[Diagnostic]) {
    for d in diagnostics {
        eprintln!(
            "{name}:{}:{}: {} [{}]",
            d.position.line,
            d.position.column,
            d.message,
            d.code.as_str()
        );
    }
}

fn run() -> Result<ExitCode, CliError> {
    let Some(cli) = parse_args()? else {
        return Ok(ExitCode::SUCCESS);
    };

    if cli.files.is_empty() {
        // stdin → stdout filter.
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| io_err("reading stdin", &e))?;
        let result = run_format(&cli, &source);
        report_diagnostics(&cli.stdin_name, &result.diagnostics);
        if cli.check {
            return Ok(if result.text == source {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            });
        }
        io::stdout()
            .write_all(result.text.as_bytes())
            .map_err(|e| io_err("writing stdout", &e))?;
        return Ok(if result.formatted {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(4)
        });
    }

    let mut would_change = false;
    let mut any_skipped = false;
    let mut any_io_error = false;
    for path in &cli.files {
        // A bad file must not abort the run: report it, keep formatting the
        // rest, and surface exit code 3 at the end.
        let source = match read_source(path) {
            Ok(s) => s,
            Err(message) => {
                eprintln!("psfmt: {message}");
                any_io_error = true;
                continue;
            }
        };
        let result = run_format(&cli, &source);
        report_diagnostics(&path.display().to_string(), &result.diagnostics);
        if !result.formatted {
            any_skipped = true;
            continue;
        }
        if cli.check {
            if result.text != source {
                would_change = true;
                eprintln!("{}: would be reformatted", path.display());
            }
        } else if cli.write {
            if result.text != source {
                match write_atomically(path, &result.text) {
                    Ok(()) => {}
                    Err(e) => {
                        eprintln!("psfmt: writing {}: {e}", path.display());
                        any_io_error = true;
                    }
                }
            }
        } else {
            // Default mode is a filter: emit the (possibly unchanged)
            // source, exactly like stdin mode does.
            io::stdout()
                .write_all(result.text.as_bytes())
                .map_err(|e| io_err("writing stdout", &e))?;
        }
    }

    Ok(if any_io_error {
        ExitCode::from(3)
    } else if any_skipped {
        ExitCode::from(4)
    } else if cli.check && would_change {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

/// Read a script as UTF-8, with a targeted message for the encoding
/// Windows tooling most often produces instead.
fn read_source(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return Err(format!(
            "reading {}: file is UTF-16 (byte-order mark found); psfmt reads UTF-8 — convert it first",
            path.display()
        ));
    }
    String::from_utf8(bytes).map_err(|e| {
        format!(
            "reading {}: stream did not contain valid UTF-8 ({e})",
            path.display()
        )
    })
}

/// Replace `path` with `contents` atomically (write temp + fsync + rename),
/// preserving the original permissions where practical.
fn write_atomically(path: &Path, contents: &str) -> io::Result<()> {
    // Resolve symlinks first: renaming over a symlink would replace the
    // link itself with a regular file instead of updating its target.
    let path = std::fs::canonicalize(path)?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "psfmt".to_owned());
    let tmp = dir.join(format!(".{file_name}.psfmt-{}", std::process::id()));
    let metadata = std::fs::metadata(&path).ok();

    let outcome = (|| -> io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(contents.as_bytes())?;
        // Rename gives atomic visibility, not durability: without the fsync
        // a power cut after the rename can leave an empty file behind.
        file.sync_all()?;
        drop(file);
        if let Some(meta) = &metadata {
            let _ = std::fs::set_permissions(&tmp, meta.permissions());
        }
        std::fs::rename(&tmp, &path)
    })();
    if outcome.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    outcome
}
