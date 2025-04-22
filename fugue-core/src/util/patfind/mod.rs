use fugue_specs::PatternSpecs;
use static_init::dynamic;

#[dynamic(lazy)]
pub static ARM_LE_32: PatternSpecs = PatternSpecs::from_str(include_str!("./arm-le-32.yml")).unwrap();
