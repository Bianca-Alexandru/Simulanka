use crate::lexer::Token;
use crate::{Exp, BExp, Command, Program, SpeciesDef, RoutineDef, FitnessDef, MutationRule, EnvDef};
use std::collections::HashMap;

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
        if t != Token::EOF { self.pos += 1; }
        t
    }

    fn expect(&mut self, expected: Token) -> Result<(), String> {
        let t = self.advance();
        if t == expected { Ok(()) } 
        else { Err(format!("Expected {:?}, found {:?}", expected, t)) }
    }

    // The entry point: Parses the whole file into the Program struct
    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut program = Program::default();

        while *self.peek() != Token::EOF {
            match self.peek() {
                Token::Environment => {
                    let env = self.parse_env_block()?;
                    program.env_width = env.width;
                    program.env_height = env.height;
                    program.env_steps = env.steps;
                    program.visualize = env.visualize;
                }
                Token::Species => {
                    self.parse_species_block(&mut program)?;
                }
                Token::Evolve => {
                    self.parse_evolve_block(&mut program)?;
                }
                Token::Fitness => {
                    program.fitness_block = Some(self.parse_fitness_block()?);
                }
                Token::Mutate => {
                    program.mutations_block = self.parse_mutate_block()?;
                }
                Token::ResultKeyword => {
                    self.advance();
                    program.result_block = self.parse_block()?;
                }
                Token::Visualize => {
                    self.advance();
                    program.visualize_block = self.parse_block()?;
                }
                Token::Spawn => {
                    program.spawns_block = self.parse_spawn_block()?;
                }
                _ => { 
                    self.advance();
                }
            }
        }
        Ok(program)
    }

    fn parse_env_block(&mut self) -> Result<EnvDef, String> {
        self.expect(Token::Environment)?;
        self.expect(Token::LBrace)?;
        let mut env = EnvDef { width: 50, height: 50, steps: 10, visualize: false };
        while *self.peek() != Token::RBrace {
            let key = match self.advance() {
                Token::Identifier(n) => n,
                Token::Visualize => "visualize".into(),
                _ => return Err("Expected Key in ENVIRONMENT".into()),
            };
            self.expect(Token::Colon)?;
            match key.as_str() {
                "width" => if let Token::Number(v) = self.advance() { env.width = v; },
                "height" => if let Token::Number(v) = self.advance() { env.height = v; },
                "steps" => if let Token::Number(v) = self.advance() { env.steps = v; },
                "visualize" => {
                    if let Token::Identifier(v) = self.advance() {
                        if v == "true" { env.visualize = true; }
                    }
                }
                _ => { self.advance(); }
            }
            if *self.peek() == Token::Comma { self.advance(); }
        }
        self.expect(Token::RBrace)?;
        Ok(env)
    }

    fn parse_species_block(&mut self, program: &mut Program) -> Result<(), String> {
        self.expect(Token::Species)?;
        
        // Support both SPECIES { name: X { ... } } and SPECIES X { ... }
        if *self.peek() == Token::LBrace {
            self.advance();
            while *self.peek() != Token::RBrace {
                if *self.peek() == Token::Routine {
                    let routine = self.parse_routine_def()?;
                    program.routines_block.insert(routine.name.clone(), routine);
                    if *self.peek() == Token::Comma { self.advance(); }
                    continue;
                }

                let key = if let Token::Identifier(n) = self.advance() { n } else { return Err("Expected 'name' or species name".into()); };
                let name = if key == "name" {
                    self.expect(Token::Colon)?;
                    if let Token::Identifier(n) = self.advance() { n } else { return Err("Expected species name".into()); }
                } else {
                    key
                };
                
                self.expect(Token::LBrace)?;
                let mut props = HashMap::new();
                let mut routine_call = String::new();
                
                while *self.peek() != Token::RBrace {
                    let prop_key = match self.advance() {
                        Token::Identifier(n) => n,
                        Token::Routine => "routine".into(),
                        _ => return Err("Property Key".into()),
                    };
                    self.expect(Token::Colon)?;
                    let val = self.parse_exp()?;
                    if *self.peek() == Token::SemiColon { self.advance(); }
                    if *self.peek() == Token::Comma { self.advance(); }

                    if prop_key == "routine" { 
                        if let Exp::Var(v) = val { routine_call = v; }
                    } 
                    else { props.insert(prop_key, val); }
                }
                self.expect(Token::RBrace)?;
                program.species_block.insert(name.clone(), SpeciesDef { name, properties: props, routine_call, routine_arg: "".into() });
                if *self.peek() == Token::Comma { self.advance(); }
            }
            self.expect(Token::RBrace)?;
        } else {
            // Single species definition: SPECIES X { ... }
            let name = if let Token::Identifier(n) = self.advance() { n } else { return Err("Expected species name".into()); };
            self.expect(Token::LBrace)?;
            let mut props = HashMap::new();
            let mut routine_call = String::new();
            while *self.peek() != Token::RBrace {
                let prop_key = match self.advance() {
                    Token::Identifier(n) => n,
                    Token::Routine => "routine".into(),
                    _ => return Err("Property Key".into()),
                };
                self.expect(Token::Colon)?;
                let val = self.parse_exp()?;
                if *self.peek() == Token::SemiColon { self.advance(); }
                if *self.peek() == Token::Comma { self.advance(); }

                if prop_key == "routine" { 
                    if let Exp::Var(v) = val { routine_call = v; }
                } 
                else { props.insert(prop_key, val); }
            }
            self.expect(Token::RBrace)?;
            program.species_block.insert(name.clone(), SpeciesDef { name, properties: props, routine_call, routine_arg: "".into() });
        }
        Ok(())
    }

    fn parse_spawn_block(&mut self) -> Result<Vec<Command>, String> {
        self.expect(Token::Spawn)?;
        self.parse_block()
    }

    fn parse_fitness_block(&mut self) -> Result<FitnessDef, String> {
        self.expect(Token::Fitness)?;
        let commands = self.parse_block()?;
        Ok(FitnessDef { expressions: HashMap::new(), commands })
    }

    fn parse_mutate_block(&mut self) -> Result<Vec<MutationRule>, String> {
        self.expect(Token::Mutate)?;
        self.expect(Token::LBrace)?;
        let mut rules = Vec::new();
        while *self.peek() != Token::RBrace {
            let key = if let Token::Identifier(n) = self.advance() { n } else { return Err("Expected key".into()); };
            self.expect(Token::Colon)?;
            
            let body = self.parse_block()?;
            rules.push(MutationRule { probability: 1.0, action: key, body: Some(body) });
            if *self.peek() == Token::Comma { self.advance(); }
        }
        self.expect(Token::RBrace)?;
        Ok(rules)
    }

    fn parse_evolve_block(&mut self, program: &mut Program) -> Result<(), String> {
        self.expect(Token::Evolve)?;
        self.expect(Token::LBrace)?;
        while *self.peek() != Token::RBrace {
            let key = if let Token::Identifier(n) = self.advance() { n } else { return Err("Expected key".into()); };
            self.expect(Token::Colon)?;
            match key.as_str() {
                "generations" => if let Token::Number(n) = self.advance() { program.evolve_block.generations = n; },
                "instances" => if let Token::Number(n) = self.advance() { program.evolve_block.instances = n; },
                "stop" => program.evolve_block.stop_condition = Some(self.parse_block()?),
                _ => { self.advance(); }
            }
            if *self.peek() == Token::Comma { self.advance(); }
        }
        self.expect(Token::RBrace)?;
        Ok(())
    }

    fn parse_routine_def(&mut self) -> Result<RoutineDef, String> {
        self.expect(Token::Routine)?;
        let name = if let Token::Identifier(n) = self.advance() { n } else { return Err("Name".into()); };
        let mut args = Vec::new();
        if *self.peek() == Token::LParen {
            self.advance();
            while *self.peek() != Token::RParen {
                if let Token::Identifier(arg) = self.advance() {
                    args.push(arg);
                }
                if *self.peek() == Token::Comma { self.advance(); }
            }
            self.expect(Token::RParen)?;
        }
        let body = self.parse_block()?;
        Ok(RoutineDef { name, args, body })
    }

    pub fn parse_block(&mut self) -> Result<Vec<Command>, String> {
        self.expect(Token::LBrace)?;
        let mut cmds = Vec::new();
        while *self.peek() != Token::RBrace {
            cmds.push(self.parse_command()?);
        }
        self.expect(Token::RBrace)?;
        Ok(cmds)
    }

    pub fn parse_command(&mut self) -> Result<Command, String> {
        match self.peek() {
            Token::If => {
                self.advance();
                self.expect(Token::LParen)?;
                let cond = self.parse_bexp()?;
                self.expect(Token::RParen)?;
                let then_b = self.parse_block()?;
                let mut else_b = None;
                if *self.peek() == Token::Else {
                    self.advance();
                    if *self.peek() == Token::If {
                        else_b = Some(vec![self.parse_command()?]);
                    } else {
                        else_b = Some(self.parse_block()?);
                    }
                }
                Ok(Command::If { condition: cond, then_block: then_b, else_block: else_b })
            }
            Token::While => {
                self.advance();
                self.expect(Token::LParen)?;
                let cond = self.parse_bexp()?;
                self.expect(Token::RParen)?;
                let body = self.parse_block()?;
                Ok(Command::While { condition: cond, body })
            }
            Token::For => {
                self.advance();
                let var = if let Token::Identifier(n) = self.advance() { n } else { return Err("Expected var".into()); };
                self.expect(Token::In)?;
                let collection = match self.advance() {
                    Token::Identifier(n) => n,
                    Token::Environment => "environment".to_string(),
                    _ => return Err("Expected collection".into()),
                };
                let body = self.parse_block()?;
                Ok(Command::For { var, collection, body })
            }
            Token::Return => {
                self.advance();
                let exp = self.parse_exp()?;
                if *self.peek() == Token::SemiColon { self.advance(); }
                Ok(Command::Return(exp))
            }
            Token::Print => {
                self.advance();
                self.expect(Token::LParen)?;
                let mut exps = Vec::new();
                while *self.peek() != Token::RParen {
                    exps.push(self.parse_exp()?);
                    if *self.peek() == Token::Comma { self.advance(); }
                }
                self.expect(Token::RParen)?;
                if *self.peek() == Token::SemiColon { self.advance(); }
                Ok(Command::Print(exps))
            }
            Token::Spawn => {
                self.advance();
                let species = if let Token::Identifier(n) = self.advance() { n } else { return Err("Expected species".into()); };
                self.expect(Token::At)?;
                self.expect(Token::LParen)?;
                let x = self.parse_exp()?;
                self.expect(Token::Comma)?;
                let y = self.parse_exp()?;
                self.expect(Token::RParen)?;
                if *self.peek() == Token::SemiColon { self.advance(); }
                Ok(Command::Spawn { species, x, y })
            }
            _ => {
                let exp = self.parse_exp()?;
                if *self.peek() == Token::Equal {
                    self.advance();
                    let value = self.parse_exp()?;
                    if *self.peek() == Token::SemiColon { self.advance(); }
                    Ok(Command::Assign { target: exp, value })
                } else {
                    if *self.peek() == Token::SemiColon { self.advance(); }
                    Ok(Command::Exp(exp))
                }
            }
        }
    }

    pub fn parse_exp(&mut self) -> Result<Exp, String> {
        self.parse_sum()
    }

    fn parse_sum(&mut self) -> Result<Exp, String> {
        let mut left = self.parse_term()?;
        while matches!(self.peek(), Token::Plus | Token::Minus) {
            let op = match self.advance() {
                Token::Plus => "+".into(),
                Token::Minus => "-".into(),
                _ => unreachable!(),
            };
            let right = self.parse_term()?;
            left = Exp::BinaryOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Exp, String> {
        let mut left = self.parse_primary()?;
        while matches!(self.peek(), Token::Star | Token::Slash | Token::Percent) {
            let op = match self.advance() {
                Token::Star => "*".into(),
                Token::Slash => "/".into(),
                Token::Percent => "%".into(),
                _ => unreachable!(),
            };
            let right = self.parse_primary()?;
            left = Exp::BinaryOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Exp, String> {
        let t = self.advance();
        let mut node = match t {
            Token::Number(v) => Exp::Int(v),
            Token::StringLiteral(s) => Exp::StringLiteral(s),
            Token::LBracket => {
                let mut exps = Vec::new();
                while *self.peek() != Token::RBracket {
                    exps.push(self.parse_exp()?);
                    if *self.peek() == Token::Comma { self.advance(); }
                }
                self.expect(Token::RBracket)?;
                Exp::List(exps)
            }
            Token::LParen => {
                let exp = self.parse_exp()?;
                self.expect(Token::RParen)?;
                exp
            }
            Token::Identifier(_) | Token::Random => {
                let name = match t {
                    Token::Identifier(s) => s,
                    Token::Random => "random".into(),
                    _ => unreachable!(),
                };
                if *self.peek() == Token::LParen {
                    self.advance();
                    let mut args = Vec::new();
                    while *self.peek() != Token::RParen {
                        args.push(self.parse_exp()?);
                        if *self.peek() == Token::Comma { self.advance(); }
                    }
                    self.expect(Token::RParen)?;
                    Exp::Call(name, args)
                } else {
                    Exp::Var(name)
                }
            },
            _ => {
                return Err(format!("Expected exp, found {:?}", t));
            }
        };
        while matches!(self.peek(), Token::Dot | Token::LBracket) {
            if *self.peek() == Token::Dot {
                self.advance();
                if let Token::Identifier(f) = self.advance() { node = Exp::Dot(Box::new(node), f); }
            } else {
                self.advance();
                let idx = self.parse_exp()?;
                self.expect(Token::RBracket)?;
                node = Exp::Index(Box::new(node), Box::new(idx));
            }
        }
        Ok(node)
    }

    pub fn parse_bexp(&mut self) -> Result<BExp, String> {
        let mut left = self.parse_and_exp()?;
        while *self.peek() == Token::Or {
            self.advance();
            let right = self.parse_and_exp()?;
            left = BExp::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and_exp(&mut self) -> Result<BExp, String> {
        let mut left = self.parse_primary_bexp()?;
        while *self.peek() == Token::And {
            self.advance();
            let right = self.parse_primary_bexp()?;
            left = BExp::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_primary_bexp(&mut self) -> Result<BExp, String> {
        let left = self.parse_exp()?;
        let op = self.advance();
        let right = self.parse_exp()?;
        match op {
            Token::Greater => Ok(BExp::Greater(left, right)),
            Token::Less => Ok(BExp::Less(left, right)),
            Token::DoubleEqual => Ok(BExp::Equal(left, right)),
            Token::NotEqual => Ok(BExp::NotEqual(left, right)),
            _ => Err(format!("Expected comparison operator, found {:?}", op)),
        }
    }
}