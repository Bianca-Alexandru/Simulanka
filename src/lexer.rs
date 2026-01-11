#[derive(Debug, PartialEq, Clone)]
//just recognizes tokens nothing notable or complicated
pub enum TokenKind {
    Environment, Species, Evolve, Mutate, Fitness, Visualize,
    Routine, Spawn, At, Random,
    If, Else, While, For, In, Return, Print,
    True, False,
    Identifier(String),
    Number(i32),
    StringLiteral(String),
    LBrace, RBrace, LParen, RParen, LBracket, RBracket,
    Colon, SemiColon, Comma, Equal, Plus, Minus, Star, Slash,
    Greater, Less, GreaterEqual, LessEqual, DoubleEqual, NotEqual, Percent, Dot,
    And, Or,
    EOF,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

pub fn lexer(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut line = 1;
    let mut col = 1;

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\r' | '\t' => { 
                col += 1;
                chars.next(); 
            }
            '\n' => {
                line += 1;
                col = 1;
                chars.next();
            }
            '!' => {
                let start_col = col;
                chars.next(); col += 1;
                if chars.peek() == Some(&'=') {
                    tokens.push(Token { kind: TokenKind::NotEqual, line, col: start_col });
                    chars.next(); col += 1;
                }
            }
            '"' => {
                let start_col = col;
                chars.next(); col += 1;
                let mut s = String::new();
                while let Some(&cc) = chars.peek() {
                    if cc == '"' { chars.next(); col += 1; break; }
                    s.push(cc);
                    chars.next(); col += 1;
                }
                tokens.push(Token { kind: TokenKind::StringLiteral(s), line, col: start_col });
            }
            '{' => { tokens.push(Token { kind: TokenKind::LBrace, line, col }); chars.next(); col += 1; }
            '}' => { tokens.push(Token { kind: TokenKind::RBrace, line, col }); chars.next(); col += 1; }
            '(' => { tokens.push(Token { kind: TokenKind::LParen, line, col }); chars.next(); col += 1; }
            ')' => { tokens.push(Token { kind: TokenKind::RParen, line, col }); chars.next(); col += 1; }
            '[' => { tokens.push(Token { kind: TokenKind::LBracket, line, col }); chars.next(); col += 1; }
            ']' => { tokens.push(Token { kind: TokenKind::RBracket, line, col }); chars.next(); col += 1; }
            ':' => { tokens.push(Token { kind: TokenKind::Colon, line, col }); chars.next(); col += 1; }
            ';' => { tokens.push(Token { kind: TokenKind::SemiColon, line, col }); chars.next(); col += 1; }
            ',' => { tokens.push(Token { kind: TokenKind::Comma, line, col }); chars.next(); col += 1; }
            '.' => { tokens.push(Token { kind: TokenKind::Dot, line, col }); chars.next(); col += 1; }
            '=' => {
                let start_col = col;
                chars.next(); col += 1;
                if chars.peek() == Some(&'=') {
                    tokens.push(Token { kind: TokenKind::DoubleEqual, line, col: start_col });
                    chars.next(); col += 1;
                } else {
                    tokens.push(Token { kind: TokenKind::Equal, line, col: start_col });
                }
            }
            '+' => { tokens.push(Token { kind: TokenKind::Plus, line, col }); chars.next(); col += 1; }
            '-' => { tokens.push(Token { kind: TokenKind::Minus, line, col }); chars.next(); col += 1; }
            '*' => { tokens.push(Token { kind: TokenKind::Star, line, col }); chars.next(); col += 1; }
            '/' => {
                let start_col = col;
                chars.next(); col += 1;
                if chars.peek() == Some(&'/') {
                    while let Some(&cc) = chars.peek() {
                        if cc == '\n' { break; }
                        chars.next(); col += 1;
                    }
                } else {
                    tokens.push(Token { kind: TokenKind::Slash, line, col: start_col });
                }
            }
            '>' => {
                let start_col = col;
                chars.next(); col += 1;
                if chars.peek() == Some(&'=') {
                    tokens.push(Token { kind: TokenKind::GreaterEqual, line, col: start_col });
                    chars.next(); col += 1;
                } else {
                    tokens.push(Token { kind: TokenKind::Greater, line, col: start_col });
                }
            }
            '<' => {
                let start_col = col;
                chars.next(); col += 1;
                if chars.peek() == Some(&'=') {
                    tokens.push(Token { kind: TokenKind::LessEqual, line, col: start_col });
                    chars.next(); col += 1;
                } else {
                    tokens.push(Token { kind: TokenKind::Less, line, col: start_col });
                }
            }
            '%' => { tokens.push(Token { kind: TokenKind::Percent, line, col }); chars.next(); col += 1; }
            '@' => { tokens.push(Token { kind: TokenKind::At, line, col }); chars.next(); col += 1; }
            '|' => {
                chars.next(); col += 1;
                if chars.peek() == Some(&'|') {
                    tokens.push(Token { kind: TokenKind::Or, line, col: col - 1 });
                    chars.next(); col += 1;
                }
            }
            '&' => {
                chars.next(); col += 1;
                if chars.peek() == Some(&'&') {
                    tokens.push(Token { kind: TokenKind::And, line, col: col - 1 });
                    chars.next(); col += 1;
                }
            }
            '0'..='9' => {
                let start_col = col;
                let mut num_str = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() {
                        num_str.push(d);
                        chars.next(); col += 1;
                    } else { break; }
                }
                tokens.push(Token { kind: TokenKind::Number(num_str.parse().unwrap()), line, col: start_col });
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start_col = col;
                let mut ident = String::new();
                while let Some(&l) = chars.peek() {
                    if l.is_alphanumeric() || l == '_' {
                        ident.push(l);
                        chars.next(); col += 1;
                    } else { break; }
                }
                let kind = match ident.to_uppercase().as_str() {
                    "ENVIRONMENT" => TokenKind::Environment,
                    "SPECIES" => TokenKind::Species,
                    "EVOLVE" => TokenKind::Evolve,
                    "MUTATE" => TokenKind::Mutate,
                    "FITNESS" => TokenKind::Fitness,
                    "VISUALIZE" => TokenKind::Visualize,
                    "ROUTINE" => TokenKind::Routine,
                    "SPAWN" => TokenKind::Spawn,
                    "AT" => TokenKind::At,
                    "RANDOM" => TokenKind::Random,
                    "IF" => TokenKind::If,
                    "ELSE" => TokenKind::Else,
                    "WHILE" => TokenKind::While,
                    "FOR" => TokenKind::For,
                    "IN" => TokenKind::In,
                    "RETURN" => TokenKind::Return,
                    "PRINT" => TokenKind::Print,
                    "TRUE" => TokenKind::True,
                    "FALSE" => TokenKind::False,
                    _ => TokenKind::Identifier(ident),
                };
                tokens.push(Token { kind, line, col: start_col });
            }
            _ => { chars.next(); col += 1; }
        }
    }
    tokens.push(Token { kind: TokenKind::EOF, line, col });
    tokens
}