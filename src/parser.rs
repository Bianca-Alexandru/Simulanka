use crate::lexer::{Token, TokenKind};
use crate::types::*;
use std::collections::HashMap;
// parses tokens
//also checks that all blocks are present 
// blocks environment, evolve dont have code just parameters
// helper struct for parsing environment settings
#[derive(Default)]
pub struct EnvDef {
    pub width: i32,
    pub height: i32,
    pub steps: i32,
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if t.kind != TokenKind::EOF { self.pos += 1; }
        t
    }

    fn error(&self, msg: &str) -> String {
        let t = self.peek();
        format!("Error at line {}:{}: {}", t.line, t.col, msg)
    }

    fn expect(&mut self, expected: TokenKind) -> Result<(), String> {
        if self.peek().kind == expected {
            self.advance();
            Ok(())
        } else {
            Err(self.error(&format!("Expected {:?}, found {:?}", expected, self.peek().kind)))
        }
    }

    //the entry point- parses the whole file into the program struct
    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut program = Program::default();
        let mut found_environment = false;
        let mut found_species = false;
        let mut found_evolve = false;
        let mut found_fitness = false;
        let mut found_mutate = false;
        let mut found_spawn = false;

        while self.peek().kind != TokenKind::EOF {
            match self.peek().kind {
                TokenKind::Environment => {
                    let env = self.parse_env_block()?;
                    program.env_width = env.width;
                    program.env_height = env.height;
                    program.env_steps = env.steps;
                    found_environment = true;
                }
                TokenKind::Species => {
                    self.parse_species_block(&mut program)?;
                    found_species = true;
                }
                TokenKind::Evolve => {
                    self.parse_evolve_block(&mut program)?;
                    found_evolve = true;
                }
                TokenKind::Fitness => {
                    program.fitness_block = self.parse_fitness_block()?;
                    found_fitness = true;
                }
                TokenKind::Mutate => {
                    program.mutations_block = self.parse_mutate_block()?;
                    found_mutate = true;
                }
                TokenKind::Visualize => {
                    self.advance(); //bc it uses basic parsing instead of specialized
                    program.visualize_block = self.parse_block()?;
                    program.visualize = true;
                }
                TokenKind::Spawn => {
                    program.spawns_block = self.parse_spawn_block()?;
                    found_spawn = true;
                }
                _ => { 
                    self.advance();
                }
            }
        }

        if !found_environment {
            return Err("Syntax Error: Missing obligatory ENVIRONMENT block".to_string());
        }
        if !found_species {
            return Err("Syntax Error: Missing obligatory SPECIES block".to_string());
        }
        if !found_evolve {
             return Err("Syntax Error: Missing obligatory EVOLVE block".to_string());
        }
        if !found_fitness {
            return Err("Syntax Error: Missing obligatory FITNESS block".to_string());
        }
        if !found_mutate {
             return Err("Syntax Error: Missing obligatory MUTATE block".to_string());
        }
        if !found_spawn {
             return Err("Syntax Error: Missing obligatory SPAWN block".to_string());
        }

        Ok(program)
    }

    //specific parsers for each block
    fn parse_env_block(&mut self) -> Result<EnvDef, String> {
        self.expect(TokenKind::Environment)?;
        self.expect(TokenKind::LBrace)?;
        let mut env = EnvDef { width: 50, height: 50, steps: 10 };
        while self.peek().kind != TokenKind::RBrace {
            let key = match self.peek().kind {
                TokenKind::Identifier(ref n) => n.clone(),
                _ => return Err(self.error("Expected Key in ENVIRONMENT")),
            };
            self.advance();
            self.expect(TokenKind::Colon)?;
            match key.as_str() {
                "width" => if let TokenKind::Number(v) = self.advance().kind { env.width = v; },
                "height" => if let TokenKind::Number(v) = self.advance().kind { env.height = v; },
                "steps" => if let TokenKind::Number(v) = self.advance().kind { env.steps = v; },
                _ => { self.advance(); }
            }
            if self.peek().kind == TokenKind::Comma { self.advance(); }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(env)
    }

    fn parse_species_block(&mut self, program: &mut Program) -> Result<(), String> {
        self.expect(TokenKind::Species)?;
        self.expect(TokenKind::LBrace)?;

        while self.peek().kind != TokenKind::RBrace {
            if self.peek().kind == TokenKind::Routine {
                let routine = self.parse_routine_def()?;
                program.routines_block.insert(routine.name.clone(), routine);
                if self.peek().kind == TokenKind::Comma { self.advance(); }
                continue;
            }

            let name = if let TokenKind::Identifier(n) = self.advance().kind { n } else { return Err(self.error("Expected species name")); };
            
            self.expect(TokenKind::LBrace)?;
            let mut props = HashMap::new();
            let mut routine_call = String::new();
            
            while self.peek().kind != TokenKind::RBrace {
                let prop_key = match self.advance().kind {
                    TokenKind::Identifier(n) => n,
                    TokenKind::Routine => "routine".into(),
                    _ => return Err(self.error("Property Key")),
                };
                self.expect(TokenKind::Colon)?;
                let val = self.parse_exp()?;
                if self.peek().kind == TokenKind::SemiColon { self.advance(); }
                if self.peek().kind == TokenKind::Comma { self.advance(); }

                if prop_key == "routine" { 
                    if let Exp::Var(v, _) = val { routine_call = v; }
                } 
                else { props.insert(prop_key, val); }
            }
            self.expect(TokenKind::RBrace)?;
            program.species_block.insert(name.clone(), SpeciesDef { properties: props, routine_call });
            if self.peek().kind == TokenKind::Comma { self.advance(); }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(())
    }

    fn parse_spawn_block(&mut self) -> Result<Vec<Command>, String> {
        self.expect(TokenKind::Spawn)?;
        self.parse_block()
    }

    fn parse_fitness_block(&mut self) -> Result<FitnessBlock, String> {
        self.expect(TokenKind::Fitness)?;
        let commands = self.parse_block()?;
        Ok(FitnessBlock { commands })
    }

    fn parse_mutate_block(&mut self) -> Result<Vec<MutationRule>, String> {
        self.expect(TokenKind::Mutate)?;
        self.expect(TokenKind::LBrace)?;
        let mut rules = Vec::new();
        while self.peek().kind != TokenKind::RBrace {
            let key = if let TokenKind::Identifier(n) = self.advance().kind { n } else { return Err(self.error("Expected key")); };
            self.expect(TokenKind::Colon)?;
            
            let body = self.parse_block()?;
            rules.push(MutationRule { probability: 1.0, action: key, body: Some(body) });
            if self.peek().kind == TokenKind::Comma { self.advance(); }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(rules)
    }

    fn parse_evolve_block(&mut self, program: &mut Program) -> Result<(), String> {
        self.expect(TokenKind::Evolve)?;
        self.expect(TokenKind::LBrace)?;
        while self.peek().kind != TokenKind::RBrace {
            let key = if let TokenKind::Identifier(n) = self.advance().kind { n } else { return Err(self.error("Expected key")); };
            self.expect(TokenKind::Colon)?;
            match key.as_str() {
                "generations" => if let TokenKind::Number(n) = self.advance().kind { program.evolve_block.generations = n; },
                "instances" => if let TokenKind::Number(n) = self.advance().kind { program.evolve_block.instances = n; },
                _ => { self.advance(); }
            }
            if self.peek().kind == TokenKind::Comma { self.advance(); }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(())
    }

    fn parse_routine_def(&mut self) -> Result<RoutineDef, String> {
        self.expect(TokenKind::Routine)?;
        let name = if let TokenKind::Identifier(n) = self.advance().kind { n } else { return Err(self.error("Name")); };
        let body = self.parse_block()?;
        Ok(RoutineDef { name, body })
    }

    //non specific parsers
    pub fn parse_block(&mut self) -> Result<Vec<Command>, String> {
        self.expect(TokenKind::LBrace)?;
        let mut cmds = Vec::new();
        while self.peek().kind != TokenKind::RBrace {
            cmds.push(self.parse_command()?);
        }
        self.expect(TokenKind::RBrace)?;
        Ok(cmds)
    }

    pub fn parse_command(&mut self) -> Result<Command, String> {
        let line = self.peek().line;
        match self.peek().kind {
            TokenKind::If => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let cond = self.parse_bexp()?;
                self.expect(TokenKind::RParen)?;
                let then_b = self.parse_block()?;
                let mut else_b = None;
                if self.peek().kind == TokenKind::Else {
                    self.advance();
                    if self.peek().kind == TokenKind::If {
                        else_b = Some(vec![self.parse_command()?]);
                    } else {
                        else_b = Some(self.parse_block()?);
                    }
                }
                Ok(Command::If { condition: cond, then_block: then_b, else_block: else_b, line })
            }
            TokenKind::While => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let cond = self.parse_bexp()?;
                self.expect(TokenKind::RParen)?;
                let body = self.parse_block()?;
                Ok(Command::While { condition: cond, body, line })
            }
            TokenKind::For => {
                self.advance();
                let var = if let TokenKind::Identifier(n) = self.advance().kind { n } else { return Err(self.error("Expected var")); };
                self.expect(TokenKind::In)?;
                let collection = match self.advance().kind {
                    TokenKind::Identifier(n) => n,
                    TokenKind::Environment => "environment".to_string(),
                    _ => return Err(self.error("Expected collection")),
                };
                let body = self.parse_block()?;
                Ok(Command::For { var, collection, body, line })
            }
            TokenKind::Return => {
                self.advance();
                let exp = self.parse_exp()?;
                if self.peek().kind == TokenKind::SemiColon { self.advance(); }
                Ok(Command::Return(exp, line))
            }
            TokenKind::Print => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let mut exps = Vec::new();
                while self.peek().kind != TokenKind::RParen {
                    exps.push(self.parse_exp()?);
                    if self.peek().kind == TokenKind::Comma { self.advance(); }
                }
                self.expect(TokenKind::RParen)?;
                if self.peek().kind == TokenKind::SemiColon { self.advance(); }
                Ok(Command::Print(exps, line))
            }
            TokenKind::Spawn => {
                self.advance();
                let species = if let TokenKind::Identifier(n) = self.advance().kind { n } else { return Err(self.error("Expected species")); };
                self.expect(TokenKind::At)?;
                self.expect(TokenKind::LParen)?;
                let x = self.parse_exp()?;
                self.expect(TokenKind::Comma)?;
                let y = self.parse_exp()?;
                self.expect(TokenKind::RParen)?;
                if self.peek().kind == TokenKind::SemiColon { self.advance(); }
                Ok(Command::Spawn { species, x, y, line })
            }
            _ => {
                let exp = self.parse_exp()?;
                if self.peek().kind == TokenKind::Equal {
                    self.advance();
                    let value = self.parse_exp()?;
                    if self.peek().kind == TokenKind::SemiColon { self.advance(); }
                    Ok(Command::Assign { target: exp, value, line })
                } else {
                    if self.peek().kind == TokenKind::SemiColon { self.advance(); }
                    Ok(Command::Exp(exp, line))
                }
            }
        }
    }

    pub fn parse_exp(&mut self) -> Result<Exp, String> {
        self.parse_sum()
    }

    fn parse_sum(&mut self) -> Result<Exp, String> {
        let mut left = self.parse_term()?;
        while matches!(self.peek().kind, TokenKind::Plus | TokenKind::Minus) {
            let tok = self.advance();
            let op = match tok.kind {
                TokenKind::Plus => "+".into(),
                TokenKind::Minus => "-".into(),
                _ => unreachable!(),
            };
            let right = self.parse_term()?;
            let line = tok.line;
            left = Exp::BinaryOp(Box::new(left), op, Box::new(right), line);
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Exp, String> {
        let mut left = self.parse_primary()?;
        while matches!(self.peek().kind, TokenKind::Star | TokenKind::Slash | TokenKind::Percent) {
            let tok = self.advance();
            let op = match tok.kind {
                TokenKind::Star => "*".into(),
                TokenKind::Slash => "/".into(),
                TokenKind::Percent => "%".into(),
                _ => unreachable!(),
            };
            let right = self.parse_primary()?;
            let line = tok.line;
            left = Exp::BinaryOp(Box::new(left), op, Box::new(right), line);
        }
        Ok(left)
    }

    // convert a token to a field name (handles keywords that can also be field names)
    fn token_to_field_name(&self, token: &Token) -> Option<String> {
        match &token.kind {
            TokenKind::Identifier(name) => Some(name.clone()),
            TokenKind::Species => Some("species".to_string()),
            TokenKind::Spawn => Some("spawn".to_string()),
            TokenKind::Routine => Some("routine".to_string()),
            TokenKind::Fitness => Some("fitness".to_string()),
            _ => None,
        }
    }

    fn parse_primary(&mut self) -> Result<Exp, String> {
        // handle negative numbers: -5 becomes (0 - 5)
        if self.peek().kind == TokenKind::Minus {
            let tok = self.advance();
            let right = self.parse_primary()?;
            let line = tok.line;
            let mut node = Exp::BinaryOp(Box::new(Exp::Int(0, line)), "-".into(), Box::new(right), line);
            node = self.parse_dot_and_index(node)?;
            return Ok(node);
        }

        let t = self.advance();
        let line = t.line;
        let mut node = match t.kind {
            TokenKind::Number(v) => Exp::Int(v, line),
            TokenKind::StringLiteral(s) => Exp::StringLiteral(s, line),
            TokenKind::True => Exp::Bool(true, line),
            TokenKind::False => Exp::Bool(false, line),
            TokenKind::LBracket => {
                let mut exps = Vec::new();
                while self.peek().kind != TokenKind::RBracket {
                    exps.push(self.parse_exp()?);
                    if self.peek().kind == TokenKind::Comma { self.advance(); }
                }
                self.expect(TokenKind::RBracket)?;
                Exp::List(exps, line)
            }
            TokenKind::LParen => {
                let exp = self.parse_exp()?;
                self.expect(TokenKind::RParen)?;
                exp
            }
            TokenKind::Identifier(_) | TokenKind::Random | TokenKind::Environment => {
                let name = match t.kind {
                    TokenKind::Identifier(s) => s,
                    TokenKind::Random => "random".into(),
                    TokenKind::Environment => "environment".into(),
                    _ => unreachable!(),
                };
                if self.peek().kind == TokenKind::LParen {
                    self.advance();
                    let mut args = Vec::new();
                    while self.peek().kind != TokenKind::RParen {
                        args.push(self.parse_exp()?);
                        if self.peek().kind == TokenKind::Comma { self.advance(); }
                    }
                    self.expect(TokenKind::RParen)?;
                    Exp::Call(name, args, line)
                } else {
                    Exp::Var(name, line)
                }
            },
            _ => {
                return Err(self.error(&format!("Expected exp, found {:?}", t.kind)));
            }
        };
        
        node = self.parse_dot_and_index(node)?;
        Ok(node)
    }

    // parse .field and [index] access after an expression
    fn parse_dot_and_index(&mut self, mut node: Exp) -> Result<Exp, String> {
        while matches!(self.peek().kind, TokenKind::Dot | TokenKind::LBracket) {
            let tok = self.advance();
            let line = tok.line;
            if tok.kind == TokenKind::Dot {
                let field_token = self.advance();
                let field_name = self.token_to_field_name(&field_token)
                    .ok_or_else(|| self.error(&format!("Expected field name after '.', found {:?}", field_token.kind)))?;
                node = Exp::Dot(Box::new(node), field_name, line);
            } else {
                let idx = self.parse_exp()?;
                self.expect(TokenKind::RBracket)?;
                node = Exp::Index(Box::new(node), Box::new(idx), line);
            }
        }
        Ok(node)
    }

    pub fn parse_bexp(&mut self) -> Result<BExp, String> {
        let mut left = self.parse_and_exp()?;
        while self.peek().kind == TokenKind::Or {
            self.advance();
            let right = self.parse_and_exp()?;
            left = BExp::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and_exp(&mut self) -> Result<BExp, String> {
        let mut left = self.parse_primary_bexp()?;
        while self.peek().kind == TokenKind::And {
            self.advance();
            let right = self.parse_primary_bexp()?;
            left = BExp::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_primary_bexp(&mut self) -> Result<BExp, String> {
        let left = self.parse_exp()?;
        let op = self.advance().kind;
        let right = self.parse_exp()?;
        match op {
            TokenKind::Greater => Ok(BExp::Greater(left, right)),
            TokenKind::Less => Ok(BExp::Less(left, right)),
            TokenKind::GreaterEqual => Ok(BExp::GreaterEqual(left, right)),
            TokenKind::LessEqual => Ok(BExp::LessEqual(left, right)),
            TokenKind::DoubleEqual => Ok(BExp::Equal(left, right)),
            TokenKind::NotEqual => Ok(BExp::NotEqual(left, right)),
            _ => Err(self.error(&format!("Expected comparison operator, found {:?}", op))),
        }
    }
}