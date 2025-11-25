//! Parsing for file:refs syntax into structured line references.
//!
//! This module handles parsing user input like `file.nix:10,15,-20` into
//! structured [`FileLineRefs`] that can be used to filter git diffs.
//!
//! # Syntax
//!
//! The expected format is `FILE:REFS` where:
//! - `FILE` is a file path (cannot be empty)
//! - `REFS` is a comma-separated list of line references
//!
//! # Line Reference Types
//!
//! - `N` - Addition at new line N
//! - `-N` - Deletion at old line N
//! - `N..M` - Range of additions (inclusive)
//! - `-N..-M` - Range of deletions (inclusive)
//!
//! # Examples
//!
//! ```
//! use git_lines::parse::{parse_file_refs, LineRef};
//! use std::num::NonZeroU32;
//!
//! // Single addition
//! let refs = parse_file_refs("flake.nix:137").unwrap();
//! assert_eq!(refs.file, "flake.nix");
//! assert_eq!(refs.refs, vec![LineRef::Add(NonZeroU32::new(137).unwrap())]);
//!
//! // Range
//! let refs = parse_file_refs("config.nix:10..15").unwrap();
//! assert_eq!(refs.refs, vec![LineRef::AddRange(
//!     NonZeroU32::new(10).unwrap(),
//!     NonZeroU32::new(15).unwrap()
//! )]);
//!
//! // Mixed operations
//! let refs = parse_file_refs("file.nix:-10,12").unwrap();
//! assert_eq!(refs.refs, vec![
//!     LineRef::Delete(NonZeroU32::new(10).unwrap()),
//!     LineRef::Add(NonZeroU32::new(12).unwrap())
//! ]);
//! ```

use error_set::error_set;
use std::num::NonZeroU32;

error_set! {
    /// Errors from parsing file:refs syntax
    ParseError := {
        /// Input string does not contain a colon separator
        #[display("Invalid format '{input}': expected 'file:refs'")]
        InvalidFormat { input: String },
        /// File name portion before the colon is empty or whitespace
        #[display("Invalid format '{input}': file name cannot be empty")]
        EmptyFileName { input: String },
        /// No line references provided after the colon
        #[display("No line references provided")]
        EmptyRefs,
        /// Line number could not be parsed as a valid non-zero u32
        #[display("Invalid line number '{value}'")]
        InvalidLineNumber { value: String },
        /// Range has start greater than end
        #[display("Invalid range {start}..{end}: start must be <= end")]
        InvalidRange { start: u32, end: u32 },
        /// Deletion reference does not start with '-' prefix
        #[display("Delete reference must start with '-', got '{value}'")]
        InvalidDeleteRef { value: String },
    }
}

/// A reference to specific lines to stage.
///
/// Line references specify which lines from a git diff should be staged.
/// Additions reference new line numbers, deletions reference old line numbers.
#[derive(Debug, Clone, PartialEq)]
pub enum LineRef {
    /// Addition at new line number
    Add(NonZeroU32),
    /// Addition range (inclusive start and end)
    AddRange(NonZeroU32, NonZeroU32),
    /// Deletion at old line number
    Delete(NonZeroU32),
    /// Deletion range (inclusive start and end)
    DeleteRange(NonZeroU32, NonZeroU32),
}

/// Parsed file reference with line selections.
///
/// Represents the structured form of a `file:refs` string after parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct FileLineRefs {
    /// The file path
    pub file: String,
    /// The line references to stage from this file
    pub refs: Vec<LineRef>,
}

/// Parse a file:refs string into structured data.
///
/// # Format
///
/// `FILE:REFS` where REFS is a comma-separated list of:
/// - `N` - Addition at line N
/// - `-N` - Deletion of line N
/// - `N..M` - Addition range
/// - `-N..-M` - Deletion range
///
/// # Examples
///
/// ```
/// use git_lines::parse::{parse_file_refs, LineRef};
/// use std::num::NonZeroU32;
///
/// let refs = parse_file_refs("flake.nix:137").unwrap();
/// assert_eq!(refs.file, "flake.nix");
/// assert_eq!(refs.refs, vec![LineRef::Add(NonZeroU32::new(137).unwrap())]);
///
/// let refs = parse_file_refs("file.nix:10..15").unwrap();
/// assert_eq!(refs.refs, vec![LineRef::AddRange(
///     NonZeroU32::new(10).unwrap(),
///     NonZeroU32::new(15).unwrap()
/// )]);
///
/// let refs = parse_file_refs("file.nix:10,15,-20").unwrap();
/// assert_eq!(refs.refs, vec![
///     LineRef::Add(NonZeroU32::new(10).unwrap()),
///     LineRef::Add(NonZeroU32::new(15).unwrap()),
///     LineRef::Delete(NonZeroU32::new(20).unwrap())
/// ]);
/// ```
///
/// # Errors
///
/// Returns [`ParseError`] if:
/// - Input doesn't contain `:` separator
/// - File name is empty or whitespace
/// - No line references provided
/// - Line numbers are invalid
pub fn parse_file_refs(input: &str) -> Result<FileLineRefs, ParseError> {
    let parts: Vec<&str> = input.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(ParseError::InvalidFormat {
            input: input.to_string(),
        });
    }

    let file = parts[0].trim();
    if file.is_empty() {
        return Err(ParseError::EmptyFileName {
            input: input.to_string(),
        });
    }

    Ok(FileLineRefs {
        file: file.to_string(),
        refs: parse_line_refs(parts[1])?,
    })
}

/// Parse the line references part (after the colon)
/// Examples: "137", "10..15", "10,15,-20"
fn parse_line_refs(input: &str) -> Result<Vec<LineRef>, ParseError> {
    let refs: Vec<LineRef> = input
        .split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(parse_single_ref)
        .collect::<Result<Vec<_>, _>>()?;

    if refs.is_empty() {
        return Err(ParseError::EmptyRefs);
    }

    Ok(refs)
}

/// Parse a single line reference (could be single number, range, or deletion)
fn parse_single_ref(input: &str) -> Result<LineRef, ParseError> {
    // Check for range syntax (N..M or -N..-M)
    if let Some((start_str, end_str)) = input.split_once("..") {
        // Determine if it's a deletion range
        if start_str.starts_with('-') {
            let start = parse_delete_number(start_str)?;
            let end = parse_delete_number(end_str)?;
            if start > end {
                return Err(ParseError::InvalidRange {
                    start: start.get(),
                    end: end.get(),
                });
            }
            Ok(LineRef::DeleteRange(start, end))
        } else {
            let start = parse_add_number(start_str)?;
            let end = parse_add_number(end_str)?;
            if start > end {
                return Err(ParseError::InvalidRange {
                    start: start.get(),
                    end: end.get(),
                });
            }
            Ok(LineRef::AddRange(start, end))
        }
    } else if input.starts_with('-') {
        Ok(LineRef::Delete(parse_delete_number(input)?))
    } else {
        Ok(LineRef::Add(parse_add_number(input)?))
    }
}

/// Parse a positive line number (for additions)
fn parse_add_number(input: &str) -> Result<NonZeroU32, ParseError> {
    input
        .parse::<NonZeroU32>()
        .map_err(|_| ParseError::InvalidLineNumber {
            value: input.to_string(),
        })
}

/// Parse a negative line number (for deletions)
fn parse_delete_number(input: &str) -> Result<NonZeroU32, ParseError> {
    if !input.starts_with('-') {
        return Err(ParseError::InvalidDeleteRef {
            value: input.to_string(),
        });
    }
    input[1..]
        .parse::<NonZeroU32>()
        .map_err(|_| ParseError::InvalidLineNumber {
            value: input.to_string(),
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use similar_asserts::assert_eq;

    fn nz(n: u32) -> NonZeroU32 {
        NonZeroU32::new(n).unwrap()
    }

    #[test]
    fn parse_single_addition() {
        let result = parse_file_refs("flake.nix:137").unwrap();
        assert_eq!(result.file, "flake.nix");
        assert_eq!(result.refs, vec![LineRef::Add(nz(137))]);
    }

    #[test]
    fn parse_addition_range() {
        let result = parse_file_refs("flake.nix:39..43").unwrap();
        assert_eq!(result.file, "flake.nix");
        assert_eq!(result.refs, vec![LineRef::AddRange(nz(39), nz(43))]);
    }

    #[test]
    fn parse_multiple_additions() {
        let result = parse_file_refs("default.nix:40,41").unwrap();
        assert_eq!(result.file, "default.nix");
        assert_eq!(
            result.refs,
            vec![LineRef::Add(nz(40)), LineRef::Add(nz(41))]
        );
    }

    #[test]
    fn parse_single_deletion() {
        let result = parse_file_refs("zsh.nix:-15").unwrap();
        assert_eq!(result.file, "zsh.nix");
        assert_eq!(result.refs, vec![LineRef::Delete(nz(15))]);
    }

    #[test]
    fn parse_deletion_range() {
        let result = parse_file_refs("gtk.nix:-10..-11").unwrap();
        assert_eq!(result.file, "gtk.nix");
        assert_eq!(result.refs, vec![LineRef::DeleteRange(nz(10), nz(11))]);
    }

    #[test]
    fn parse_mixed_refs() {
        let result = parse_file_refs("gtk.nix:-10,-11,12").unwrap();
        assert_eq!(result.file, "gtk.nix");
        assert_eq!(
            result.refs,
            vec![
                LineRef::Delete(nz(10)),
                LineRef::Delete(nz(11)),
                LineRef::Add(nz(12))
            ]
        );
    }

    #[test]
    fn parse_range_with_deletion() {
        let result = parse_file_refs("file.nix:10..15,-20").unwrap();
        assert_eq!(result.file, "file.nix");
        assert_eq!(
            result.refs,
            vec![LineRef::AddRange(nz(10), nz(15)), LineRef::Delete(nz(20))]
        );
    }

    #[test]
    fn parse_invalid_format() {
        assert!(parse_file_refs("no_colon").is_err());
    }

    #[test]
    fn parse_empty_refs() {
        assert!(parse_file_refs("file.nix:").is_err());
    }

    #[test]
    fn parse_empty_file_name() {
        let result = parse_file_refs(":10");
        assert!(matches!(result, Err(ParseError::EmptyFileName { .. })));
    }

    #[test]
    fn parse_empty_file_with_range() {
        let result = parse_file_refs(":10..15");
        assert!(matches!(result, Err(ParseError::EmptyFileName { .. })));
    }

    #[test]
    fn parse_whitespace_file_name() {
        let result = parse_file_refs("  :10");
        assert!(matches!(result, Err(ParseError::EmptyFileName { .. })));
    }

    #[test]
    fn parse_just_colon() {
        let result = parse_file_refs(":");
        assert!(result.is_err());
    }

    #[test]
    fn parse_zero_line_number() {
        let result = parse_file_refs("file.nix:0");
        assert!(matches!(result, Err(ParseError::InvalidLineNumber { .. })));
    }

    #[test]
    fn parse_zero_deletion() {
        let result = parse_file_refs("file.nix:-0");
        assert!(matches!(result, Err(ParseError::InvalidLineNumber { .. })));
    }

    #[test]
    fn parse_zero_in_range_start() {
        let result = parse_file_refs("file.nix:0..10");
        assert!(matches!(result, Err(ParseError::InvalidLineNumber { .. })));
    }

    #[test]
    fn parse_zero_in_range_end() {
        let result = parse_file_refs("file.nix:10..0");
        // Zero check happens before range validation
        assert!(matches!(result, Err(ParseError::InvalidLineNumber { .. })));
    }

    #[test]
    fn parse_inverted_range() {
        let result = parse_file_refs("file.nix:15..10");
        assert!(matches!(
            result,
            Err(ParseError::InvalidRange { start: 15, end: 10 })
        ));
    }

    #[test]
    fn parse_inverted_deletion_range() {
        let result = parse_file_refs("file.nix:-15..-10");
        assert!(matches!(
            result,
            Err(ParseError::InvalidRange { start: 15, end: 10 })
        ));
    }

    #[test]
    fn parse_equal_range() {
        // 10..10 is valid - it's a single-element range
        let result = parse_file_refs("file.nix:10..10").unwrap();
        assert_eq!(result.refs, vec![LineRef::AddRange(nz(10), nz(10))]);
    }
}
