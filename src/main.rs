extern crate clap;
extern crate ioe;
extern crate walkdir;
extern crate zip;

#[macro_use] mod log;
mod result;

use clap::{Arg, ArgMatches, App, SubCommand};
use std::env;
use std::io::{self, Read, Seek, Write};
use std::fs::{self, File};
use std::iter::Iterator;
use std::path::{Path, PathBuf};
use std::process;
use walkdir::{WalkDir, DirEntry};
use zip::{
    CompressionMethod, ZipArchive, ZipWriter,
    read::ZipFile,
    write::FileOptions,
};
use crate::result::ZippyResult;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ZippyResult<()> {
    let matches: ArgMatches = App::new(APP_NAME)
        .version(APP_VERSION)
        .author("Joey Ezechiels")
        .about("A CLI-based, `Cargo install`able way to un/zip files & directories.")
        .arg(Arg::with_name("v")
             .short("v")
             .multiple(true)
             .help("Sets the level of verbosity"))
        .subcommand(SubCommand::with_name("unzip")
                    .about("uncompress a zip file.")
                    .arg(Arg::with_name("input")
                         .required(true)
                         .takes_value(true)
                         .short("i")
                         .long("input")
                         .help("A zip file to unzip."))
                    .arg(Arg::with_name("output")
                         .required(true)
                         .takes_value(true)
                         .short("o")
                         .long("output")
                         .help("The destination of the zip file's uncompressed contents.")))
        .subcommand(SubCommand::with_name("zip")
                    .about("compress files and directories into a zip file.")
                    .arg(Arg::with_name("input")
                         .multiple(true)
                         .required(true)
                         .takes_value(true)
                         .short("i")
                         .long("input")
                         .help("A list of input files and directories to zip."))
                    .arg(Arg::with_name("output")
                         .required(true)
                         .takes_value(true)
                         .short("o")
                         .long("output")
                         .help("The destination zip file.")))
        .get_matches();

    // Record how many times the user used the "verbose" flag
    // (e.g. 'myprog -v -v -v' or 'myprog -vvv' vs 'myprog -v')
    let num_verbose_flags: u64 = matches.occurrences_of("v");
    let verbose_flag = "v".repeat(num_verbose_flags as usize);
    match num_verbose_flags {
        0 => {/* silent mode */},
        1 => log!("-{}: verbose mode", verbose_flag),
        2 => log!("-{}: very verbose mode", verbose_flag),
        _ => {
            log!("-{}: why do you even want so much information?", verbose_flag);
            process::exit(-1);
        },
    }

    // TODO: The `method` should be an argument to the `zip` subcmd:
    let compression_method = CompressionMethod::Stored;

    if let Some(zip_matches) = matches.subcommand_matches("zip") {
        if let Some(output_path) = zip_matches.value_of("output") {
            let output_path: &Path = Path::new(output_path);
            log!("creating zip file @ {}", output_path.display());
            let mut zippy = Zippy::new();
            if let Some(input_paths) = zip_matches.values_of("input") {
                zippy.zip(
                    input_paths.into_iter().map(Path::new),
                    output_path,
                    compression_method
                )?;
            }
        }
    }

    if let Some(unzip_matches) = matches.subcommand_matches("unzip") {
        let unzip_matches: &ArgMatches = unzip_matches;
        if let Some(output_dirpath) = unzip_matches.value_of("output") {
            let output_dirpath: &Path = Path::new(output_dirpath);
            log!("unzip to {}", output_dirpath.display());
            let mut zippy = Zippy::new();
            if let Some(input_zip_path) = unzip_matches.value_of("input") {
                zippy.unzip(Path::new(input_zip_path), output_dirpath)?;
            }
        }
    }

    Ok(())
}

struct Zippy {
    buffer: Vec<u8>,
}

impl Zippy {
    pub fn new() -> Self {
        Self {
            buffer: vec![],
        }
    }

    pub fn zip<'z>(&'z mut self,
                   input_paths: impl Iterator<Item = &'z Path>,
                   output_path: &Path,
                   method: CompressionMethod)
                   -> ZippyResult<()> {
        if output_path.exists() {
            // TODO: addition mode i.e. open the existing
            // zip file and add the new contents to it.
            log!("zip file exists: {}", output_path.display());
            process::exit(-1);
        }
        let mut zip: ZipWriter<_> = ZipWriter::new(File::create(output_path)?);
        let options = FileOptions::default()
            .compression_method(method)
            .unix_permissions(0o755);
        for input_path in input_paths {
            // log!("input: {}", input_path.display());
            if input_path.is_dir() {
                self.add_dir(&input_path, &mut zip, options)?;
            } else if input_path.is_file() {
                self.add_file(&input_path, &mut zip, options)?;
            } else {
                panic!("Neither file nor directory: {}", input_path.display());
                // TODO
            }
        }
        Ok(())
    }

    fn add_file<W>(&mut self,
                   input_filepath: &Path,
                   zip: &mut ZipWriter<W>,
                   options: FileOptions)
                   -> ZippyResult<()>
    where W: Write + Seek {
        if !input_filepath.is_file() {
            panic!("Error: not a file: {}", input_filepath.display());
            // TODO
        }
        let current_dir: PathBuf = env::current_dir()?;
        let input_filepath: &Path = input_filepath
            .strip_prefix(&current_dir)
            .unwrap_or(input_filepath);
        log!("zip {}", input_filepath.display());
        zip.start_file(input_filepath.to_str().unwrap(/*TODO*/), options)?;
        let mut f = File::open(&input_filepath)?;
        f.read_to_end(&mut self.buffer)?;
        zip.write_all(&*self.buffer)?;
        self.buffer.clear();
        Ok(())
    }

    fn add_dir<W>(&mut self,
                  input_dirpath: &Path,
                  zip: &mut ZipWriter<W>,
                  options: FileOptions)
                  -> ZippyResult<()>
    where W: Write + Seek {
        let dirpath: PathBuf = fs::canonicalize(input_dirpath)?;
        if !dirpath.is_dir() {
            panic!("Error: not a directory: {}", dirpath.display());
            // TODO
        }
        // log!("zip DIR {}", dirpath.display());
        for entry in WalkDir::new(&dirpath) { // recursively walk `dirpath`
            let entry: DirEntry = entry?;
            let entry_path: PathBuf = fs::canonicalize(entry.path())?;
            if entry_path.is_dir() {
                // log!("SKIP dir {}", entry_path.display());
                continue
            }
            self.add_file(&entry_path, zip, options)?;
        }
        Ok(())
    }

    pub fn unzip(&mut self, zip_filepath: &Path, output_dirpath: &Path)
                 -> ZippyResult<()> {
        if !output_dirpath.exists() {
            fs::create_dir(output_dirpath)?;
            println!("[unzip] created {}", output_dirpath.display());
        }

        let mut archive = ZipArchive::new(File::open(&zip_filepath)?)?;
        for i in 0..archive.len() {
            let mut zip_file: ZipFile = archive.by_index(i)?;
            let zip_file_name = zip_file.enclosed_name()
                .expect("Failed to extract file name from zip archive (idx: {i})");
            let output_path: PathBuf = output_dirpath.join(zip_file_name);
            Self::log_comment(i, &zip_file);

            if (&*zip_file.name()).ends_with('/') {
                println!("[unzip/{}] extracted dir {}", i, output_path.display());
                fs::create_dir_all(&output_path)?;
            } else {
                if let Some(p) = output_path.parent() {
                    if !p.exists() { fs::create_dir_all(&p)?; }
                }
                let mut output_file = File::create(&output_path)?;
                io::copy(&mut zip_file, &mut output_file)?;
                println!("[unzip/{}] extracted file {} ({})",
                         i,
                         output_path.display(),
                         Self::humanize(zip_file.size()));
            }

            Self::set_file_permissions(&zip_file, &output_path)?;
        }
        log!("[unzip] extracted {} files.", archive.len());
        Ok(())
    }

    fn log_comment(file_num: usize, zip_file: &ZipFile) {
        let comment = zip_file.comment();
        if !comment.is_empty() {
            println!("[unzip/{}] comment: {}", file_num, comment);
        }
    }

    #[cfg(unix)]
    fn set_file_permissions(zip_file: &ZipFile, output_path: &Path)
                            -> ZippyResult<()> {
        use std::os::unix::fs::PermissionsExt;
        if let Some(mode) = zip_file.unix_mode() {
            let permissions = fs::Permissions::from_mode(mode);
            fs::set_permissions(&output_path, permissions)?;
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn set_file_permissions(zip_file: &ZipFile, output_path: &Path)
                            -> ZippyResult<()> {
        Ok(()) // NOP
    }

    #[allow(non_upper_case_globals)]
    fn humanize<N>(bytes: N) -> String
    where N: Into<u128> {
        let bytes: u128 = bytes.into();
        const KiB: u128 = 1024;        // kibibyte
        const MiB: u128 = 1024 * KiB;  // mebibyte
        const GiB: u128 = 1024 * MiB;  // gibibyte
        const TiB: u128 = 1024 * GiB;  // tebibyte
        const PiB: u128 = 1024 * TiB;  // pebibyte
        const EiB: u128 = 1024 * PiB;  // exbibyte
        const ZiB: u128 = 1024 * EiB;  // zebibyte
        const YiB: u128 = 1024 * ZiB;  // yobibyte
        match bytes {
            bytes if bytes < KiB => format!("{} bytes", bytes),
            bytes if KiB <= bytes && bytes < MiB =>
                format!("{} KiB", bytes / KiB),
            bytes if MiB <= bytes && bytes < GiB =>
                format!("{} MiB", bytes / MiB),
            bytes if GiB <= bytes && bytes < TiB =>
                format!("{} GiB", bytes / GiB),
            bytes if TiB <= bytes && bytes < EiB =>
                format!("{} TiB", bytes / TiB),
            bytes if EiB <= bytes && bytes < ZiB =>
                format!("{} EiB", bytes / EiB),
            bytes if ZiB <= bytes && bytes < YiB =>
                format!("{} ZiB", bytes / ZiB),
            bytes => format!("{} YiB", bytes / YiB),
        }
    }

}
