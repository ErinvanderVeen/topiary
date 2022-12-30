use std::{error::Error, ffi, fmt, io, process, str, string};

/// The various errors the formatter may return.
#[derive(Debug)]
pub enum TopiaryError {
    /// The input produced output that cannot be formatted, i.e. trying to format the
    /// output again produced an error. If this happened using our provided
    /// query files, it is a bug. Please log an issue.
    Formatting(Box<TopiaryError>),

    /// The input produced output that isn't idempotent, i.e. formatting the
    /// output again made further changes. If this happened using our provided
    /// query files, it is a bug. Please log an issue.
    Idempotence,

    /// An internal error occurred. This is a bug. Please log an issue.
    Internal(String, Option<io::Error>),

    /// Tree-sitter could not parse the input without errors.
    Parsing {
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    },

    /// There was an error in the query file. If this happened using our
    /// provided query files, it is a bug. Please log an issue.
    Query(String, Option<tree_sitter::QueryError>),

    /// Could not detect the input language from the (filename, Option<extension>)
    LanguageDetection(String, Option<ffi::OsString>),

    /// Could not read the input.
    Reading(ReadingError),

    /// Could not write the formatted output.
    Writing(WritingError),

    /// Any issue that occured related to Git.
    Git(git2::Error),

    /// There was an error loading tree-sitter error
    ParserLoading(libloading::Error),

    /// Any error related to the compilation
    ParserCompilation(ParserCompilationError),
}

/// A subtype of `TopiaryError::Reading`.
#[derive(Debug)]
pub enum ReadingError {
    Io(String, io::Error),
    Utf8(str::Utf8Error),
}

/// A subtype of `TopiaryError::Writing`.
#[derive(Debug)]
pub enum WritingError {
    Fmt(fmt::Error),
    IntoInner(io::IntoInnerError<io::BufWriter<Vec<u8>>>),
    Io(io::Error),
    FromUtf8(string::FromUtf8Error),
}

/// A subtype of `TopiaryError::ParserCompilation`.
#[derive(Debug)]
pub enum ParserCompilationError {
    Io(io::Error),
    Cc(String, String),
}

impl fmt::Display for TopiaryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let please_log_message = "It would be helpful if you logged this error at https://github.com/tweag/topiary/issues/new?assignees=&labels=type%3A+bug&template=bug_report.md";
        match self {
            Self::Idempotence => {
                write!(
                    f,
                    "The formatter did not produce the same result when invoked twice (idempotence check).\n{please_log_message}"
                )
            }
            Self::Parsing {
                start_line,
                start_column,
                end_line,
                end_column,
            } => {
                write!(f, "Parsing error between line {start_line}, column {start_column} and line {end_line}, column {end_column}")
            }
            Self::Reading(ReadingError::Io(message, _)) => {
                write!(f, "{message}")
            }
            Self::Reading(ReadingError::Utf8(_)) => {
                write!(f, "Input is not UTF8")
            }
            Self::Writing(_) => {
                write!(f, "Writing error")
            }
            Self::Internal(message, _) | Self::Query(message, _) => {
                write!(f, "{message}")
            }
            Self::LanguageDetection(filename, extension) => {
                let file: String = match filename.as_str() {
                    "-" => "from standard input".into(),
                    _ => format!("of file '{filename}'"),
                };

                match extension {
                    Some(extension) => write!(f,
                        "Cannot detect language {file} due to unknown extension '.{}'. Try specifying language explicitly.",
                        extension.to_string_lossy()
                    ),
                    None => write!(f,
                        "Cannot detect language {file}. Try specifying language explicitly."
                    ),
                }
            }
            Self::Formatting(_err) => {
                write!(
                    f,
                    "The formatter errored when trying to format the code twice (idempotence check).\nThis probably means that the formatter produced invalid code.\n{please_log_message}"
                )
            }
            Self::Git(err) => {
                write!(
                    f,
                    "The formatter errored when trying to fetch a grammar from git. See the following message: {}", err
                )
            }
            Self::ParserLoading(err) => {
                write!(
                    f,
                    "The formatter errored when trying load the parser dynamic library: {}",
                    err
                )
            }
            Self::ParserCompilation(err) => match err {
                ParserCompilationError::Io(err) => write!(
                    f,
                    "The formatter ran into an IO error when compiling a Parser: {err}"
                ),
                ParserCompilationError::Cc(out, err) => write!(
                    f,
                    "The formatter ran into an error when compiling a Parser. Output from the CC subprocess: {out} {err}"
                ),
            },
        }
    }
}

impl Error for TopiaryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Idempotence => None,
            Self::Internal(_, source) => source.as_ref().map(|e| e as &dyn Error),
            Self::Parsing { .. } => None,
            Self::Query(_, source) => source.as_ref().map(|e| e as &dyn Error),
            Self::LanguageDetection(_, _) => None,
            Self::Reading(ReadingError::Io(_, source)) => Some(source),
            Self::Reading(ReadingError::Utf8(source)) => Some(source),
            Self::Writing(WritingError::Fmt(source)) => Some(source),
            Self::Writing(WritingError::FromUtf8(source)) => Some(source),
            Self::Writing(WritingError::IntoInner(source)) => Some(source),
            Self::Writing(WritingError::Io(source)) => Some(source),
            Self::Formatting(err) => Some(err),
            Self::Git(err) => Some(err),
            Self::ParserLoading(err) => Some(err),
            Self::ParserCompilation(ParserCompilationError::Io(err)) => Some(err),
            Self::ParserCompilation(ParserCompilationError::Cc(_, _)) => None,
        }
    }
}

impl From<str::Utf8Error> for TopiaryError {
    fn from(e: str::Utf8Error) -> Self {
        TopiaryError::Reading(ReadingError::Utf8(e))
    }
}

impl From<io::Error> for TopiaryError {
    fn from(e: io::Error) -> Self {
        TopiaryError::Writing(WritingError::Io(e))
    }
}

impl From<string::FromUtf8Error> for TopiaryError {
    fn from(e: string::FromUtf8Error) -> Self {
        TopiaryError::Writing(WritingError::FromUtf8(e))
    }
}

impl From<io::IntoInnerError<io::BufWriter<Vec<u8>>>> for TopiaryError {
    fn from(e: io::IntoInnerError<io::BufWriter<Vec<u8>>>) -> Self {
        TopiaryError::Writing(WritingError::IntoInner(e))
    }
}

impl From<fmt::Error> for TopiaryError {
    fn from(e: fmt::Error) -> Self {
        TopiaryError::Writing(WritingError::Fmt(e))
    }
}
