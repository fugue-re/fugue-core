use std::borrow::Cow;
use std::path::Path;
use std::str::FromStr;

use fallible_iterator::FallibleIterator;

use object::{File, Object as ObjectT, ObjectSegment};

use crate::lifter::{Language, Lifter, LifterBuilder};
use crate::loader::{Loadable, LoadableSegment, LoaderError};
use crate::memory::SegmentProperties;
use crate::types::{Address, AttributeMap, BytesOrMapping};

#[ouroboros::self_referencing]
struct ObjectInner<'a> {
    data: BytesOrMapping<'a>,
    #[borrows(data)]
    #[covariant]
    view: File<'this, &'this BytesOrMapping<'a>>,
}

pub struct Object<'a> {
    object: ObjectInner<'a>,
    lifter: Lifter,
    attributes: AttributeMap,
}

pub fn object_lifter<'a>(object: &impl ObjectT<'a>) -> Result<Lifter, LoaderError> {
    use object::Architecture as A;

    let is_64 = object.is_64();
    let is_le = object.is_little_endian();
    let is_tmode = object.entry() & 1 == 1;

    let triple = match object.architecture() {
        A::Arm if is_64 && is_le => "AARCH64:LE:64",
        A::Arm if is_64 => "AARCH64:BE:64",
        A::Arm if is_le && is_tmode => "ARM:LE:32:v8T",
        A::Arm if is_le => "ARM:LE:32",
        A::Arm if is_tmode => "ARM:BE:32:v8T",
        A::Arm => "ARM:BE:32",
        A::I386 => "x86:LE:32",
        A::X86_64 => "x86:LE:64",
        _ => return Err(LoaderError::UnsupportedArch),
    };

    LifterBuilder::from_str(triple)?
        .build()
        .map_err(LoaderError::Lifter)
}

impl<'a> Object<'a> {
    pub fn new(data: impl Into<BytesOrMapping<'a>>) -> Result<Self, LoaderError> {
        Self::new_with(data, AttributeMap::new())
    }

    pub fn new_with(
        data: impl Into<BytesOrMapping<'a>>,
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, LoaderError> {
        let object = ObjectInner::try_new(data.into(), |data| {
            File::parse(data).map_err(LoaderError::format)
        })?;

        let view = object.borrow_view();
        let lifter = object_lifter(view)?;

        Ok(Self {
            object,
            lifter,
            attributes: attributes.into(),
        })
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, LoaderError> {
        Self::from_file_with(path, AttributeMap::new())
    }

    pub fn from_file_with(
        path: impl AsRef<Path>,
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, LoaderError> {
        let data = BytesOrMapping::from_file(path)?;
        Self::new_with(data, attributes)
    }
}

impl Loadable for Object<'_> {
    fn entry_address(&self) -> Option<Address> {
        Some(self.object.borrow_view().entry().into())
    }

    fn attributes(&self) -> &AttributeMap {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut AttributeMap {
        &mut self.attributes
    }

    fn language(&self) -> &'static Language {
        self.lifter.language()
    }

    fn lifter(&self) -> Lifter {
        self.lifter.clone()
    }

    fn segments<'a>(
        &'a self,
    ) -> impl FallibleIterator<Item = LoadableSegment<'a>, Error = LoaderError> + 'a {
        let view = self.object.borrow_view();

        // NOTE: we need to apply relocations
        // NOTE: we need to make a mapping of externs

        fallible_iterator::convert(view.segments().into_iter().filter_map(|segm| {
            if segm.size() == 0 {
                return None;
            }

            let address = Address::from(segm.address());
            let data = segm.data().unwrap_or_default();

            let bytes = if data.len() as u64 != segm.size() {
                // we have some partial or fully uninitialised segment?

                let mut data = data.to_owned();
                data.resize(segm.size() as _, 0);

                Cow::Owned(data)
            } else {
                Cow::Borrowed(data)
            };

            Some(Ok(LoadableSegment {
                name: segm
                    .name()
                    .ok()
                    .flatten()
                    .map_or_else(|| Cow::Borrowed("LOAD"), |name| Cow::Owned(name.to_owned())),
                address,
                properties: SegmentProperties::all(),
                bytes,
            }))
        }))
    }

    fn segment_range(&self) -> (Address, Address) {
        let mut start = None::<Address>;
        let mut end = None::<Address>;

        for segm in self.object.borrow_view().segments() {
            if segm.size() == 0 {
                continue;
            }

            let nstart = Address::from(segm.address());
            let nend = nstart + segm.size() - 1usize;

            start = Some(start.map_or(nstart, |start| start.min(nstart)));
            end = Some(end.map_or(nend, |end| end.max(nend)));
        }

        (start.unwrap_or_default(), end.unwrap_or_default())
    }
}
