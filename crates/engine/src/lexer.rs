#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    IncPtr,
    DecPtr,
    IncCell,
    DecCell,
    Output,
    Input,
    LoopStart,
    LoopEnd,
    ParStart,
    ParSep,
    ParEnd,
    LockStart,
    LockEnd,
    Wait,
}

pub fn lex(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '>' => tokens.push(Token::IncPtr),
            '<' => tokens.push(Token::DecPtr),
            '+' => tokens.push(Token::IncCell),
            '-' => tokens.push(Token::DecCell),
            '.' => tokens.push(Token::Output),
            ',' => tokens.push(Token::Input),
            '[' => tokens.push(Token::LoopStart),
            ']' => tokens.push(Token::LoopEnd),
            '{' => tokens.push(Token::ParStart),
            '|' => tokens.push(Token::ParSep),
            '}' => tokens.push(Token::ParEnd),
            '(' => tokens.push(Token::LockStart),
            ')' => tokens.push(Token::LockEnd),
            '~' => tokens.push(Token::Wait),
            ';' => {
                while chars.peek().is_some() && *chars.peek().unwrap() != '\n' {
                    chars.next();
                }
            }
            _ => {}
        }
    }

    tokens
}
