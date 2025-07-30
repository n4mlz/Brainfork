use crate::lexer::Token;
use std::iter::Peekable;
use std::slice::Iter;

#[derive(Debug, Clone)]
pub enum Node {
    IncPtr,
    DecPtr,
    IncCell,
    DecCell,
    Output,
    Input,
    Loop(Vec<Node>),
    Parallel(Vec<Vec<Node>>),
    LockAcquire,
    LockRelease,
    Wait(usize),
}

fn parse_parallel(iter: &mut Peekable<Iter<Token>>) -> Vec<Vec<Node>> {
    let mut branches = Vec::new();
    loop {
        let branch = parse_nodes(iter, &[Token::ParSep, Token::ParEnd]);
        branches.push(branch);
        match iter.next() {
            Some(Token::ParSep) => continue,
            Some(Token::ParEnd) => break,
            other => panic!("Expected '|' or '}}', found {other:?}"),
        }
    }
    branches
}

fn parse_nodes(iter: &mut Peekable<Iter<Token>>, terminators: &[Token]) -> Vec<Node> {
    let mut nodes = Vec::new();
    while let Some(&token_ref) = iter.peek() {
        let token = token_ref;
        if terminators.contains(token) {
            break;
        }
        match iter.next().unwrap() {
            Token::IncPtr => nodes.push(Node::IncPtr),
            Token::DecPtr => nodes.push(Node::DecPtr),
            Token::IncCell => nodes.push(Node::IncCell),
            Token::DecCell => nodes.push(Node::DecCell),
            Token::Output => nodes.push(Node::Output),
            Token::Input => nodes.push(Node::Input),
            Token::LoopStart => {
                let body = parse_nodes(iter, &[Token::LoopEnd]);
                iter.next();
                nodes.push(Node::Loop(body));
            }
            Token::ParStart => {
                let branches = parse_parallel(iter);
                nodes.push(Node::Parallel(branches));
            }
            Token::LockStart => nodes.push(Node::LockAcquire),
            Token::LockEnd => nodes.push(Node::LockRelease),
            Token::Wait => {
                let mut count = 1;
                while let Some(&&Token::Wait) = iter.peek() {
                    count += 1;
                    iter.next();
                }
                nodes.push(Node::Wait(count));
            }
            _ => break,
        }
    }
    nodes
}

pub fn parse(tokens: &[Token]) -> Vec<Node> {
    let mut iter = tokens.iter().peekable();
    parse_nodes(&mut iter, &[])
}
