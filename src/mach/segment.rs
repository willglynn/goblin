use scroll::{self, Pread, Pwrite};
use scroll::ctx::{self, SizeWith};

use std::fmt;
use std::ops::{Deref, DerefMut};

use container;
use error;

use mach::relocation::RelocationInfo;
use mach::load_command::{Section32, Section64, SegmentCommand32, SegmentCommand64, SIZEOF_SECTION_32, SIZEOF_SECTION_64, SIZEOF_SEGMENT_COMMAND_32, SIZEOF_SEGMENT_COMMAND_64, LC_SEGMENT, LC_SEGMENT_64};

pub struct RelocationIterator<'a> {
    data: &'a [u8],
    nrelocs: usize,
    offset: usize,
    count: usize,
    ctx: scroll::Endian,
}

impl<'a> Iterator for RelocationIterator<'a> {
    type Item = error::Result<RelocationInfo>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.count >= self.nrelocs {
            None
        } else {
            self.count += 1;
            match self.data.gread_with(&mut self.offset, self.ctx) {
                Ok(res) => Some(Ok(res)),
                Err(e) => Some(Err(e.into()))
            }
        }
    }
}

/// Generalized 32/64 bit Section
#[derive(Default)]
pub struct Section {
    /// name of this section
    pub sectname:  [u8; 16],
    /// segment this section goes in
    pub segname:   [u8; 16],
    /// memory address of this section
    pub addr:      u64,
    /// size in bytes of this section
    pub size:      u64,
    /// file offset of this section
    pub offset:    u32,
    /// section alignment (power of 2)
    pub align:     u32,
    /// file offset of relocation entries
    pub reloff:    u32,
    /// number of relocation entries
    pub nreloc:    u32,
    /// flags (section type and attributes
    pub flags:     u32,
}

impl Section {
    /// The name of this section
    pub fn name(&self) -> error::Result<&str> {
        Ok(self.sectname.pread::<&str>(0)?)
    }
    /// The containing segment's name
    pub fn segname(&self) -> error::Result<&str> {
        Ok(self.segname.pread::<&str>(0)?)
    }
    /// Iterate this sections relocations given `data`; `data` must be the original binary
    pub fn iter_relocations<'b>(&self, data: &'b [u8], ctx: container::Ctx) -> RelocationIterator<'b> {
        let offset = self.reloff as usize;
        debug!("Relocations for {} starting at offset: {:#x}", self.name().unwrap_or("BAD_SECTION_NAME"), offset);
        RelocationIterator {
            offset: offset,
            nrelocs: self.nreloc as usize,
            count: 0,
            data: data,
            ctx: ctx.le,
        }
    }
}

impl From<Section> for Section64 {
    fn from(section: Section) -> Self {
        Section64 {
            sectname: section.sectname,
            segname:  section.segname,
            addr:     section.addr as u64,
            size:     section.size as u64,
            offset:   section.offset,
            align:    section.align,
            reloff:   section.reloff,
            nreloc:   section.nreloc,
            flags:    section.flags,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
        }
    }
}

impl From<Section> for Section32 {
    fn from(section: Section) -> Self {
        Section32 {
            sectname: section.sectname,
            segname:  section.segname,
            addr:     section.addr as u32,
            size:     section.size as u32,
            offset:   section.offset,
            align:    section.align,
            reloff:   section.reloff,
            nreloc:   section.nreloc,
            flags:    section.flags,
            reserved1: 0,
            reserved2: 0,
        }
    }
}

impl fmt::Debug for Section {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Section")
            .field("sectname", &self.name().unwrap())
            .field("segname",  &self.segname().unwrap())
            .field("addr",     &self.addr)
            .field("size",     &self.size)
            .field("offset",   &self.offset)
            .field("align",    &self.align)
            .field("reloff",   &self.reloff)
            .field("nreloc",   &self.nreloc)
            .field("flags",    &self.flags)
            .finish()
    }
}

impl From<Section32> for Section {
    fn from(section: Section32) -> Self {
        Section {
            sectname: section.sectname,
            segname:  section.segname,
            addr:     section.addr as u64,
            size:     section.size as u64,
            offset:   section.offset,
            align:    section.align,
            reloff:   section.reloff,
            nreloc:   section.nreloc,
            flags:    section.flags,
        }
    }
}

impl From<Section64> for Section {
    fn from(section: Section64) -> Self {
        Section {
            sectname: section.sectname,
            segname:  section.segname,
            addr:     section.addr,
            size:     section.size,
            offset:   section.offset,
            align:    section.align,
            reloff:   section.reloff,
            nreloc:   section.nreloc,
            flags:    section.flags,
        }
    }
}

impl<'a> ctx::TryFromCtx<'a, container::Ctx> for Section {
    type Error = ::error::Error;
    type Size = usize;
    fn try_from_ctx(bytes: &'a [u8], ctx: container::Ctx) -> Result<(Self, Self::Size), Self::Error> {
        match ctx.container {
            container::Container::Little => {
                let section = Section::from(bytes.pread_with::<Section32>(0, ctx.le)?);
                Ok((section, SIZEOF_SECTION_32))
            },
            container::Container::Big    => {
                let section = Section::from(bytes.pread_with::<Section64>(0, ctx.le)?);
                Ok((section, SIZEOF_SECTION_64))
            },
        }
    }
}

impl ctx::SizeWith<container::Ctx> for Section {
    type Units = usize;
    fn size_with(ctx: &container::Ctx) -> usize {
        match ctx.container {
            container::Container::Little => SIZEOF_SECTION_32,
            container::Container::Big    => SIZEOF_SECTION_64,
        }
    }
}

impl ctx::TryIntoCtx<container::Ctx> for Section {
    type Error = ::error::Error;
    type Size = usize;
    fn try_into_ctx(self, bytes: &mut [u8], ctx: container::Ctx) -> Result<Self::Size, Self::Error> {
        if ctx.is_big () {
            bytes.pwrite_with::<Section64>(self.into(), 0, ctx.le)?;
        } else {
            bytes.pwrite_with::<Section32>(self.into(), 0, ctx.le)?;
        }
        Ok(Self::size_with(&ctx))
    }
}

impl ctx::IntoCtx<container::Ctx> for Section {
    fn into_ctx(self, bytes: &mut [u8], ctx: container::Ctx) {
        bytes.pwrite_with(self, 0, ctx).unwrap();
    }
}

pub struct SectionIterator<'a> {
    data: &'a [u8],
    count: usize,
    offset: usize,
    idx: usize,
    ctx: container::Ctx,
}

pub type SectionData<'a> = &'a [u8];

impl<'a> ::std::iter::ExactSizeIterator for SectionIterator<'a> {
    fn len(&self) -> usize {
        self.count
    }
}

impl<'a> Iterator for SectionIterator<'a> {
    type Item = error::Result<(Section, SectionData<'a>)>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.count {
            None
        } else {
            self.idx += 1;
            match self.data.gread_with::<Section>(&mut self.offset, self.ctx) {
                Ok(section) => {
                    let data = &self.data[section.offset as usize..][..section.size as usize];
                    Some(Ok((section, data)))
                },
                Err(e) => Some(Err(e.into()))
            }
        }
    }
}

impl<'a, 'b> IntoIterator for &'b Segment<'a> {
    type Item = error::Result<(Section, SectionData<'a>)>;
    type IntoIter = SectionIterator<'a>;
    fn into_iter(self) -> Self::IntoIter {
        SectionIterator {
            data: self.raw_data,
            count: self.nsects as usize,
            offset: self.offset + Segment::size_with(&self.ctx),
            idx: 0,
            ctx: self.ctx,
        }
    }
}

/// Generalized 32/64 bit Segment Command
pub struct Segment<'a> {
    pub cmd:      u32,
    pub cmdsize:  u32,
    pub segname:  [u8; 16],
    pub vmaddr:   u64,
    pub vmsize:   u64,
    pub fileoff:  u64,
    pub filesize: u64,
    pub maxprot:  u32,
    pub initprot: u32,
    pub nsects:   u32,
    pub flags:    u32,
    pub data:     &'a [u8],
    offset:       usize,
    raw_data:     &'a [u8],
    ctx:          container::Ctx,
}

impl<'a> From<Segment<'a>> for SegmentCommand64 {
    fn from(segment: Segment<'a>) -> Self {
        SegmentCommand64 {
            cmd:      segment.cmd,
            cmdsize:  segment.cmdsize,
            segname:  segment.segname,
            vmaddr:   segment.vmaddr   as u64,
            vmsize:   segment.vmsize   as u64,
            fileoff:  segment.fileoff  as u64,
            filesize: segment.filesize as u64,
            maxprot:  segment.maxprot,
            initprot: segment.initprot,
            nsects:   segment.nsects,
            flags:    segment.flags,
        }
    }
}

impl<'a> From<Segment<'a>> for SegmentCommand32 {
    fn from(segment: Segment<'a>) -> Self {
        SegmentCommand32 {
            cmd:      segment.cmd,
            cmdsize:  segment.cmdsize,
            segname:  segment.segname,
            vmaddr:   segment.vmaddr   as u32,
            vmsize:   segment.vmsize   as u32,
            fileoff:  segment.fileoff  as u32,
            filesize: segment.filesize as u32,
            maxprot:  segment.maxprot,
            initprot: segment.initprot,
            nsects:   segment.nsects,
            flags:    segment.flags,
        }
    }
}

impl<'a> fmt::Debug for Segment<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Segment")
            .field("cmd", &self.cmd)
            .field("cmdsize", &self.cmdsize)
            .field("segname", &self.segname.pread::<&str>(0).unwrap())
            .field("vmaddr",  &self.vmaddr)
            .field("vmsize",  &self.vmsize)
            .field("fileoff", &self.fileoff)
            .field("filesize", &self.filesize)
            .field("maxprot", &self.maxprot)
            .field("initprot", &self.initprot)
            .field("nsects", &self.nsects)
            .field("flags", &self.flags)
            .field("data", &self.data.len())
            .field("sections", &self.sections().unwrap())
            .finish()
    }
}

impl<'a> ctx::SizeWith<container::Ctx> for Segment<'a> {
    type Units = usize;
    fn size_with(ctx: &container::Ctx) -> usize {
        match ctx.container {
            container::Container::Little => SIZEOF_SEGMENT_COMMAND_32,
            container::Container::Big    => SIZEOF_SEGMENT_COMMAND_64,
        }
    }
}

impl<'a> ctx::TryIntoCtx<container::Ctx> for Segment<'a> {
    type Error = ::error::Error;
    type Size = usize;
    fn try_into_ctx(self, bytes: &mut [u8], ctx: container::Ctx) -> Result<Self::Size, Self::Error> {
        let segment_size = Self::size_with(&ctx);
        if ctx.is_big () {
            bytes.pwrite_with::<SegmentCommand64>(self.into(), 0, ctx.le)?;
        } else {
            bytes.pwrite_with::<SegmentCommand32>(self.into(), 0, ctx.le)?;
        }
        Ok(segment_size )
    }
}

impl<'a> ctx::IntoCtx<container::Ctx> for Segment<'a> {
    fn into_ctx(self, bytes: &mut [u8], ctx: container::Ctx) {
        bytes.pwrite_with(self, 0, ctx).unwrap();
    }
}

impl<'a> Segment<'a> {
    /// Create a new, blank segment, with cmd either `LC_SEGMENT_64`, or `LC_SEGMENT`, depending on `ctx`. **NB** You are responsible for providing a correctly marshalled byte array as the sections. You should not use this for anything other than writing.
    pub fn new(ctx: container::Ctx, sections: &'a [u8]) -> Self {
        Segment {
            cmd:      if ctx.is_big() { LC_SEGMENT_64 } else { LC_SEGMENT },
            cmdsize:  (Self::size_with(&ctx) + sections.len()) as u32,
            segname:  [0; 16],
            vmaddr:   0,
            vmsize:   0,
            fileoff:  0,
            filesize: 0,
            maxprot:  0,
            initprot: 0,
            nsects:   0,
            flags:    0,
            data:     sections,
            offset:   0,
            raw_data: &[],
            ctx:      ctx,
        }
    }
    /// Get the name of this segment
    pub fn name(&self) -> error::Result<&str> {
        Ok(self.segname.pread::<&str>(0)?)
    }
    /// Get the sections from this segment, erroring if any section couldn't be retrieved
    pub fn sections(&self) -> error::Result<Vec<(Section, SectionData<'a>)>> {
        let mut sections = Vec::new();
        for section in self.into_iter() {
            sections.push(section?);
        }
        Ok(sections)
    }
    /// Convert the raw C 32-bit segment command to a generalized version
    pub fn from_32(bytes: &'a[u8], segment: &SegmentCommand32, offset: usize, ctx: container::Ctx) -> Self {
        let data = &bytes[segment.fileoff as usize..(segment.fileoff + segment.filesize) as usize];
        Segment {
            cmd:      segment.cmd,
            cmdsize:  segment.cmdsize,
            segname:  segment.segname,
            vmaddr:   segment.vmaddr   as u64,
            vmsize:   segment.vmsize   as u64,
            fileoff:  segment.fileoff  as u64,
            filesize: segment.filesize as u64,
            maxprot:  segment.maxprot,
            initprot: segment.initprot,
            nsects:   segment.nsects,
            flags:    segment.flags,
            data:     data,
            offset:   offset,
            raw_data: bytes,
            ctx:      ctx,
        }
    }
    /// Convert the raw C 64-bit segment command to a generalized version
    pub fn from_64(bytes: &'a [u8], segment: &SegmentCommand64, offset: usize, ctx: container::Ctx) -> Self {
        let data = &bytes[segment.fileoff as usize..(segment.fileoff + segment.filesize) as usize];
        Segment {
            cmd:      segment.cmd,
            cmdsize:  segment.cmdsize,
            segname:  segment.segname,
            vmaddr:   segment.vmaddr,
            vmsize:   segment.vmsize,
            fileoff:  segment.fileoff,
            filesize: segment.filesize,
            maxprot:  segment.maxprot,
            initprot: segment.initprot,
            nsects:   segment.nsects,
            flags:    segment.flags,
            offset:   offset,
            data:     data,
            raw_data: bytes,
            ctx:      ctx,
        }
    }
}

#[derive(Debug, Default)]
/// An opaque 32/64-bit container for Mach-o segments
pub struct Segments<'a> {
    segments: Vec<Segment<'a>>,
    ctx: container::Ctx,
}

impl<'a> Deref for Segments<'a> {
    type Target = Vec<Segment<'a>>;
    fn deref(&self) -> &Self::Target {
        &self.segments
    }
}

impl<'a> DerefMut for Segments<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.segments
    }
}

impl<'a, 'b> IntoIterator for &'b Segments<'a> {
    type Item = &'b Segment<'a>;
    type IntoIter = ::std::slice::Iter<'b, Segment<'a>>;
    fn into_iter(self) -> Self::IntoIter {
        self.segments.iter()
    }
}

impl<'a> Segments<'a> {
    /// Construct a new generalized segment container from this `ctx`
    pub fn new(ctx: container::Ctx) -> Self {
        Segments {
            segments: Vec::new(),
            ctx: ctx,
        }
    }
    /// Get every section from every segment
    // thanks to SpaceManic for figuring out the 'b lifetimes here :)
    pub fn sections<'b>(&'b self) -> Box<Iterator<Item=SectionIterator<'a>> + 'b> {
        Box::new(self.segments.iter().map(|segment| segment.into_iter()))
    }
}
