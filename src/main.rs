//! Port of the verdict path in `cli.py`.

#![allow(clippy::used_underscore_items)]

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use opencrbasic::{compiler, profiles, pystr};

const SPEC_VERSION: &str = "1.2.1";
const HELP: &str = "Usage: crbc [options] program [messagefile]\n\
    -v  Print version information before compiling\n\
    -d  Print version information and exit\n\
    -m  Accepted for compiler compatibility\n\
    --logger {cr6,cr1000x,cr300,cr350}  Override the logger inferred from the extension\n\
    --no-message-file  Write verdict to stdout only";

struct Options {
    version: bool,
    version_only: bool,
    logger: Option<String>,
    message_file: bool,
}

/// Port of `cli.py::_parse`.
fn _parse(
    argv: impl Iterator<Item = OsString>,
    stdout: &mut impl Write,
) -> io::Result<Option<(Options, Vec<OsString>)>> {
    let mut opts = Options {
        version: false,
        version_only: false,
        logger: None,
        message_file: true,
    };
    let mut positional = Vec::new();
    let mut argv = argv;
    while let Some(raw_arg) = argv.next() {
        let arg = raw_arg.to_string_lossy();
        if arg.starts_with("--") {
            match arg.split_once('=') {
                Some(("--logger", logger)) => opts.logger = Some(logger.to_owned()),
                _ if arg == "--logger" => {
                    opts.logger = Some(
                        argv.next()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
                _ if arg == "--no-message-file" => opts.message_file = false,
                _ if arg == "--help" => {
                    writeln!(stdout, "{HELP}")?;
                    return Ok(None);
                }
                _ => writeln!(stdout, "illegal option!")?,
            }
            continue;
        }
        if let Some(letters) = arg
            .strip_prefix('-')
            .filter(|letters| !letters.is_empty() && positional.is_empty())
        {
            for ch in letters.chars() {
                match ch.to_ascii_lowercase() {
                    'v' => opts.version = true,
                    'd' => opts.version_only = true,
                    'm' => {}
                    _ => writeln!(stdout, "illegal option!")?,
                }
            }
            continue;
        }
        positional.push(raw_arg);
    }
    Ok(Some((opts, positional)))
}

/// Port of `cli.py::_message_bytes`.
fn _message_bytes(lines: &[String]) -> Vec<u8> {
    let body = match lines {
        [header, blank, rest @ ..] if blank.is_empty() && header.ends_with("Failed!") => {
            format!("{header}\r\n\n{}\r\n", rest.join("\r\n"))
        }
        _ => format!("{}\r\n", lines.join("\r\n")),
    };
    pystr::encode_latin1_replace(&body)
}

/// Port of `cli.py::_write_message_file`.
fn _write_message_file(path: &Path, lines: &[String], version: Option<&str>) -> io::Result<()> {
    let mut bytes = version.map_or_else(Vec::new, |line| {
        pystr::encode_latin1_replace(&format!("[Version]\r\n{line}\r\n"))
    });
    bytes.extend(_message_bytes(lines));
    fs::write(path, bytes)
}

/// Port of `cli.py::main`.
fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut stdout = io::stdout().lock();
    let argv = env::args_os().skip(1);
    let Some((opts, positional)) = _parse(argv, &mut stdout)? else {
        return Ok(ExitCode::SUCCESS);
    };
    let explicit_profile = opts
        .logger
        .as_deref()
        .filter(|logger| !logger.is_empty())
        .map(profiles::get)
        .transpose()?;
    let default_profile = explicit_profile.unwrap_or(profiles::get("cr6")?);
    let version = (opts.version || opts.version_only).then(|| {
        format!(
            "crbc {SPEC_VERSION} VERSION:{}  DATE:{}",
            default_profile.version, default_profile.date
        )
    });
    if let Some(line) = &version {
        writeln!(stdout, "{line}")?;
    }
    if opts.version_only {
        return Ok(ExitCode::SUCCESS);
    }
    if positional.len() > 2 {
        return Ok(ExitCode::FAILURE);
    }

    let program = positional.first().map(Path::new);
    let profile =
        explicit_profile.unwrap_or_else(|| program.map_or(default_profile, profiles::for_file));
    let default_message = OsString::from(format!("{}cmple.xxx", profile.name.to_ascii_uppercase()));
    let message_path = Path::new(positional.get(1).unwrap_or(&default_message));

    let (ok, lines, version) = if let Some(program) = program.filter(|path| path.is_file()) {
        let result = compiler::compile_file(
            program,
            Some(profile),
            None,
            false,
            None,
            true,
            false,
            false,
        );
        (result.ok(), result.result_lines(false), version.as_deref())
    } else {
        let name = positional
            .first()
            .map(|arg| arg.to_string_lossy())
            .unwrap_or_default();
        (
            false,
            vec![
                format!("{name} -- Compile Failed!"),
                String::new(),
                "No Program File".to_owned(),
            ],
            None,
        )
    };
    stdout.write_all(&pystr::encode_latin1_replace(&format!(
        "{}\n",
        lines.join("\n")
    )))?;
    if opts.message_file {
        _write_message_file(message_path, &lines, version)?;
    }
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
