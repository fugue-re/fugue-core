use pest_derive::Parser;

pub mod ast;
pub mod cfg;
pub mod ir;

#[derive(Parser)]
#[grammar = "parser.pest"]
pub struct SleighParser;
