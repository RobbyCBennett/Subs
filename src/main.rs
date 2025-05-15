mod file_remover;
mod time;


use std::borrow::Cow;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use file_remover::FileRemover;
use time::*;


fn main() -> ExitCode
{
	const ARG_MIN: usize = 2;
	const ARG_MAX: usize = 4;
	let args = std::env::args_os();
	let arg_count = args.len();
	if arg_count < ARG_MIN || arg_count > ARG_MAX {
		print_help();
		return ExitCode::FAILURE;
	}

	let mut input_path = PathBuf::new();
	let mut vtt_input = true;
	let mut begin_diff = Time::default();
	let mut end_diff = Time::default();

	for (i, arg_os) in args.skip(1).enumerate() {
		let i = i + 1;
		match (i, arg_os.as_encoded_bytes()) {
			(_, b"-h" | b"--help") => {
				print_help();
				return ExitCode::SUCCESS;
			}
			(_, b"-v" | b"--version") => {
				print_version();
				return ExitCode::SUCCESS;
			}
			(1, arg) => {
				if arg.ends_with(b".srt") {
					input_path = PathBuf::from(arg_os);
					vtt_input = false;
				}
				else if arg.ends_with(b".vtt") {
					input_path = PathBuf::from(arg_os);
				}
				else {
					return fail(format!("expected .srt or .vtt input_path, but got \"{}\"", arg_str(arg, i)));
				}
			},
			(2, arg) => match Time::parse_seconds(arg) {
				Some(time) => begin_diff = time,
				None => match arg_count {
					3 => return fail(format!("failed to parse seconds difference \"{}\"", arg_str(arg, i))),
					_ => return fail(format!("failed to parse begin seconds difference \"{}\"", arg_str(arg, i))),
				},
			},
			(3, arg) => match Time::parse_seconds(arg) {
				Some(time) => end_diff = time,
				None => return fail(format!("failed to parse end seconds difference \"{}\"", arg_str(arg, i))),
			},
			_ => (),
		}
	}

	// Apply the begin time argument to the end time
	if arg_count == 3 {
		end_diff = begin_diff;
	}

	// Copy the input file to a temporary file and open it
	let backup_path = std::env::temp_dir().join(format!("subs_{}", std::process::id()));
	match std::fs::copy(&input_path, &backup_path) {
		Ok(_) => (),
		Err(error) => return fail(match error.kind() {
			 ErrorKind::NotFound => format!("file not found"),
			 _ => format!("failed to backup input file: {}", error.to_string()),
		}),
	}
	let _file_remover = FileRemover::new(&backup_path);
	let input = match OpenOptions::new().read(true).open(&backup_path) {
		Ok(input) => input,
		Err(error) => return fail(format!("failed to open backup input file to read: {}", error.to_string())),
	};

	// Open the output file
	let output_path = match vtt_input {
		true => input_path.clone(),
		false => {
			let mut output_path = PathBuf::from(&input_path);
			output_path.set_extension("vtt");
			match std::fs::rename(&input_path, &output_path) {
				Ok(()) => (),
				Err(error) => return fail(format!("failed to rename file from .srt to .vtt: {}", error.to_string())),
			}
			output_path
		},
	};
	let output = match OpenOptions::new().create(true).truncate(true).write(true).open(&output_path) {
		Ok(input) => input,
		Err(error) => {
			return fail(format!("failed to open file to write: {}", error.to_string()));
		},
	};

	// Edit the file or restore it on failure
	return match edit_file(input, output, begin_diff, end_diff) {
		ExitCode::SUCCESS => ExitCode::SUCCESS,
		_ => copy_file_content(&backup_path, &input_path),
	};
}


/// Argument to string
fn arg_str(arg: &[u8], arg_i: usize) -> Cow<str>
{
	return match core::str::from_utf8(arg) {
		Ok(arg) => Cow::Borrowed(arg),
		Err(_) => Cow::Owned(format!("(argument {arg_i})")),
	};
}


/// Copy the content and leave the permissions alone
fn copy_file_content(input: &Path, output: &Path) -> ExitCode
{
	let input = match OpenOptions::new().read(true).open(input) {
		Ok(file) => file,
		Err(error) => return fail(format!("failed to restore input file: {error}")),
	};
	let output = match OpenOptions::new().create(true).truncate(true).write(true).open(output) {
		Ok(file) => file,
		Err(error) => return fail(format!("failed to restore input file: {error}")),
	};

	let mut reader = BufReader::new(input);
	let mut writer = BufWriter::new(output);

	return match std::io::copy(&mut reader, &mut writer) {
		Ok(_) => ExitCode::SUCCESS,
		Err(error) => fail(format!("failed to restore input file: {}", error.to_string())),
	};
}


/// Edit a Web Video Text Tracks file
fn edit_file(input: File, mut output: File, begin_diff: Time, end_diff: Time) -> ExitCode
{
	#[derive(Clone, Copy, Debug)]
	enum Expecting
	{
		FileHeaderCueNumberTimesOrEmpty,
		CueNumberOrTimes,
		CueNumberTimesOrEmpty,
		CueText,
		CueTextOrEmpty,
	}
	use Expecting::*;

	// Write the header or fail
	match output.write_all("WEBVTT\n".as_bytes()) {
		Ok(()) => (),
		Err(error) => return fail(format!("failed to write to temporary file: {}", error.to_string())),
	}

	// Copy each subtitle or fail
	let reader = BufReader::new(input);
	let mut expecting = FileHeaderCueNumberTimesOrEmpty;
	let mut line_i: usize = 0;
	for line in reader.lines() {
		line_i += 1;
		let line = match line {
			Ok(line) => line,
			Err(error) => return fail(format!("failed to read line {line_i}: {}", error.to_string())),
		};
		let line = line.as_str();
		match (expecting, line.is_empty()) {
			(FileHeaderCueNumberTimesOrEmpty, true) => (),
			(CueNumberOrTimes, true) => return fail(format!("expected times then text, but got empty line at {line_i}")),
			(CueNumberTimesOrEmpty, true) => (),
			(FileHeaderCueNumberTimesOrEmpty | CueNumberOrTimes | CueNumberTimesOrEmpty, false) => {
				let (begin, end) = match Time::parse_times(line.as_bytes()) {
					Some((begin, end)) => (begin + begin_diff, end + end_diff),
					None => {
						expecting = match line == "WEBVTT" {
							true  => FileHeaderCueNumberTimesOrEmpty,
							false => CueNumberOrTimes,
						};
						continue;
					},
				};
				if begin.is_negative() {
					return fail(String::from("begin seconds difference is too negative"));
				}
				else if end.is_negative() {
					return fail(String::from("end seconds difference is too negative"));
				}
				match write!(&mut output, "\n{begin} --> {end}\n") {
					Ok(()) => (),
					Err(error) => return fail(format!("failed to write to temporary file: {}", error.to_string())),
				}
				expecting = CueText;
			},
			(CueText, true) => return fail(format!("expected text at line {line_i}")),
			(CueText | CueTextOrEmpty, false) => match Time::parse_times(line.as_bytes()) {
				Some(_) => return fail(format!("expected a blank line for times at line {line_i}")),
				None => {
					expecting = CueTextOrEmpty;
					match write!(&mut output, "{line}\n") {
						Ok(()) => (),
						Err(error) => return fail(format!("failed to write to temporary file: {}", error.to_string())),
					}
				},
			},
			(CueTextOrEmpty, true) => expecting = CueNumberTimesOrEmpty,
		}
	}

	return ExitCode::SUCCESS;
}


fn fail(error: String) -> ExitCode
{
	let _ = write!(std::io::stderr(), concat!(env!("CARGO_PKG_NAME"), ": {}\n"), error);
	return ExitCode::FAILURE;
}


fn print(string: &str)
{
	let _ = std::io::stdout().write_all(string.as_bytes());
}


fn print_help()
{
	print(concat!(
		env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), "\n",
		"  ", env!("CARGO_PKG_DESCRIPTION"), "\n",
		"\n",
		"usage:\n",
		"  ", env!("CARGO_PKG_NAME"), " FILE\n",
		"  ", env!("CARGO_PKG_NAME"), " FILE BOTH_SEC_DIFF\n",
		"  ", env!("CARGO_PKG_NAME"), " FILE BEGIN_SEC_DIFF END_SEC_DIFF\n",
		"\n",
		"examples:\n",
		"  ", env!("CARGO_PKG_NAME"), " Alien.srt\n",
		"  ", env!("CARGO_PKG_NAME"), " Alien.vtt +1\n",
		"  ", env!("CARGO_PKG_NAME"), " Alien.srt .5 -0.25\n",
	));
}


fn print_version()
{
	print(concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), "\n"));
}
