mod time;


use std::borrow::Cow;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use crate::time::*;


fn main()
{
	const ARG_MIN: usize = 2;
	const ARG_MAX: usize = 4;
	let args = std::env::args_os();
	let arg_count = args.len();
	if arg_count < ARG_MIN || arg_count > ARG_MAX {
		print_help();
		std::process::exit(1);
	}

	let mut file = PathBuf::new();
	let mut vtt_file = true;
	let mut begin_diff = Time::default();
	let mut end_diff = Time::default();

	for (i, arg_os) in args.skip(1).enumerate() {
		let i = i + 1;
		match (i, arg_os.as_encoded_bytes()) {
			(_, b"-h" | b"--help") => {
				print_help();
				return;
			}
			(_, b"-v" | b"--version") => {
				print_version();
				return;
			}
			(1, arg) => {
				if arg.ends_with(b".srt") {
					file = PathBuf::from(arg_os);
					vtt_file = false;
				}
				else if arg.ends_with(b".vtt") {
					file = PathBuf::from(arg_os);
				}
				else {
					fail(format!("expected .srt or .vtt file, but got \"{}\"", arg_str(arg, i)));
				}
			},
			(2, arg) => match Time::parse_seconds(arg) {
				Some(time) => begin_diff = time,
				None => match arg_count {
					3 => fail(format!("failed to parse seconds difference \"{}\"", arg_str(arg, i))),
					_ => fail(format!("failed to parse begin seconds difference \"{}\"", arg_str(arg, i))),
				},
			},
			(3, arg) => match Time::parse_seconds(arg) {
				Some(time) => end_diff = time,
				None => fail(format!("failed to parse end seconds difference \"{}\"", arg_str(arg, i))),
			},
			_ => (),
		}
	}

	if arg_count == 3 {
		end_diff = begin_diff;
	}

	edit_file(&file, begin_diff, end_diff, vtt_file);
}


/// Argument to string
fn arg_str(arg: &[u8], arg_i: usize) -> Cow<str>
{
	return match core::str::from_utf8(arg) {
		Ok(arg) => Cow::Borrowed(arg),
		Err(_) => Cow::Owned(format!("(argument {arg_i})")),
	};
}


/// Edit a Web Video Text Tracks file
fn edit_file(input_path: &Path, begin_diff: Time, end_diff: Time, header: bool)
{
	#[derive(Clone, Copy, Debug)]
	enum Expecting
	{
		FileHeader,
		CueNumberOrTimes,
		CueNumberTimesOrEmpty,
		CueText,
		CueTextOrEmpty,
	}
	use Expecting::*;

	// Open the input file or fail
	let input = match OpenOptions::new().read(true).open(input_path) {
		Ok(input) => input,
		Err(error) => fail(format!("failed to open file to read: {}", error.to_string())),
	};

	// Create a temporary output file or fail
	let mut tmp_path = PathBuf::from(input_path);
	tmp_path.set_extension("vtt.tmp");
	let mut output = match OpenOptions::new().create(true).truncate(true).write(true).open(&tmp_path) {
		Ok(output) => output,
		Err(error) => fail(format!("failed to create temporary file to write: {}", error.to_string())),
	};
	let output = &mut output;

	// Write the header or fail
	write_or_fail(output, "WEBVTT\n");

	// Copy each subtitle or fail
	let reader = BufReader::new(input);
	let mut expecting = if header { FileHeader } else { CueNumberTimesOrEmpty };
	let mut line_i: usize = 0;
	for line in reader.lines() {
		line_i += 1;
		let line = match line {
			Ok(line) => line,
			Err(error) => fail(format!("failed to read line {line_i}: {}", error.to_string())),
		};
		let line = line.as_str();
		match (expecting, line.is_empty()) {
			(FileHeader, true) => expecting = CueNumberTimesOrEmpty,
			(FileHeader, false) => match Time::parse_times(line.as_bytes()) {
				Some(_) => fail(format!("expected WEBVTT and blank line before times at line {line_i}")),
				None => (),
			},
			(CueNumberOrTimes, true) => fail(format!("expected times then text, but got empty line at {line_i}")),
			(CueNumberTimesOrEmpty, true) => (),
			(CueNumberOrTimes | CueNumberTimesOrEmpty, false) => {
				let (begin, end) = match Time::parse_times(line.as_bytes()) {
					Some((begin, end)) => (begin + begin_diff, end + end_diff),
					None => {
						expecting = CueNumberOrTimes;
						continue;
					},
				};
				if begin.is_negative() {
					fail(String::from("begin seconds difference is too negative"));
				}
				else if end.is_negative() {
					fail(String::from("end seconds difference is too negative"));
				}
				match write!(output, "\n{begin} --> {end}\n") {
					Ok(()) => (),
					Err(error) => fail(format!("failed to write to temporary file: {}", error.to_string())),
				}
				expecting = CueText;
			},
			(CueText, true) => fail(format!("expected text at line {line_i}")),
			(CueText | CueTextOrEmpty, false) => match Time::parse_times(line.as_bytes()) {
				Some(_) => fail(format!("expected a blank line for times at line {line_i}")),
				None => {
					expecting = CueTextOrEmpty;
					write_line_or_fail(output, line);
				},
			},
			(CueTextOrEmpty, true) => expecting = CueNumberTimesOrEmpty,
		}
	}

	// Remove the old file
	match std::fs::remove_file(input_path) {
		Ok(()) => (),
		Err(error) => fail(format!("failed to remove old file: {}", error.to_string())),
	}

	// Rename the temporary file
	let mut output_path = PathBuf::from(input_path);
	output_path.set_extension("vtt");
	match std::fs::rename(tmp_path, output_path) {
		Ok(()) => (),
		Err(error) => fail(format!("failed to rename temporary file {}", error.to_string())),
	}
}


fn fail(error: String) -> !
{
	let _ = write!(std::io::stderr(), concat!(env!("CARGO_PKG_NAME"), ": {}\n"), error);
	std::process::exit(1);
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


fn write_line_or_fail(file: &mut File, content: &str)
{
	match write!(file, "{content}\n") {
		Ok(()) => (),
		Err(error) => fail(format!("failed to write to temporary file: {}", error.to_string())),
	}
}


fn write_or_fail(file: &mut File, content: &str)
{
	match file.write_all(content.as_bytes()) {
		Ok(()) => (),
		Err(error) => fail(format!("failed to write to temporary file: {}", error.to_string())),
	}
}
