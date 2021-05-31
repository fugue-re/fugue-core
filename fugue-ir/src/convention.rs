use crate::VarnodeData;
use crate::compiler::{self, Specification};
use crate::deserialise::error::Error as DeserialiseError;
use crate::space::AddressSpace;
use crate::space_manager::SpaceManager;

use fnv::FnvHashMap as Map;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrototypeOperand<'space> {
    Register {
        name: &'space str,
        varnode: VarnodeData<'space>,
    },
    RegisterJoin {
        first_name: &'space str,
        first_varnode: VarnodeData<'space>,
        second_name: &'space str,
        second_varnode: VarnodeData<'space>,
    },
    StackRelative(u64),
}

impl<'space> PrototypeOperand<'space> {
    pub fn from_spec(spec: &compiler::PrototypeOperand, registers: &Map<&'space str, VarnodeData<'space>>) -> Result<Self, DeserialiseError> {
        match spec {
            compiler::PrototypeOperand::Register(ref name) => {
                let (name, varnode) = registers.get_key_value(&**name)
                    .ok_or_else(|| DeserialiseError::Invariant("register for prototype operand invalid"))?;
                Ok(Self::Register {
                    name: *name,
                    varnode: varnode.clone(),
                })
            },
            compiler::PrototypeOperand::RegisterJoin(ref first_name, ref second_name) => {
                let (first_name, first_varnode) = registers.get_key_value(&**first_name)
                    .ok_or_else(|| DeserialiseError::Invariant("register for prototype operand invalid"))?;

                let (second_name, second_varnode) = registers.get_key_value(&**second_name)
                    .ok_or_else(|| DeserialiseError::Invariant("register for prototype operand invalid"))?;

                Ok(Self::RegisterJoin {
                    first_name: *first_name,
                    first_varnode: first_varnode.clone(),
                    second_name: *second_name,
                    second_varnode: second_varnode.clone(),
                })
            },
            compiler::PrototypeOperand::StackRelative(offset) => {
                Ok(Self::StackRelative(*offset))
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrototypeEntry<'space> {
    min_size: usize,
    max_size: usize,
    alignment: u64,
    meta_type: Option<String>,
    extension: Option<String>,
    operand: PrototypeOperand<'space>,
}

impl<'space> PrototypeEntry<'space> {
    pub fn from_spec(spec: &compiler::PrototypeEntry, registers: &Map<&'space str, VarnodeData<'space>>) -> Result<Self, DeserialiseError> {
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
pub struct Prototype<'space> {
    name: String,
    extra_pop: u64,
    stack_shift: u64,
    inputs: Vec<PrototypeEntry<'space>>,
    outputs: Vec<PrototypeEntry<'space>>,
}

impl<'space> Prototype<'space> {
    pub fn from_spec(spec: &compiler::Prototype, registers: &Map<&'space str, VarnodeData<'space>>) -> Result<Self, DeserialiseError> {
        Ok(Self {
            name: spec.name.clone(),
            extra_pop:spec.extra_pop,
            stack_shift: spec.stack_shift,
            inputs: spec.inputs.iter().map(|input| PrototypeEntry::from_spec(input, registers)).collect::<Result<_, _>>()?,
            outputs: spec.outputs.iter().map(|output| PrototypeEntry::from_spec(output, registers)).collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnAddress<'space> {
    Register {
        name: &'space str,
        varnode: VarnodeData<'space>,
    },
    StackRelative {
        offset: u64,
        size: usize,
    },
}

impl<'space> ReturnAddress<'space> {
    pub fn from_spec(spec: &compiler::ReturnAddress, registers: &Map<&'space str, VarnodeData<'space>>) -> Result<Self, DeserialiseError> {
        match spec {
            compiler::ReturnAddress::Register(ref name) => {
                let (name, varnode) = registers.get_key_value(&**name)
                    .ok_or_else(|| DeserialiseError::Invariant("register for return address invalid"))?;
                Ok(Self::Register {
                    name: *name,
                    varnode: varnode.clone(),
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
pub struct StackPointer<'space> {
    name: &'space str,
    varnode: VarnodeData<'space>,
    space: &'space AddressSpace,
}

impl<'space> StackPointer<'space> {
    pub fn from_spec(spec: &compiler::StackPointer, registers: &Map<&'space str, VarnodeData<'space>>, manager: &'space SpaceManager) -> Result<Self, DeserialiseError> {
        let space = manager.space_by_name(&spec.space)
            .ok_or_else(|| DeserialiseError::Invariant("stack pointer space for convention invalid"))?;
        let (name, varnode) = registers.get_key_value(&*spec.register)
            .ok_or_else(|| DeserialiseError::Invariant("named stack pointer invalid"))?;

        Ok(Self {
            name: *name,
            varnode: varnode.clone(),
            space,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Convention<'space> {
    name: String,
    data_organisation: compiler::DataOrganisation,
    stack_pointer: StackPointer<'space>,
    return_address: ReturnAddress<'space>,
    default_prototype: Prototype<'space>,
    additional_prototypes: Vec<Prototype<'space>>,
}

impl<'space> Convention<'space> {
    pub fn from_spec(
        spec: &Specification,
        registers_by_name: &Map<&'space str, VarnodeData<'space>>,
        manager: &'space SpaceManager
    ) -> Result<Self, DeserialiseError> {
        Ok(Self {
            name: spec.name.clone(),
            data_organisation: spec.data_organisation.clone(),
            stack_pointer: StackPointer::from_spec(&spec.stack_pointer,
                                                   registers_by_name,
                                                   manager)?,
            return_address: ReturnAddress::from_spec(&spec.return_address,
                                                     registers_by_name)?,
            default_prototype: Prototype::from_spec(&spec.default_prototype,
                                                    registers_by_name)?,
            additional_prototypes: spec.additional_prototypes.iter()
                .map(|prototype| Prototype::from_spec(prototype, registers_by_name))
                .collect::<Result<_, _>>()?,
        })
    }
}
