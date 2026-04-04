use crate::query::lexer::{Token, tokenize};

/// AST node for query expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Comparison {
        field: String,
        is_frontmatter: bool,
        op: Op,
        value: Value,
    },
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    FuncCall {
        name: String,
        args: Vec<Expr>,
    },
    /// Match all (no filter)
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
    Contains,
    Matches,
    In,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    List(Vec<Value>),
}

/// Parse a query expression string into an AST.
pub fn parse(input: &str) -> Result<Expr, String> {
    let tokens = tokenize(input)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
    };
    let expr = parser.parse_or()?;
    if parser.current() != &Token::End {
        return Err(format!("Unexpected token: {:?}", parser.current()));
    }
    Ok(expr)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn current(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::End)
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens.get(self.pos).cloned().unwrap_or(Token::End);
        self.pos += 1;
        t
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let left = self.parse_and()?;
        while matches!(self.current(), Token::Or) {
            self.advance();
            let right = self.parse_and()?;
            return Ok(Expr::Or(Box::new(left), Box::new(right)));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let left = self.parse_comparison()?;
        while matches!(self.current(), Token::And) {
            self.advance();
            let right = self.parse_comparison()?;
            return Ok(Expr::And(Box::new(left), Box::new(right)));
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        // Handle function calls: name(args)
        if let Token::Field(name) = self.current().clone() {
            // Peek ahead to see if next is LParen
            if self.pos + 1 < self.tokens.len() && matches!(self.tokens[self.pos + 1], Token::LParen) {
                self.advance(); // consume name
                self.advance(); // consume (
                let mut args = Vec::new();
                while !matches!(self.current(), Token::RParen) {
                    args.push(self.parse_value_expr()?);
                    if matches!(self.current(), Token::Comma) {
                        self.advance();
                    }
                }
                self.advance(); // consume )
                return Ok(Expr::FuncCall { name, args });
            }
        }

        // field op value
        let (field, is_frontmatter) = match self.current().clone() {
            Token::Field(name) => {
                self.advance();
                (name, false)
            }
            Token::FmField(name) => {
                self.advance();
                (name, true)
            }
            t => return Err(format!("Expected field name, got {:?}", t)),
        };

        let op = match self.current() {
            Token::Eq => Op::Eq,
            Token::Neq => Op::Neq,
            Token::Gt => Op::Gt,
            Token::Lt => Op::Lt,
            Token::Gte => Op::Gte,
            Token::Lte => Op::Lte,
            Token::Contains => Op::Contains,
            Token::Matches => Op::Matches,
            Token::In => Op::In,
            t => return Err(format!("Expected operator, got {:?}", t)),
        };
        self.advance();

        let value = self.parse_value()?;

        Ok(Expr::Comparison {
            field,
            is_frontmatter,
            op,
            value,
        })
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match self.current().clone() {
            Token::String(s) => {
                self.advance();
                Ok(Value::String(s))
            }
            Token::Number(n) => {
                self.advance();
                Ok(Value::Number(n))
            }
            Token::Boolean(b) => {
                self.advance();
                Ok(Value::Boolean(b))
            }
            Token::LBracket => {
                self.advance();
                let mut items = Vec::new();
                while !matches!(self.current(), Token::RBracket) {
                    items.push(self.parse_value()?);
                    if matches!(self.current(), Token::Comma) {
                        self.advance();
                    }
                }
                self.advance(); // consume ]
                Ok(Value::List(items))
            }
            t => Err(format!("Expected value, got {:?}", t)),
        }
    }

    fn parse_value_expr(&mut self) -> Result<Expr, String> {
        // For function args, just parse as a string literal comparison
        match self.current().clone() {
            Token::String(s) => {
                self.advance();
                Ok(Expr::Comparison {
                    field: String::new(),
                    is_frontmatter: false,
                    op: Op::Eq,
                    value: Value::String(s),
                })
            }
            t => Err(format!("Expected string in function arg, got {:?}", t)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_comparison() {
        let expr = parse("status = 'draft'").unwrap();
        assert_eq!(expr, Expr::Comparison {
            field: "status".to_string(),
            is_frontmatter: false,
            op: Op::Eq,
            value: Value::String("draft".to_string()),
        });
    }

    #[test]
    fn test_parse_and() {
        let expr = parse("status = 'draft' AND tags contains 'rust'").unwrap();
        match expr {
            Expr::And(_, _) => {}
            e => panic!("Expected And, got {:?}", e),
        }
    }

    #[test]
    fn test_parse_frontmatter() {
        let expr = parse("frontmatter.date > '2024-01-01'").unwrap();
        match expr {
            Expr::Comparison { is_frontmatter: true, .. } => {}
            e => panic!("Expected frontmatter comparison, got {:?}", e),
        }
    }

    #[test]
    fn test_parse_list_value() {
        let expr = parse("status in ['draft', 'review']").unwrap();
        match expr {
            Expr::Comparison { value: Value::List(items), .. } => {
                assert_eq!(items.len(), 2);
            }
            e => panic!("Expected list value, got {:?}", e),
        }
    }
}
