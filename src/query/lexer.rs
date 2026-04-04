/// Token types for the query expression language.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    String(String),
    Number(f64),
    Boolean(bool),

    // Identifiers
    Field(String),     // e.g. title, tags, path
    FmField(String),   // e.g. frontmatter.status → "status"

    // Operators
    Eq,        // =
    Neq,       // !=
    Gt,        // >
    Lt,        // <
    Gte,       // >=
    Lte,       // <=

    // Keywords
    And,
    Or,
    Contains,
    Matches,
    In,

    // Special functions
    Func(String),       // linked_from, etc.

    // Delimiters
    LParen,
    RParen,
    Comma,
    LBracket,
    RBracket,

    End,
}

/// Tokenize a query expression string.
pub fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '=' => {
                chars.next();
                tokens.push(Token::Eq);
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Neq);
                } else {
                    return Err("Unexpected '!'".to_string());
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Gte);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Lte);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            '[' => {
                chars.next();
                tokens.push(Token::LBracket);
            }
            ']' => {
                chars.next();
                tokens.push(Token::RBracket);
            }
            '\'' | '"' => {
                let quote = c;
                chars.next();
                let mut s = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch == quote {
                        chars.next();
                        break;
                    }
                    s.push(ch);
                    chars.next();
                }
                tokens.push(Token::String(s));
            }
            '0'..='9' => {
                let mut num = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_digit() || ch == '.' {
                        num.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Number(num.parse::<f64>().map_err(|e| e.to_string())?));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' || ch == '.' {
                        ident.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }

                // Check for keywords
                match ident.as_str() {
                    "AND" => tokens.push(Token::And),
                    "OR" => tokens.push(Token::Or),
                    "contains" => tokens.push(Token::Contains),
                    "matches" => tokens.push(Token::Matches),
                    "in" => tokens.push(Token::In),
                    "true" => tokens.push(Token::Boolean(true)),
                    "false" => tokens.push(Token::Boolean(false)),
                    s if s.starts_with("frontmatter.") => {
                        tokens.push(Token::FmField(s[12..].to_string()));
                    }
                    s => tokens.push(Token::Field(s.to_string())),
                }
            }
            _ => {
                return Err(format!("Unexpected character: '{}'", c));
            }
        }
    }

    tokens.push(Token::End);
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_basic() {
        let tokens = tokenize("status = 'draft'").unwrap();
        assert_eq!(tokens[0], Token::Field("status".to_string()));
        assert_eq!(tokens[1], Token::Eq);
        assert_eq!(tokens[2], Token::String("draft".to_string()));
    }

    #[test]
    fn test_tokenize_and() {
        let tokens = tokenize("status = 'draft' AND tags contains 'rust'").unwrap();
        assert_eq!(tokens[3], Token::And);
        assert_eq!(tokens[5], Token::Contains);
    }

    #[test]
    fn test_tokenize_frontmatter() {
        let tokens = tokenize("frontmatter.date > '2024-01-01'").unwrap();
        assert_eq!(tokens[0], Token::FmField("date".to_string()));
        assert_eq!(tokens[1], Token::Gt);
    }

    #[test]
    fn test_tokenize_operators() {
        let tokens = tokenize("count >= 5 AND age != 10").unwrap();
        assert!(tokens.contains(&Token::Gte));
        assert!(tokens.contains(&Token::Neq));
    }

    #[test]
    fn test_tokenize_brackets() {
        let tokens = tokenize("tags in ['rust', 'cli']").unwrap();
        assert!(tokens.contains(&Token::In));
        assert!(tokens.contains(&Token::LBracket));
        assert!(tokens.contains(&Token::RBracket));
    }
}
