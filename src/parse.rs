/// A reference to specific lines to stage
#[derive(Debug, Clone, PartialEq)]
pub enum LineRef {
    /// Addition at new line number
    Add(u32),
    /// Addition range (inclusive)
    AddRange(u32, u32),
    /// Deletion at old line number
    Delete(u32),
    /// Deletion range (inclusive)
    DeleteRange(u32, u32),
}

/// Parsed file reference with line selections
#[derive(Debug, Clone, PartialEq)]
pub struct FileLineRefs {
    pub file: String,
    pub refs: Vec<LineRef>,
}

/// Parse a file:refs string into structured data
/// Examples:
/// - "flake.nix:137" -> FileLineRefs { file: "flake.nix", refs: [Add(137)] }
/// - "file.nix:10..15" -> FileLineRefs { file: "file.nix", refs: [AddRange(10, 15)] }
/// - "file.nix:10,15,-20" -> FileLineRefs { file: "file.nix", refs: [Add(10), Add(15), Delete(20)] }
pub fn parse_file_refs(input: &str) -> Result<FileLineRefs, String> {
    let parts: Vec<&str> = input.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid format '{}': expected 'file:refs'", input));
    }

    let file = parts[0].trim();
    if file.is_empty() {
        return Err(format!(
            "Invalid format '{}': file name cannot be empty",
            input
        ));
    }

    Ok(FileLineRefs {
        file: file.to_string(),
        refs: parse_line_refs(parts[1])?,
    })
}

/// Parse the line references part (after the colon)
/// Examples: "137", "10..15", "10,15,-20"
fn parse_line_refs(input: &str) -> Result<Vec<LineRef>, String> {
    input
        .split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(parse_single_ref)
        .collect::<Result<Vec<_>, _>>()
        .and_then(|r| {
            if r.is_empty() {
                Err("No line references provided".into())
            } else {
                Ok(r)
            }
        })
}

/// Parse a single line reference (could be single number, range, or deletion)
fn parse_single_ref(input: &str) -> Result<LineRef, String> {
    // Check for range syntax (N..M or -N..-M)
    if let Some((start_str, end_str)) = input.split_once("..") {
        // Determine if it's a deletion range
        if start_str.starts_with('-') {
            Ok(LineRef::DeleteRange(
                parse_delete_number(start_str)?,
                parse_delete_number(end_str)?,
            ))
        } else {
            Ok(LineRef::AddRange(
                parse_add_number(start_str)?,
                parse_add_number(end_str)?,
            ))
        }
    } else if input.starts_with('-') {
        Ok(LineRef::Delete(parse_delete_number(input)?))
    } else {
        Ok(LineRef::Add(parse_add_number(input)?))
    }
}

/// Parse a positive line number (for additions)
fn parse_add_number(input: &str) -> Result<u32, String> {
    input
        .parse::<u32>()
        .map_err(|_| format!("Invalid line number '{}'", input))
}

/// Parse a negative line number (for deletions)
fn parse_delete_number(input: &str) -> Result<u32, String> {
    if !input.starts_with('-') {
        return Err(format!(
            "Delete reference must start with '-', got '{}'",
            input
        ));
    }
    input[1..]
        .parse::<u32>()
        .map_err(|_| format!("Invalid delete line number '{}'", input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_addition() {
        let result = parse_file_refs("flake.nix:137").unwrap();
        assert_eq!(result.file, "flake.nix");
        assert_eq!(result.refs, vec![LineRef::Add(137)]);
    }

    #[test]
    fn parse_addition_range() {
        let result = parse_file_refs("flake.nix:39..43").unwrap();
        assert_eq!(result.file, "flake.nix");
        assert_eq!(result.refs, vec![LineRef::AddRange(39, 43)]);
    }

    #[test]
    fn parse_multiple_additions() {
        let result = parse_file_refs("default.nix:40,41").unwrap();
        assert_eq!(result.file, "default.nix");
        assert_eq!(result.refs, vec![LineRef::Add(40), LineRef::Add(41)]);
    }

    #[test]
    fn parse_single_deletion() {
        let result = parse_file_refs("zsh.nix:-15").unwrap();
        assert_eq!(result.file, "zsh.nix");
        assert_eq!(result.refs, vec![LineRef::Delete(15)]);
    }

    #[test]
    fn parse_deletion_range() {
        let result = parse_file_refs("gtk.nix:-10..-11").unwrap();
        assert_eq!(result.file, "gtk.nix");
        assert_eq!(result.refs, vec![LineRef::DeleteRange(10, 11)]);
    }

    #[test]
    fn parse_mixed_refs() {
        let result = parse_file_refs("gtk.nix:-10,-11,12").unwrap();
        assert_eq!(result.file, "gtk.nix");
        assert_eq!(
            result.refs,
            vec![LineRef::Delete(10), LineRef::Delete(11), LineRef::Add(12)]
        );
    }

    #[test]
    fn parse_range_with_deletion() {
        let result = parse_file_refs("file.nix:10..15,-20").unwrap();
        assert_eq!(result.file, "file.nix");
        assert_eq!(
            result.refs,
            vec![LineRef::AddRange(10, 15), LineRef::Delete(20)]
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
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn parse_empty_file_with_range() {
        let result = parse_file_refs(":10..15");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn parse_whitespace_file_name() {
        let result = parse_file_refs("  :10");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn parse_just_colon() {
        let result = parse_file_refs(":");
        assert!(result.is_err());
    }
}
