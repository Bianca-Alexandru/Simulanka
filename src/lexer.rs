#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Environment, Species, Evolve, Mutate, Fitness, ResultKeyword, Visualize,
    Define, Routine, Spawn, At, Random, Unique,
    If, Else, While, For, In, IntType, Return, Print,
    Identifier(String),
    Number(i32),
    StringLiteral(String),
    LBrace, RBrace, LParen, RParen, LBracket, RBracket,
    Colon, SemiColon, Comma, Equal, Plus, Minus, Star, Slash,
    Greater, Less, GreaterEqual, LessEqual, DoubleEqual, NotEqual, Percent, Dot,
    And, Or,
    EOF,
}

pub fn lexer(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\r' | '\n' | '\t' => { chars.next(); }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    tokens.push(Token::NotEqual);
                    chars.next();
                }
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                while let Some(&cc) = chars.peek() {
                    if cc == '"' { chars.next(); break; }
                    s.push(cc);
                    chars.next();
                }
                tokens.push(Token::StringLiteral(s));
            }
            '{' => { tokens.push(Token::LBrace); chars.next(); }
            '}' => { tokens.push(Token::RBrace); chars.next(); }
            '(' => { tokens.push(Token::LParen); chars.next(); }
            ')' => { tokens.push(Token::RParen); chars.next(); }
            '[' => { tokens.push(Token::LBracket); chars.next(); }
            ']' => { tokens.push(Token::RBracket); chars.next(); }
            ':' => { tokens.push(Token::Colon); chars.next(); }
            ';' => { tokens.push(Token::SemiColon); chars.next(); }
            ',' => { tokens.push(Token::Comma); chars.next(); }
            '.' => { tokens.push(Token::Dot); chars.next(); }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    tokens.push(Token::DoubleEqual);
                    chars.next();
                } else {
                    tokens.push(Token::Equal);
                }
            }
            '+' => { tokens.push(Token::Plus); chars.next(); }
            '-' => { tokens.push(Token::Minus); chars.next(); }
            '*' => { tokens.push(Token::Star); chars.next(); }
            '/' => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    while let Some(&cc) = chars.peek() {
                        if cc == '\n' { break; }
                        chars.next();
                    }
                } else {
                    tokens.push(Token::Slash);
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    tokens.push(Token::GreaterEqual);
                    chars.next();
                } else {
                    tokens.push(Token::Greater);
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    tokens.push(Token::LessEqual);
                    chars.next();
                } else {
                    tokens.push(Token::Less);
                }
            }
            '%' => { tokens.push(Token::Percent); chars.next(); }
            '@' => { tokens.push(Token::At); chars.next(); }
            '|' => {
                chars.next();
                if chars.peek() == Some(&'|') {
                    tokens.push(Token::Or);
                    chars.next();
                }
            }
            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') {
                    tokens.push(Token::And);
                    chars.next();
                }
            }
            '0'..='9' => {
                let mut num_str = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() {
                        num_str.push(d);
                        chars.next();
                    } else { break; }
                }
                tokens.push(Token::Number(num_str.parse().unwrap()));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                while let Some(&l) = chars.peek() {
                    if l.is_alphanumeric() || l == '_' {
                        ident.push(l);
                        chars.next();
                    } else { break; }
                }
                match ident.to_uppercase().as_str() {
                    "ENVIRONMENT" => tokens.push(Token::Environment),
                    "SPECIES" => tokens.push(Token::Species),
                    "EVOLVE" => tokens.push(Token::Evolve),
                    "MUTATE" => tokens.push(Token::Mutate),
                    "FITNESS" => tokens.push(Token::Fitness),
                    "RESULT" => tokens.push(Token::ResultKeyword),
                    "VISUALIZE" => tokens.push(Token::Visualize),
                    "DEFINE" => tokens.push(Token::Define),
                    "ROUTINE" => tokens.push(Token::Routine),
                    "SPAWN" => tokens.push(Token::Spawn),
                    "AT" => tokens.push(Token::At),
                    "RANDOM" => tokens.push(Token::Random),
                    "UNIQUE" => tokens.push(Token::Unique),
                    "IF" => tokens.push(Token::If),
                    "ELSE" => tokens.push(Token::Else),
                    "WHILE" => tokens.push(Token::While),
                    "FOR" => tokens.push(Token::For),
                    "IN" => tokens.push(Token::In),
                    "INT" => tokens.push(Token::IntType),
                    "RETURN" => tokens.push(Token::Return),
                    "PRINT" => tokens.push(Token::Print),
                    _ => tokens.push(Token::Identifier(ident)),
                }
            }
            _ => { chars.next(); }
        }
    }
    tokens.push(Token::EOF);
    tokens
}