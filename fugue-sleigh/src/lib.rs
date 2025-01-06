use pest_derive::Parser;

pub mod ast;

#[derive(Parser)]
#[grammar = "parser.pest"]
pub struct SleighParser;
