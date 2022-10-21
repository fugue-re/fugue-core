use crate::deserialise::error::Error as DeserialiseError;
use crate::deserialise::parse::XmlExt;
use crate::error::Error;

use ahash::AHashMap as Map;

use std::fs::File;
use std::io::Read;
use std::iter::FromIterator;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct DataOrganisation {
    pub absolute_max_alignment: u64,
    pub machine_alignment: u64,
    pub default_alignment: u64,
    pub default_pointer_alignment: u64,
    pub pointer_size: usize,
    pub wchar_size: usize,
    pub short_size: usize,
    pub integer_size: usize,
    pub long_size: usize,
    pub long_long_size: usize,
    pub float_size: usize,
    pub double_size: usize,
    pub long_double_size: usize,
    pub size_alignment_map: Map<usize, u64>,
}

impl Default for DataOrganisation {
    fn default() -> Self {
        Self {
            absolute_max_alignment: 0,
            machine_alignment: 1,
            default_alignment: 1,
            default_pointer_alignment: 4,
            pointer_size: 4,
            wchar_size: 2,
            short_size: 2,
            integer_size: 4,
            long_size: 4,
            long_long_size: 8,
            float_size: 4,
            double_size: 8,
            long_double_size: 12,
            size_alignment_map: Map::from_iter(vec![
                (1, 1),
                (2, 2),
                (4, 4),
                (8, 8),
            ]),
        }
    }
}

impl DataOrganisation {
    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "data_organization" {
            return Err(DeserialiseError::TagUnexpected(
                    input.tag_name().name().to_owned(),
            ));
        }

        let mut data = Self::default();

        for child in input.children().filter(xml::Node::is_element) {
            match child.tag_name().name() {
                "absolute_max_alignment" => {
                    data.absolute_max_alignment = child.attribute_int("value")?;
                },
                "machine_alignment" => {
                    data.machine_alignment = child.attribute_int("value")?;
                },
                "default_alignment" => {
                    data.default_alignment = child.attribute_int("value")?;
                },
                "default_pointer_alignment" => {
                    data.default_pointer_alignment = child.attribute_int("value")?;
                },
                "pointer_size" => {
                    data.pointer_size = child.attribute_int("value")?;
                },
                "wchar_size" => {
                    data.wchar_size = child.attribute_int("value")?;
                },
                "short_size" => {
                    data.short_size = child.attribute_int("value")?;
                },
                "integer_size" => {
                    data.integer_size = child.attribute_int("value")?;
                },
                "long_size" => {
                    data.long_size = child.attribute_int("value")?;
                },
                "long_long_size" => {
                    data.long_long_size = child.attribute_int("value")?;
                },
                "float_size" => {
                    data.float_size = child.attribute_int("value")?;
                },
                "double_size" => {
                    data.double_size = child.attribute_int("value")?;
                },
                "long_double_size" => {
                    data.long_double_size = child.attribute_int("value")?;
                },
                "size_alignment_map" => {
                    for entry in child.children().filter(|e| e.is_element() && e.tag_name().name() == "entry") {
                        data.size_alignment_map.insert(
                            entry.attribute_int("size")?,
                            entry.attribute_int("alignment")?,
                        );
                    }
                },
                _ => (),
            }
        }

        Ok(data)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct StackPointer {
    pub(crate) register: String,
    pub(crate) space: String,
}

impl StackPointer {
    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "stackpointer" {
            return Err(DeserialiseError::TagUnexpected(
                    input.tag_name().name().to_owned(),
            ));
        }

        Ok(Self {
            register: input.attribute_string("register")?,
            space: input.attribute_string("space")?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum ReturnAddress {
    Register(String),
    StackRelative {
        offset: u64,
        size: usize,
    },
}

impl ReturnAddress {
    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "returnaddress" {
            return Err(DeserialiseError::TagUnexpected(
                    input.tag_name().name().to_owned(),
            ));
        }

        let mut children = input.children().filter(xml::Node::is_element);

        let node = children.next()
            .ok_or_else(|| DeserialiseError::Invariant("no children for returnaddress"))?;

        match node.tag_name().name() {
            "register" => {
                Ok(Self::Register(node.attribute_string("name")?))
            },
            "varnode" if node.attribute_string("space").map(|space| space == "stack").unwrap_or(false) => {
                Ok(Self::StackRelative {
                    offset: node.attribute_int("offset")?,
                    size: node.attribute_int("size")?,
                })
            },
            tag => {
                Err(DeserialiseError::TagUnexpected(tag.to_owned()))
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum PrototypeOperand {
    Register(String),
    RegisterJoin(String, String),
    StackRelative(u64),
}

impl PrototypeOperand {
    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        match input.tag_name().name() {
            "addr" => match input.attribute_string("space")?.as_ref() {
                "join" => {
                    Ok(Self::RegisterJoin(
                            input.attribute_string("piece1")?,
                            input.attribute_string("piece2")?,
                    ))
                },
                "stack" => {
                    Ok(Self::StackRelative(
                            input.attribute_int("offset")?,
                    ))
                },
                tag => {
                    Err(DeserialiseError::TagUnexpected(tag.to_owned()))
                },
            },
            "register" => {
                Ok(Self::Register(
                        input.attribute_string("name")?,
                ))
            },
            tag => {
                Err(DeserialiseError::TagUnexpected(tag.to_owned()))
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct PrototypeEntry {
    pub(crate) killed_by_call: bool,
    pub(crate) min_size: usize,
    pub(crate) max_size: usize,
    pub(crate) alignment: u64,
    pub(crate) meta_type: Option<String>,
    pub(crate) extension: Option<String>,
    pub(crate) operand: PrototypeOperand,
}

impl PrototypeEntry {
    pub fn from_xml(input: xml::Node, killed_by_call: bool) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "pentry" {
            return Err(DeserialiseError::TagUnexpected(
                    input.tag_name().name().to_owned(),
            ));
        }

        let min_size = input.attribute_int("minsize")?;
        let max_size = input.attribute_int("maxsize")?;
        let alignment = input.attribute_int_opt("alignment", 1)?;

        let meta_type = input.attribute_string("metatype")
            .map(Some)
            .unwrap_or_default();
        let extension = input.attribute_string("extension")
            .map(Some)
            .unwrap_or_default();

        let node = input.children().filter(xml::Node::is_element).next();
        if node.is_none() {
            return Err(DeserialiseError::Invariant(
                    "compiler specification prototype entry does not define an operand"
            ))
        }

        let operand = PrototypeOperand::from_xml(node.unwrap())?;

        Ok(Self {
            killed_by_call,
            min_size,
            max_size,
            alignment,
            meta_type,
            extension,
            operand,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Prototype {
    pub(crate) name: String,
    pub(crate) extra_pop: u64,
    pub(crate) stack_shift: u64,
    pub(crate) inputs: Vec<PrototypeEntry>,
    pub(crate) outputs: Vec<PrototypeEntry>,
    pub(crate) unaffected: Vec<PrototypeOperand>,
    pub(crate) killed_by_call: Vec<PrototypeOperand>,
    pub(crate) likely_trashed: Vec<PrototypeOperand>,
}

impl Prototype {
    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "prototype" {
            return Err(DeserialiseError::TagUnexpected(
                    input.tag_name().name().to_owned(),
            ));
        }

        let name = input.attribute_string("name")?;
        let extra_pop = if matches!(input.attribute("extrapop"), Some("unknown")) {
            0
        } else {
            input.attribute_int("extrapop")?
        };
        let stack_shift = input.attribute_int("stackshift")?;

        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut unaffected = Vec::new();
        let mut killed_by_call = Vec::new();
        let mut likely_trashed = Vec::new();

        for child in input.children().filter(xml::Node::is_element) {
            match child.tag_name().name() {
                "input" => {
                    let mut values = child.children()
                        .filter(xml::Node::is_element)
                        .map(|v| PrototypeEntry::from_xml(v, false))
                        .collect::<Result<Vec<_>, _>>()?;
                    inputs.append(&mut values);
                },
                "output" => {
                    let killed = child.attribute_bool("killedbycall").unwrap_or(false);
                    let mut values = child.children()
                        .filter(xml::Node::is_element)
                        .map(|v| PrototypeEntry::from_xml(v, killed))
                        .collect::<Result<Vec<_>, _>>()?;
                    outputs.append(&mut values);
                },
                "unaffected" => {
                    let mut values = child.children()
                        .filter(xml::Node::is_element)
                        .filter_map(|op| PrototypeOperand::from_xml(op).ok())
                        .collect::<Vec<_>>();
                    unaffected.append(&mut values);
                },
                "killedbycall" => {
                    let mut values = child.children()
                        .filter(xml::Node::is_element)
                        .filter_map(|op| PrototypeOperand::from_xml(op).ok())
                        .collect::<Vec<_>>();
                    killed_by_call.append(&mut values);
                },
                "likelytrash" => {
                    let mut values = child.children()
                        .filter(xml::Node::is_element)
                        .filter_map(|op| PrototypeOperand::from_xml(op).ok())
                        .collect::<Vec<_>>();
                    likely_trashed.append(&mut values);
                },
                _ => (),
            }
        }

        Ok(Self {
            name,
            extra_pop,
            stack_shift,
            inputs,
            outputs,
            unaffected,
            killed_by_call,
            likely_trashed,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Specification {
    pub(crate) name: String,
    pub(crate) data_organisation: Option<DataOrganisation>,
    pub(crate) stack_pointer: StackPointer,
    pub(crate) return_address: ReturnAddress,
    pub(crate) default_prototype: Prototype,
    pub(crate) additional_prototypes: Vec<Prototype>,
}

impl Specification {
    pub fn named_from_xml<N: Into<String>>(name: N, input: xml::Node) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "compiler_spec" {
            return Err(DeserialiseError::TagUnexpected(
                    input.tag_name().name().to_owned(),
            ));
        }

        let mut data_organisation = None;
        let mut stack_pointer = None;
        let mut return_address = None;
        let mut default_prototype = None;
        let mut additional_prototypes = Vec::new();

        for child in input.children().filter(xml::Node::is_element) {
            match child.tag_name().name() {
                "data_organization" => {
                    data_organisation = Some(DataOrganisation::from_xml(child)?);
                },
                "stackpointer" => {
                    stack_pointer = Some(StackPointer::from_xml(child)?);
                },
                "returnaddress" => {
                    return_address = Some(ReturnAddress::from_xml(child)?);
                },
                "default_proto" => {
                    let proto = child.children().filter(xml::Node::is_element).next();
                    if proto.is_none() {
                        return Err(DeserialiseError::Invariant(
                                "compiler specification does not define prototype for default prototype"
                        ))
                    }
                    default_prototype = Some(Prototype::from_xml(proto.unwrap())?);
                },
                "prototype" => {
                    additional_prototypes.push(Prototype::from_xml(child)?);
                },
                _ => (),
            }
        }

        // if data_organisation.is_none() {
        //     return Err(DeserialiseError::Invariant(
        //             "compiler specification does not define data organisation"
        //     ))
        // }

        if stack_pointer.is_none() {
            return Err(DeserialiseError::Invariant(
                    "compiler specification does not define stack pointer configuration"
            ))
        }

        if return_address.is_none() {
            return Err(DeserialiseError::Invariant(
                    "compiler specification does not define return address"
            ))
        }

        Ok(Self {
            name: name.into(),
            data_organisation,
            stack_pointer: stack_pointer.unwrap(),
            return_address: return_address.unwrap(),
            default_prototype: default_prototype.unwrap(),
            additional_prototypes,
        })
    }

    pub fn named_from_file<N: Into<String>, P: AsRef<Path>>(name: N, path: P) -> Result<Self, Error> {
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

        Self::named_from_str(name, &input).map_err(|error| Error::DeserialiseFile {
            path: path.to_owned(),
            error,
        })
    }

    pub fn named_from_str<N: Into<String>, S: AsRef<str>>(name: N, input: S) -> Result<Self, DeserialiseError> {
        let document = xml::Document::parse(input.as_ref()).map_err(DeserialiseError::Xml)?;

        let res = Self::named_from_xml(name, document.root_element());

        if let Err(ref e) = res {
            log::debug!("load failed: {:?}", e);
        }

        res
    }
}
