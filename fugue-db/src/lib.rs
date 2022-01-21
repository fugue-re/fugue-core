//#[allow(non_snake_case, unused)]
//#[path = "../target/flatbuffers/fugue_generated.rs"]
//mod fugue_schema;
//pub use fugue_schema::fugue::schema as schema;


pub mod architecture;
pub mod backend;
pub mod basic_block;
pub mod database;
pub mod error;
pub mod format;
pub mod function;
pub mod id;
pub mod inter_ref;
pub mod intra_ref;
pub mod metadata;
pub mod schema;
pub mod segment;

pub use error::*;
pub use id::Id;

pub use architecture::{ArchitectureDef, Endian};
pub use basic_block::BasicBlock;
pub use database::{Database, DatabaseImporter};
pub use format::Format;
pub use function::Function;
pub use inter_ref::InterRef;
pub use intra_ref::IntraRef;
pub use metadata::Metadata;
pub use segment::Segment;
