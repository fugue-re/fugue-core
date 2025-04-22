use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "parser.pest"]
pub(crate) struct SleighParser;
