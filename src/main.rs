use glob::{glob_with, MatchOptions};
use clap::{Arg, App};
use snafu::{ensure, ResultExt, Snafu};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

fn main() {
  let matches = App::new("Datier")
    .version("1.0.0")
    .author("Tilman Schmidt")
    .about("Renames JPEGs and related images based on the date they were taken")
    .arg(Arg::with_name("input directory")
      .required(true)
      .help("The folder in which to rename images")
      .index(1)) // first positional argument

    .arg(Arg::with_name("log")
      .short("l")
      .help("Log each file inspected by datier, and the action taken on it."))

    .arg(Arg::with_name("dry-run")
      .long("dry-run")
      .help("Don't perform any actual renaming."))

    .arg(Arg::with_name("deep")
      .short("d")
      .long("deep")
      .help("Also search sub-directories for files, and move them into the working directory."))

    .get_matches();

  let l = Logger::new(matches.is_present("log"));
  let dry_run = matches.is_present("dry-run");
  let deep = matches.is_present("deep");

  let input_dir_str = String::from(matches.value_of("input directory").unwrap());
  let input_dir = Path::new(&input_dir_str);
  if !input_dir.is_dir() {
    l.error(format_args!("Input path {} is not a directory!", input_dir.display()));
    return;
  }

  let extensions = ["jpg", "jpeg", "cr2"];
  let patterns = extensions.iter().map(|ext| {
    let mut pattern = input_dir_str.clone();
    if deep {
      pattern.extend("/**".chars());
    }
    pattern.extend("/*.".chars());
    pattern.extend(ext.chars());
    pattern
  });

  let all_paths = patterns
    .filter_map(|pattern| { // make a glob iterator for each pattern
      let mut options = MatchOptions::new();
      options.case_sensitive = false;
      match glob_with(&pattern, options) {
        Ok(paths) => Some(paths),
        Err(error) => {
          l.error(format_args!("Could not read glob pattern {}: {}", pattern, error));
          None
        }
      }
    }).flatten() // combine all iterators into a single iteratore over all matching items
    .filter_map(|glob_result| glob_result.ok()); // unwrap, and filter out any matched items that still errored

  let mut invalid_entries: Vec<(PathBuf, GetDateTimeError)> = Vec::new();
  let mut valid_entries: Vec<(PathBuf, OrdDateTime)> = Vec::new();

  for path in all_paths {
    let datetime_res = get_datetime(&path);
    match datetime_res {
      Err(error) => invalid_entries.push((path, error)),
      Ok(datetime) => valid_entries.push((path, datetime.into())),
    }
  }

  for (path, error) in invalid_entries {
    l.log(format_args!("{} skipped ({})", path.display(), error));
  }

  if valid_entries.len() == 0 {
    return;
  }

  valid_entries.sort_unstable_by(|a, b| a.1.cmp(&b.1));

  let mut img_number = 1;
  let mut prev_datetime = &valid_entries[0].1;
  for (ref path, ref datetime) in &valid_entries {
    if !datetime.date_eq(prev_datetime) {
      img_number = 1;
    }

    if datetime != prev_datetime {
      if datetime.date_eq(prev_datetime) {
        img_number += 1;
      } else {
        img_number = 1;
      }
    }

    let new_stem = format!("{}_{:02}_{:02}-{:04}", datetime.0.year, datetime.0.month, datetime.0.day, img_number);
    
    prev_datetime = &datetime;

    if let Some(ext) = path.extension() {
      let new_filename = format!("{}.{}", new_stem, ext.to_string_lossy());
      let mut rename_dest = input_dir.to_path_buf();
      rename_dest.push(new_filename);
      if !rename_dest.exists() {
        let rename_action = if !dry_run {
          let rename_res = fs::rename(&path, &rename_dest);
          match rename_res {
            Ok(()) => true,
            Err(error) => {
              l.log(format_args!("{} skipped (Rename failed: {})", path.display(), error));
              false
            }
          }
        } else {
          true
        };

        if rename_action {
          l.log(format_args!("{} -> {}", path.display(), rename_dest.display()));
        }
      } else {
        l.log(format_args!("{} skipped (Would rename, but {} already exists", path.display(), rename_dest.display()));
      }
    } else {
      l.log(format_args!("{} skipped (Has no extension)", path.display()));
    }
  }
}

struct Logger {
  print_logs: bool
}

impl Logger {
  fn new(print_logs: bool)->Logger {
    Logger {
      print_logs
    }
  }

  fn log(&self, args: std::fmt::Arguments) {
    if self.print_logs {
      println!("{}", args);
    }
  }

  fn error(&self, args: std::fmt::Arguments) {
    println!("Error: {}", args);
  }
}

#[derive(Debug, Snafu)]
enum GetDateTimeError {
  #[snafu(display("Could not open file: {}", source))]
  FileOpenError {
    source: std::io::Error,
  },
  #[snafu(display("Could not create exif reader: {}", source))]
  ReaderCreateError {
    source: exif::Error,
  },
  #[snafu(display("Could not read DateTime field: {}", source))]
  FieldReadError {
    source: DateTimeReadError
  },
}

fn get_datetime<P: AsRef<Path>>(path: P)->Result<exif::DateTime, GetDateTimeError> {
  let file = File::open(path).context(FileOpenError)?;
  let reader = exif::Reader::new(&mut std::io::BufReader::new(&file)).context(ReaderCreateError)?;

  let datetime = read_datetime(&reader).context(FieldReadError)?;
  Ok(datetime)
}

#[derive(Debug, Snafu)]
enum DateTimeReadError {
  #[snafu(display("DateTime field is missing."))]
  FieldMissing,
  #[snafu(display("DateTime field is not in ascii format."))]
  FieldNotAscii,
  #[snafu(display("DateTime field contains no data."))]
  FieldEmpty,
  #[snafu(display("DateTime field data could not be parsed: {}", source))]
  ParseError {
    source: exif::Error,
  },
}

fn read_datetime(exif_reader: &exif::Reader)->Result<exif::DateTime, DateTimeReadError> {
  let date_time_field = exif_reader.get_field(exif::Tag::DateTime, false);

  ensure!(date_time_field.is_some(), FieldMissing);
  let date_time_data = date_time_field.unwrap();

  let mut date_time = 
    if let exif::Value::Ascii(ref datetime_ascii) = date_time_data.value {
      let datetime_string = datetime_ascii.first();
      
      ensure!(datetime_string.is_some(), FieldEmpty);
      let datetime_string = datetime_string.unwrap();

      exif::DateTime::from_ascii(datetime_string).context(ParseError)?
    } else {
      return FieldNotAscii.fail();
    };

  if let Some(subsec_data) = exif_reader.get_field(exif::Tag::SubSecTime, false) {
    if let exif::Value::Ascii(ref subsec_ascii) = subsec_data.value {
      if let Some(subsec_string) = subsec_ascii.first() {
        let _ = date_time.parse_subsec(subsec_string); // ignore any parse error
      }
    }
  }

  Ok(date_time)
}

  // wrapper type around DateTime that adds ordering based on the time
  // note that for ordering and equality, the offset value is ignored
#[derive(Debug)]
struct OrdDateTime(exif::DateTime);

impl From<exif::DateTime> for OrdDateTime {
  fn from(datetime: exif::DateTime)->Self {
    Self(datetime)
  }
}

impl std::fmt::Display for OrdDateTime {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>)->std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

use std::cmp::Ordering;
impl Ord for OrdDateTime {
    // Note that this ignores the offset field
  fn cmp(&self, other: &Self)->Ordering {
            self.0.year.cmp(&other.0.year)
      .then(self.0.month.cmp(&other.0.month))
      .then(self.0.day.cmp(&other.0.day))
      .then(self.0.hour.cmp(&other.0.hour))
      .then(self.0.minute.cmp(&other.0.minute))
      .then(self.0.second.cmp(&other.0.second))
      .then(self.0.nanosecond.cmp(&other.0.nanosecond))
  }
}

impl PartialOrd for OrdDateTime {
  fn partial_cmp(&self, other: &Self)->Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl PartialEq for OrdDateTime {
    // Note that this ignores the offset field
  fn eq(&self, other: &Self)->bool {
    self.0.year       == other.0.year       &&
    self.0.month      == other.0.month      &&
    self.0.day        == other.0.day        &&
    self.0.hour       == other.0.hour       &&
    self.0.minute     == other.0.minute     &&
    self.0.second     == other.0.second     &&
    self.0.nanosecond == other.0.nanosecond
  }
}

impl Eq for OrdDateTime {}

impl OrdDateTime {
  fn date_eq(&self, other: &OrdDateTime)->bool {
    self.0.year == other.0.year && self.0.month == other.0.month && self.0.day == other.0.day
  }
}