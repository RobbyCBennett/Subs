# Subs

*Adjust subtitle times and convert .srt to .vtt*

* Reads .srt and .vtt files
* Converts to .vtt file
* Reduces file size
* Finds errors
* Optionally changes subtitle cue begin and end times


## Build

1. Install [git](https://git-scm.com/book/en/v2/Getting-Started-Installing-Git) and [rustup](https://rustup.rs)

2. Clone and build
	```
	git clone https://github.com/RobbyCBennett/Subs.git subs
	cd subs
	cargo build --release
	```

3. You have built `target/release/subs` or `target\release\subs.exe`

## User Interface

### Usage
```
subs FILE
subs FILE BOTH_SEC_DIFF
subs FILE BEGIN_SEC_DIFF END_SEC_DIFF
```

### Examples
```
subs Alien.srt
subs Alien.vtt +1
subs Alien.srt .5 -0.25
```
