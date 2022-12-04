#[macro_use]
mod log;
mod result;

use crate::result::ZippyResult;
use clap::{Parser, Subcommand, ValueEnum};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::iter::Iterator;
use std::path::{Path, PathBuf};
use std::process;
use walkdir::{DirEntry, WalkDir};
use zip::{read::ZipFile, write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn about() -> String {
    let mut buf = String::new();
    buf.push_str(&format!("{APP_NAME} v{APP_VERSION}\n"));
    buf.push_str(&format!(
        "{APP_NAME} is a simple tool for de/compressing zip files.\n"
    ));
    buf.push_str("It has the following features:\n");
    buf.push_str("- Installable using `cargo install zippy`\n");
    buf.push_str("- Easy to use");
    buf
}

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = about(),
)]
struct CliArgs {
    #[command(subcommand)]
    command: Command,

    /// Sets the level of verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbosity: u8,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(arg_required_else_help = true)]
    /// Decompress a zip file.
    Unzip {
        #[arg(required = true, short, long)]
        /// File path of the input zip archive
        input: PathBuf,
        #[arg(required = true, short, long)]
        /// Dir path of the output directory
        output: PathBuf,
    },
    #[command(arg_required_else_help = true)]
    /// Compress files and directories into a zip file.
    Zip {
        #[arg(required = true, num_args = 1.., short, long)]
        /// File paths of the files to be archived
        inputs: Vec<PathBuf>,
        #[arg(required = true, short, long)]
        /// File path of the output zip archive
        output: PathBuf,
        #[arg(short, long)]
        /// The decompression method to use
        method: Method,
        #[arg(short, long)]
        /// The compression level is dependant on which compression method
        /// is used; See the `zip-rs` documentation for more info
        level: Option<i32>,
    },
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
enum Method {
    Bzip2,
    Deflate,
    Store,
    Zstd,
}

impl From<Method> for CompressionMethod {
    fn from(method: Method) -> Self {
        match method {
            Method::Bzip2 => CompressionMethod::Bzip2,
            Method::Deflate => CompressionMethod::Deflated,
            Method::Store => CompressionMethod::Stored,
            Method::Zstd => CompressionMethod::Zstd,
        }
    }
}

fn main() -> ZippyResult<()> {
    let cli_args = CliArgs::parse();

    // Record how many times the user used the "verbose" flag
    // (e.g. 'myprog -v -v -v' or 'myprog -vvv' vs 'myprog -v')
    let verbose_flag = "v".repeat(cli_args.verbosity as usize);
    match cli_args.verbosity {
        0 => { /* silent mode */ }
        1 => log!("-{verbose_flag}: verbose mode"),
        2 => log!("-{verbose_flag}: very verbose mode"),
        _ => {
            log!("-{verbose_flag}: why do you even want so much information?");
            process::exit(-1);
        }
    }

    let mut zippy = Zippy::new();
    match &cli_args.command {
        Command::Unzip { input, output } => {
            ensure_dir_exists(Some(output))?;
            log!("unzip to directory {}", output.display());
            zippy.unzip(input, output)?;
        }
        Command::Zip { inputs, output, method, level } => {
            ensure_dir_exists(output.parent())?;
            log!("zip to file @ {}", output.display());
            zippy.zip(
                inputs.iter().map(PathBuf::as_path),
                &output,
                (*method).into(),
                *level,
            )?;
        }
    }

    Ok(())
}

fn ensure_dir_exists(dirpath: Option<&Path>) -> ZippyResult<()> {
    match dirpath {
        Some(dir) if dir.exists() => { /*NOP*/ }
        Some(dir) => std::fs::create_dir_all(dir)?,
        None => std::fs::create_dir(std::env::current_dir()?)?,
    }
    Ok(())
}

struct Zippy {
    buffer: Vec<u8>,
}

impl Zippy {
    pub fn new() -> Self {
        Self { buffer: vec![] }
    }

    pub fn zip<'zip>(
        &'zip mut self,
        input_paths: impl Iterator<Item = &'zip Path>,
        output_path: &Path,
        method: CompressionMethod,
        level: Option<i32>,
    ) -> ZippyResult<()> {
        if output_path.exists() {
            // TODO: addition mode i.e. open the existing
            // zip file and add the new contents to it.
            log!("zip file exists: {}", output_path.display());
            process::exit(-1);
        }
        let mut zip: ZipWriter<_> = ZipWriter::new(File::create(output_path)?);
        let options = FileOptions::default()
            .compression_method(method)
            .unix_permissions(0o755)
            .compression_level(level);
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

    fn add_file<W>(
        &mut self,
        input_filepath: &Path,
        zip: &mut ZipWriter<W>,
        options: FileOptions,
    ) -> ZippyResult<()>
    where
        W: Write + Seek,
    {
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

    fn add_dir<W>(
        &mut self,
        input_dirpath: &Path,
        zip: &mut ZipWriter<W>,
        options: FileOptions,
    ) -> ZippyResult<()>
    where
        W: Write + Seek,
    {
        let dirpath: PathBuf = fs::canonicalize(input_dirpath)?;
        if !dirpath.is_dir() {
            panic!("Error: not a directory: {}", dirpath.display());
            // TODO
        }
        for entry in WalkDir::new(&dirpath) { // recursively walk `dirpath`
            let entry: DirEntry = entry?;
            let entry_path: PathBuf = fs::canonicalize(entry.path())?;
            if entry_path.is_dir() {
                continue;
            }
            self.add_file(&entry_path, zip, options)?;
        }
        Ok(())
    }

    pub fn unzip(
        &mut self,
        zip_filepath: impl AsRef<Path>,
        output_dirpath: impl AsRef<Path>,
    ) -> ZippyResult<()> {
        let zip_filepath = zip_filepath.as_ref();
        let output_dirpath = output_dirpath.as_ref();
        if !output_dirpath.exists() {
            fs::create_dir(output_dirpath)?;
            println!("[unzip] created {}", output_dirpath.display());
        }

        let mut archive = ZipArchive::new(File::open(&zip_filepath)?)?;
        for i in 0..archive.len() {
            let mut zip_file: ZipFile = archive.by_index(i)?;
            let zip_file_name = zip_file
                .enclosed_name()
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
                println!(
                    "[unzip/{}] extracted file {} ({})",
                    i,
                    output_path.display(),
                    Self::humanize(zip_file.size())
                );
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
    fn set_file_permissions(zip_file: &ZipFile, output_path: &Path) -> ZippyResult<()> {
        use std::os::unix::fs::PermissionsExt;
        if let Some(mode) = zip_file.unix_mode() {
            let permissions = fs::Permissions::from_mode(mode);
            fs::set_permissions(&output_path, permissions)?;
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn set_file_permissions(zip_file: &ZipFile, output_path: &Path) -> ZippyResult<()> {
        Ok(()) // NOP
    }

    #[allow(non_upper_case_globals)]
    fn humanize<N>(bytes: N) -> String
    where
        N: Into<u128>,
    {
        let bytes: u128 = bytes.into();
        const KiB: u128 = 1024; // kibibyte
        const MiB: u128 = 1024 * KiB; // mebibyte
        const GiB: u128 = 1024 * MiB; // gibibyte
        const TiB: u128 = 1024 * GiB; // tebibyte
        const PiB: u128 = 1024 * TiB; // pebibyte
        const EiB: u128 = 1024 * PiB; // exbibyte
        const ZiB: u128 = 1024 * EiB; // zebibyte
        const YiB: u128 = 1024 * ZiB; // yobibyte
        match bytes {
            b if b < KiB => format!("{} bytes", b),
            b if KiB <= b && b < MiB => format!("{} KiB", b / KiB),
            b if MiB <= b && b < GiB => format!("{} MiB", b / MiB),
            b if GiB <= b && b < TiB => format!("{} GiB", b / GiB),
            b if TiB <= b && b < EiB => format!("{} TiB", b / TiB),
            b if EiB <= b && b < ZiB => format!("{} EiB", b / EiB),
            b if ZiB <= b && b < YiB => format!("{} ZiB", b / ZiB),
            b => format!("{} YiB", b / YiB),
        }
    }
}
