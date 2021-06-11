use interval_tree::IntervalTree;

use crate::ArchitectureDef;
use crate::Id;
use crate::IntraRef;
use crate::Database;
use crate::Segment;

use crate::error::Error;
use crate::schema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BasicBlock {
    address: u64,
    length: usize,
    architecture: Id<ArchitectureDef>,
    segment: Id<Segment>,
    predecessors: Vec<IntraRef>,
    successors: Vec<IntraRef>,
}

impl BasicBlock {
    pub fn address(&self) -> u64 {
        self.address
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn architecture_id(&self) -> Id<ArchitectureDef> {
        self.architecture.clone()
    }

    pub fn architecture<'a>(&self, project: &'a Database) -> &'a ArchitectureDef {
        &project.architectures()[self.architecture.index()]
    }

    pub fn segment_id(&self) -> Id<Segment> {
        self.segment.clone()
    }

    pub fn segment<'a>(&self, project: &'a Database) -> &'a Segment {
        project.segments().get_index(self.segment.index())
            .map(|iv| iv.value()).unwrap()
    }

    pub fn successors(&self) -> &[IntraRef] {
        &self.successors
    }

    pub fn predecessors(&self) -> &[IntraRef] {
        &self.predecessors
    }

    pub fn bytes<'a>(&self, project: &'a Database) -> &'a [u8] {
        let segment = self.segment(project);
        let offset = (self.address() - segment.address()) as usize;
        &segment.bytes()[offset..offset+self.len()]
    }

    pub(crate) fn from_reader(reader: schema::basic_block::Reader, segments: &IntervalTree<u64, Segment>) -> Result<Self, Error> {
        Ok(Self {
            address: reader.get_address(),
            length: reader.get_length() as usize,
            architecture: reader.get_architecture().into(),
            segment: segments.find_one(&reader.get_address())
                .ok_or_else(|| Error::NoBlockSegment(reader.get_address()))?.index().into(),
            predecessors: reader.get_predecessors().map_err(Error::Deserialisation)?
                .into_iter()
                .map(IntraRef::from_reader)
                .collect::<Result<Vec<_>, _>>()?,
            successors: reader.get_successors().map_err(Error::Deserialisation)?
                .into_iter()
                .map(IntraRef::from_reader)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    pub(crate) fn to_builder(&self, builder: &mut schema::basic_block::Builder) -> Result<(), Error> {
        builder.set_address(self.address());
        builder.set_length(self.len() as u32);
        builder.set_architecture(self.architecture_id().index() as u32);
        let mut predecessors = builder.reborrow().init_predecessors(self.predecessors.len() as u32);
        self.predecessors.iter().enumerate().try_for_each(|(i, r)| {
            let mut builder = predecessors.reborrow().get(i as u32);
            r.to_builder(&mut builder)
        })?;
        let mut successors = builder.reborrow().init_successors(self.successors.len() as u32);
        self.successors.iter().enumerate().try_for_each(|(i, r)| {
            let mut builder = successors.reborrow().get(i as u32);
            r.to_builder(&mut builder)
        })?;
        Ok(())
    }
}
