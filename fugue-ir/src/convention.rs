use crate::compiler::{self, Specification};
use crate::disassembly::VarnodeData;
use crate::deserialise::error::Error as DeserialiseError;
use crate::register::RegisterNames;
use crate::space::AddressSpace;
use crate::space_manager::SpaceManager;

use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum PrototypeOperand {
    Register {
        name: Arc<str>,
        varnode: VarnodeData,
    },
    RegisterJoin {
        first_name: Arc<str>,
        first_varnode: VarnodeData,
        second_name: Arc<str>,
        second_varnode: VarnodeData,
    },
    StackRelative(u64),
}

impl PrototypeOperand {
    pub fn from_spec(spec: &compiler::PrototypeOperand, registers: &RegisterNames) -> Result<Self, DeserialiseError> {
        match spec {
            compiler::PrototypeOperand::Register(ref name) => {
                let (name, offset, size) = registers.get_by_name(&**name)
                    .ok_or_else(|| DeserialiseError::Invariant("register for prototype operand invalid"))?;
                Ok(Self::Register {
                    name: name.clone(),
                    varnode: VarnodeData::new(registers.register_space(), offset, size),
                })
            },
            compiler::PrototypeOperand::RegisterJoin(ref first_name, ref second_name) => {
                let (first_name, foffset, fsize) = registers.get_by_name(&**first_name)
                    .ok_or_else(|| DeserialiseError::Invariant("register for prototype operand invalid"))?;

                let (second_name, soffset, ssize) = registers.get_by_name(&**second_name)
                    .ok_or_else(|| DeserialiseError::Invariant("register for prototype operand invalid"))?;

                Ok(Self::RegisterJoin {
                    first_name: first_name.clone(),
                    first_varnode: VarnodeData::new(registers.register_space(), foffset, fsize),
                    second_name: second_name.clone(),
                    second_varnode: VarnodeData::new(registers.register_space(), soffset, ssize),
                })
            },
            compiler::PrototypeOperand::StackRelative(offset) => {
                Ok(Self::StackRelative(*offset))
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct PrototypeEntry {
    min_size: usize,
    max_size: usize,
    alignment: u64,
    meta_type: Option<String>,
    extension: Option<String>,
    operand: PrototypeOperand,
}

impl PrototypeEntry {
    pub fn from_spec(spec: &compiler::PrototypeEntry, registers: &RegisterNames) -> Result<Self, DeserialiseError> {
        Ok(Self {
            min_size: spec.min_size,
            max_size: spec.max_size,
            alignment: spec.alignment,
            meta_type: spec.meta_type.clone(),
            extension: spec.extension.clone(),
            operand: PrototypeOperand::from_spec(&spec.operand, registers)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Prototype {
    name: String,
    extra_pop: u64,
    stack_shift: u64,
    inputs: Vec<PrototypeEntry>,
    outputs: Vec<PrototypeEntry>,
}

impl Prototype {
    pub fn from_spec(spec: &compiler::Prototype, registers: &RegisterNames) -> Result<Self, DeserialiseError> {
        Ok(Self {
            name: spec.name.clone(),
            extra_pop:spec.extra_pop,
            stack_shift: spec.stack_shift,
            inputs: spec.inputs.iter().map(|input| PrototypeEntry::from_spec(input, registers)).collect::<Result<_, _>>()?,
            outputs: spec.outputs.iter().map(|output| PrototypeEntry::from_spec(output, registers)).collect::<Result<_, _>>()?,
        })
    }

    pub fn extra_pop(&self) -> u64 {
        self.extra_pop
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum ReturnAddress {
    Register {
        name: Arc<str>,
        varnode: VarnodeData,
    },
    StackRelative {
        offset: u64,
        size: usize,
    },
}

impl ReturnAddress {
    pub fn from_spec(spec: &compiler::ReturnAddress, registers: &RegisterNames) -> Result<Self, DeserialiseError> {
        match spec {
            compiler::ReturnAddress::Register(ref name) => {
                let (name, offset, size) = registers.get_by_name(&**name)
                    .ok_or_else(|| DeserialiseError::Invariant("register for return address invalid"))?;
                Ok(Self::Register {
                    name: name.clone(),
                    varnode: VarnodeData::new(registers.register_space(), offset, size),
                })
            },
            compiler::ReturnAddress::StackRelative { offset, size } => {
                Ok(Self::StackRelative {
                    offset: *offset,
                    size: *size,
                })
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct StackPointer {
    name: Arc<str>,
    varnode: VarnodeData,
    space: Arc<AddressSpace>,
}

impl StackPointer {
    pub fn from_spec(spec: &compiler::StackPointer, registers: &RegisterNames, manager: &SpaceManager) -> Result<Self, DeserialiseError> {
        let space = manager.space_by_name(&spec.space)
            .ok_or_else(|| DeserialiseError::Invariant("stack pointer space for convention invalid"))?;
        let (name, offset, size) = registers.get_by_name(&*spec.register)
            .ok_or_else(|| DeserialiseError::Invariant("named stack pointer invalid"))?;

        Ok(Self {
            name: name.clone(),
            varnode: VarnodeData::new(registers.register_space(), offset, size),
            space,
        })
    }

    pub fn varnode(&self) -> &VarnodeData {
        &self.varnode
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Convention {
    name: String,
    data_organisation: Option<compiler::DataOrganisation>,
    stack_pointer: StackPointer,
    return_address: ReturnAddress,
    default_prototype: Prototype,
    additional_prototypes: Vec<Prototype>,
}

impl Convention {
    pub fn from_spec(
        spec: &Specification,
        registers: &RegisterNames,
        manager: &SpaceManager
    ) -> Result<Self, DeserialiseError> {
        Ok(Self {
            name: spec.name.clone(),
            data_organisation: spec.data_organisation.clone(),
            stack_pointer: StackPointer::from_spec(&spec.stack_pointer,
                                                   registers,
                                                   manager)?,
            return_address: ReturnAddress::from_spec(&spec.return_address,
                                                     registers)?,
            default_prototype: Prototype::from_spec(&spec.default_prototype,
                                                    registers)?,
            additional_prototypes: spec.additional_prototypes.iter()
                .map(|prototype| Prototype::from_spec(prototype, registers))
                .collect::<Result<_, _>>()?,
        })
    }

    pub fn stack_pointer(&self) -> &StackPointer {
        &self.stack_pointer
    }

    pub fn return_address(&self) -> &ReturnAddress {
        &self.return_address
    }

    pub fn default_prototype(&self) -> &Prototype {
        &self.default_prototype
    }
}
