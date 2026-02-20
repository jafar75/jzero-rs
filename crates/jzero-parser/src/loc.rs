/// Convert a byte offset into a 1-based line number.
///
/// Used by grammar action code to create leaf nodes with line info.
/// The source text is passed through from the parser.
pub fn line_from_offset(input: &str, offset: usize) -> usize {
    let mut line = 1;
    for (i, ch) in input.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_line() {
        assert_eq!(line_from_offset("hello world", 5), 1);
    }

    #[test]
    fn test_second_line() {
        assert_eq!(line_from_offset("hello\nworld", 6), 2);
    }

    #[test]
    fn test_third_line() {
        assert_eq!(line_from_offset("a\nb\nc", 4), 3);
    }

    #[test]
    fn test_offset_zero() {
        assert_eq!(line_from_offset("hello", 0), 1);
    }
}