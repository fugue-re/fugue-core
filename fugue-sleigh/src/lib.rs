use pest_derive::Parser;

pub mod ast;
pub mod cfg;

#[derive(Parser)]
#[grammar = "parser.pest"]
pub struct SleighParser;
