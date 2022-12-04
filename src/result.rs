///
use ioe::IoError;
use std::io;
use std::path::StripPrefixError;
use walkdir::Error as WalkDirError;
use zip::result::ZipError;

pub type ZippyResult<T> = Result<T, ZippyError>;

#[derive(Debug)]
pub enum ZippyError {
    IoError(IoError),
    StripPathPrefixError,
    WalkDirError(WalkDirError),
    ZipError(ZipError),
}

impl From<IoError> for ZippyError {
    fn from(err: IoError) -> ZippyError {
        ZippyError::IoError(err)
    }
}

impl From<io::Error> for ZippyError {
    fn from(err: io::Error) -> ZippyError {
        ZippyError::IoError(IoError::from(err))
    }
}

impl From<StripPrefixError> for ZippyError {
    fn from(_: StripPrefixError /*contains no useful data*/) -> ZippyError {
        ZippyError::StripPathPrefixError
    }
}

impl From<WalkDirError> for ZippyError {
    fn from(err: WalkDirError) -> ZippyError {
        ZippyError::WalkDirError(err)
    }
}

impl From<ZipError> for ZippyError {
    fn from(err: ZipError) -> ZippyError {
        ZippyError::ZipError(err)
    }
}
