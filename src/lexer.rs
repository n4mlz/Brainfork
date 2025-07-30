#[derive(Debug, Clone)]
pub enum Token {
    IncPtr,
    DecPtr,
    IncCell,
    DecCell,
    Output,
    Input,
    LoopStart,
    LoopEnd,
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
