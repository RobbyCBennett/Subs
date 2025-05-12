use core::fmt::Display;
use core::ops::Add;
use core::ops::AddAssign;
use core::ops::Sub;
use core::ops::SubAssign;


const MS_PER_SEC: i32 = 1000;
const SEC_PER_MIN: i32 = 60;
const MIN_PER_HR: i32 = 60;

const MS_PER_HR: i32 = MS_PER_SEC * SEC_PER_MIN * MIN_PER_HR;
const MS_PER_MIN: i32 = MS_PER_SEC * SEC_PER_MIN;
const SEC_PER_HR: i32 = SEC_PER_MIN * MIN_PER_HR;


/// A timestamp for a subtitle cue
#[derive(Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Time
{
	milliseconds: i32,
}


#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct TimeParts
{
	hours: i16,
	minutes: i8,
	seconds: i8,
	milliseconds: i16,
}


impl Time
{
	/// Parse from some seconds
	/// * Regular expression: `^[+-]?(\.\d+|\d+(\.\d+)?)$`
	pub fn parse_seconds(input: &[u8]) -> Option<Time>
	{
		#[derive(Debug)]
		enum Expecting
		{
			SignSecondsOrDot,
			SecondsOrDot,
			SecondsDotOrEnd,
			Milliseconds1,
			Milliseconds2OrEnd,
			Milliseconds3OrEnd,
			End,
		}
		use Expecting::*;

		let mut expecting = SignSecondsOrDot;
		let mut sign = 1;
		let mut seconds = &input[0..0];
		let mut milliseconds = 0;

		for (i, byte) in input.iter().enumerate() {
			match (expecting, byte) {
				(SignSecondsOrDot, b'-') => {
					sign = -1;
					expecting = SecondsOrDot;
				},
				(SignSecondsOrDot, b'+') => expecting = SecondsOrDot,
				(SignSecondsOrDot, b'.') => expecting = Milliseconds1,
				(SignSecondsOrDot | SecondsOrDot | SecondsDotOrEnd, b'0' ..= b'9') => {
					seconds = &input[0..i+1];
					expecting = SecondsDotOrEnd;
				},
				(SecondsOrDot | SecondsDotOrEnd, b'.') => {
					seconds = &input[0..i];
					expecting = Milliseconds1;
				},
				(Milliseconds1, b'0' ..= b'9') => {
					milliseconds = 100 * (byte - b'0') as i32;
					expecting = Milliseconds2OrEnd;
				},
				(Milliseconds2OrEnd, b'0' ..= b'9') => {
					milliseconds += 10 * (byte - b'0') as i32;
					expecting = Milliseconds3OrEnd;
				},
				(Milliseconds3OrEnd, b'0' ..= b'9') => {
					milliseconds += (byte - b'0') as i32;
					expecting = End;
				},
				(End, _) => return None,
				_ => return None,
			}
		}

		match expecting {
			SecondsDotOrEnd | Milliseconds2OrEnd | Milliseconds3OrEnd | End => (),
			_ => return None,
		}

		let seconds = match core::str::from_utf8(seconds) {
			Ok(seconds) => seconds,
			Err(_) => "",
		};

		milliseconds *= sign;

		milliseconds += match i32::from_str_radix(seconds, 10) {
			Ok(seconds) => seconds * 1000,
			Err(_) => 0,
		};

		return Some(Time { milliseconds });
	}


	/// Parse from some bytes, updating the slice of bytes
	/// * Regular expression: `^(\d\d:)?\d\d:\d\d[,.]\d\d\d$`
	fn parse_time(input: &mut &[u8]) -> Option<Time>
	{
		#[derive(Debug)]
		enum Expecting
		{
			HourOrMinute1,
			HourOrMinute2,
			Colon1,
			MinuteOrSecond1,
			MinuteOrSecond2,
			Colon2OrCommaOrDecimal,
			Second1,
			Second2,
			CommaOrDecimal,
			Millisecond1,
			Millisecond2,
			Millisecond3,
		}
		use Expecting::*;

		let mut expecting = HourOrMinute1;

		let mut hours_or_minutes: i16 = 0;
		let mut minutes_or_seconds: i8 = 0;
		let mut hours: i16 = 0;
		let mut minutes: i8 = 0;
		let mut seconds: i8 = 0;
		let mut milliseconds: i16 = 0;
		let mut byte_count = 0;

		for byte in *input {
			byte_count += 1;
			match (expecting, byte) {
				(HourOrMinute1, b'0' ..= b'9') => {
					hours_or_minutes = 10 * (byte - b'0') as i16;
					expecting = HourOrMinute2;
				},
				(HourOrMinute2, b'0' ..= b'9') => {
					hours_or_minutes += (byte - b'0') as i16;
					expecting = Colon1
				},
				(Colon1, b':') => expecting = MinuteOrSecond1,
				(MinuteOrSecond1, b'0' ..= b'9') => {
					minutes_or_seconds = 10 * (byte - b'0') as i8;
					expecting = MinuteOrSecond2;
				},
				(MinuteOrSecond2, b'0' ..= b'9') => {
					minutes_or_seconds += (byte - b'0') as i8;
					expecting = Colon2OrCommaOrDecimal;
				},
				(Colon2OrCommaOrDecimal, b':') => {
					hours = hours_or_minutes;
					minutes = minutes_or_seconds;
					expecting = Second1;
				},
				(Colon2OrCommaOrDecimal, b',' | b'.') => {
					minutes = hours_or_minutes as i8;
					seconds = minutes_or_seconds;
					expecting = Millisecond1;
				},
				(Second1, b'0' ..= b'9') => {
					seconds = 10 * (byte - b'0') as i8;
					expecting = Second2;
				},
				(Second2, b'0' ..= b'9') => {
					seconds += (byte - b'0') as i8;
					expecting = CommaOrDecimal;
				},
				(CommaOrDecimal, b',' | b'.') => expecting = Millisecond1,
				(Millisecond1, b'0' ..= b'9') => {
					milliseconds = 100 * (byte - b'0') as i16;
					expecting = Millisecond2;
				},
				(Millisecond2, b'0' ..= b'9') => {
					milliseconds += 10 * (byte - b'0') as i16;
					expecting = Millisecond3;
				},
				(Millisecond3, b'0' ..= b'9') => {
					milliseconds += (byte - b'0') as i16;
					break;
				},
				_ => return None,
			}
		}

		*input = &input[byte_count..];

		let parts = TimeParts { hours, minutes, seconds, milliseconds };

		return Some(Time::from(parts));
	}


	/// Parse from some bytes, updating the slice of bytes
	/// * Regular expression: `^(\d\d:)?\d\d:\d\d[,.]\d\d\d --> (\d\d:)?\d\d:\d\d[,.]\d\d\d$`
	pub fn parse_times(input: &[u8]) -> Option<(Time, Time)>
	{
		let mut input = input;

		let begin = match Time::parse_time(&mut input) {
			Some(begin) => begin,
			None => return None,
		};

		const ARROW: &str = " --> ";
		match input.starts_with(ARROW.as_bytes()) {
			true  => input = &input[ARROW.len()..],
			false => return None,
		}

		let end = match Time::parse_time(&mut input) {
			Some(begin) => begin,
			None => return None,
		};

		return Some((begin, end));
	}


	pub fn is_negative(&self) -> bool
	{
		return self.milliseconds < 0;
	}


	fn parts(&self) -> TimeParts
	{
		let hours = self.milliseconds / MS_PER_HR;
		let minutes = self.milliseconds / MS_PER_MIN - hours * MIN_PER_HR;
		let seconds = self.milliseconds / MS_PER_SEC - minutes * SEC_PER_MIN - hours * SEC_PER_HR;
		let milliseconds = self.milliseconds - seconds * MS_PER_SEC - minutes * MS_PER_MIN - hours * MS_PER_HR;

		let hours = hours as i16;
		let minutes = minutes as i8;
		let seconds = seconds as i8;
		let milliseconds = milliseconds as i16;

		return TimeParts { hours, minutes, seconds, milliseconds };
	}
}


impl From<TimeParts> for Time
{
	fn from(parts: TimeParts) -> Time
	{
		let mut milliseconds = 0;
		milliseconds += parts.milliseconds as i32;
		milliseconds += parts.seconds as i32 * MS_PER_SEC;
		milliseconds += parts.minutes as i32 * MS_PER_MIN;
		milliseconds += parts.hours as i32 * MS_PER_HR;
		return Time { milliseconds: milliseconds };
	}
}


impl Add for Time
{
	type Output = Time;

	fn add(self, other: Time) -> Time
	{
		let milliseconds = self.milliseconds + other.milliseconds;
		return Time { milliseconds };
	}
}


impl Add for &Time
{
	type Output = Time;

	fn add(self, other: &Time) -> Time
	{
		let milliseconds = self.milliseconds + other.milliseconds;
		return Time { milliseconds };
	}
}


impl AddAssign for Time
{
	fn add_assign(&mut self, other: Time)
	{
		self.milliseconds += other.milliseconds;
	}
}


impl Sub for Time
{
	type Output = Time;

	fn sub(self, other: Time) -> Time
	{
		let milliseconds = self.milliseconds - other.milliseconds;
		return Time { milliseconds };
	}
}


impl Sub for &Time
{
	type Output = Time;

	fn sub(self, other: &Time) -> Time
	{
		let milliseconds = self.milliseconds - other.milliseconds;
		return Time { milliseconds };
	}
}


impl SubAssign for Time
{
	fn sub_assign(&mut self, other: Time)
	{
		self.milliseconds -= other.milliseconds;
	}
}


impl Display for Time
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
	{
		let parts = self.parts();
		return match parts.hours {
			0 => write!(f, "{:0>2}:{:0>2}.{:0>3}", parts.minutes, parts.seconds, parts.milliseconds),
			hours => write!(f, "{:0>2}:{:0>2}:{:0>2}.{:0>3}", hours, parts.minutes, parts.seconds, parts.milliseconds),
		};
	}
}


#[test]
#[cfg(test)]
fn test()
{
	let tests = [
		i32::MIN,
		i32::MIN / 4 * 3,
		i32::MIN / 2,
		i32::MIN / 4,
		0,
		i32::MAX / 4,
		i32::MAX / 2,
		i32::MAX / 4 * 3,
		i32::MAX,
	];

	for milliseconds in tests {
		let time = Time { milliseconds };
		let parts = time.parts();
		assert_eq!(parts.hours, (milliseconds / 3600000) as i16);
		assert_eq!(parts.minutes, (milliseconds / 60000 % 60) as i8);
		assert_eq!(parts.seconds, (milliseconds / 1000 % 60) as i8);
		assert_eq!(parts.milliseconds, (milliseconds % 1000) as i16);
	}
}
