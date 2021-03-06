use crate::deserialise::error::Error as DeserialiseError;
use crate::deserialise::parse::XmlExt;
use crate::error::Error;

use ahash::AHashMap as Map;

use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Specification {
    program_counter: String,
    context_set: Map<String, u32>,
    tracked_set: Map<String, u32>,
}

impl Specification {
    pub fn program_counter(&self) -> &str {
        self.program_counter.as_ref()
    }

    pub fn context_set(&self) -> impl Iterator<Item = (&str, u32)> {
        self.context_set.iter().map(|(n, v)| (n.as_ref(), *v))
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        // Example .pspec file
        // <processor_spec>
        // <properties>
        //     <property key="addressesDoNotAppearDirectlyInCode" value="true"/>
        //     <property key="allowOffcutReferencesToFunctionStarts" value="true"/>
        //     <property key="useNewFunctionStackAnalysis" value="true"/>
        //     <property key="enableSharedReturnAnalysis" value="false"/>
        //     <property key="emulateInstructionStateModifierClass" value="ghidra.program.emulation.ARMEmulateInstructionStateModifier"/>
        // </properties>
        // <programcounter register="pc"/>
        // <context_data>
        //     <context_set space="ram">
        //     <set name="LRset" val="0" description="0 lr reg not set, 1 for LR set, affects BX as a call"/>
        //     </context_set>
        //     <tracked_set space="ram">
        //     <set name="spsr" val="0"/>
        //     </tracked_set>
        // </context_data>

        // <default_symbols>
        //     <symbol name="Reset" address="ram:0x0" entry="true"/>
        //      ...
        // </default_symbols>
        // </processor_spec>
        if input.tag_name().name() != "processor_spec" {
            return Err(DeserialiseError::TagUnexpected(
                input.tag_name().name().to_owned(),
            ));
        }

        let mut program_counter = None;
        let mut context_set = Map::default();
        let mut tracked_set = Map::default();

        for child in input.children().filter(xml::Node::is_element) {
            match child.tag_name().name() {
                "programcounter" => {
                    program_counter = Some(child.attribute_string("register")?);
                }
                "context_data" => {
                    for cchild in child.children().filter(xml::Node::is_element) {
                        match cchild.tag_name().name() {
                            "context_set" => {
                                for ct in cchild.children().filter(xml::Node::is_element) {
                                    context_set.insert(
                                        ct.attribute_string("name")?,
                                        ct.attribute_int::<u32>("val")?,
                                    );
                                }
                            }
                            "tracked_set" => {
                                for ct in cchild.children().filter(xml::Node::is_element) {
                                    tracked_set.insert(
                                        ct.attribute_string("name")?,
                                        ct.attribute_int::<u32>("val")?,
                                    );
                                }
                            }
                            _ => (),
                        }
                    }
                }
                _ => (),
            }
        }

        if let Some(program_counter) = program_counter {
            Ok(Self {
                program_counter,
                context_set,
                tracked_set,
            })
        } else {
            Err(DeserialiseError::Invariant(
                "processor specification must define a program counter",
            ))
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref();
        let mut file = File::open(path).map_err(|error| Error::ParseFile {
            path: path.to_owned(),
            error,
        })?;

        let mut input = String::new();
        file.read_to_string(&mut input)
            .map_err(|error| Error::ParseFile {
                path: path.to_owned(),
                error,
            })?;

        Self::from_str(&input).map_err(|error| Error::DeserialiseFile {
            path: path.to_owned(),
            error,
        })
    }

    pub fn from_str<S: AsRef<str>>(input: S) -> Result<Self, DeserialiseError> {
        let document = xml::Document::parse(input.as_ref()).map_err(DeserialiseError::Xml)?;

        Self::from_xml(document.root_element())
    }
}
