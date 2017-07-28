//! Load commands tell the kernel and dynamic linker anything from how to load this binary into memory, what the entry point is, apple specific information, to which libraries it requires for dynamic linking

use error;
use container;
use std::fmt::{self, Display};
use core::ops::{Deref, DerefMut};
use scroll::{self, ctx, Endian, Pread};
use scroll::ctx::{TryFromCtx, SizeWith};

use mach::relocation::RelocationInfo;

///////////////////////////////////////
// Load Commands from mach-o/loader.h
// with some rusty additions
//////////////////////////////////////

#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
/// Occurs at the beginning of every load command to serve as a sort of tagged union/enum discriminant
pub struct LoadCommandHeader {
  pub cmd: u32,
  pub cmdsize: u32,
}

impl Display for LoadCommandHeader {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "LoadCommandHeader: {} size: {}", cmd_to_str(self.cmd), self.cmdsize)
    }
}

pub const SIZEOF_LOAD_COMMAND: usize = 8;

pub type LcStr = u32;

pub const SIZEOF_LC_STR: usize = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct Section32 {
    /// name of this section
    pub sectname:  [u8; 16],
    /// segment this section goes in
    pub segname:   [u8; 16],
    /// memory address of this section
    pub addr:      u32,
    /// size in bytes of this section
    pub size:      u32,
    /// file offset of this section
    pub offset:    u32,
    /// section alignment (power of 2)
    pub align:     u32,
    /// file offset of relocation entries
    pub reloff:    u32,
    /// number of relocation entries
    pub nreloc:    u32,
    /// flags (section type and attributes)
    pub flags:     u32,
    /// reserved (for offset or index)
    pub reserved1: u32,
    /// reserved (for count or sizeof)
    pub reserved2: u32,
}

pub const SIZEOF_SECTION_32: usize = 68;

/// for 64-bit architectures
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct Section64 {
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
    /// reserved (for offset or index)
    pub reserved1: u32,
    /// reserved (for count or sizeof)
    pub reserved2: u32,
    /// reserved
    pub reserved3: u32,
}

pub const SIZEOF_SECTION_64: usize = 80;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct SegmentCommand32 {
    pub cmd:      u32,
    pub cmdsize:  u32,
    pub segname:  [u8; 16],
    pub vmaddr:   u32,
    pub vmsize:   u32,
    pub fileoff:  u32,
    pub filesize: u32,
    pub maxprot:  u32,
    pub initprot: u32,
    pub nsects:   u32,
    pub flags:    u32,
}

pub const SIZEOF_SEGMENT_COMMAND_32: usize = 56;

impl SegmentCommand32 {
    pub fn name(&self) -> error::Result<&str> {
        Ok(self.segname.pread::<&str>(0)?)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct SegmentCommand64 {
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
}

pub const SIZEOF_SEGMENT_COMMAND_64: usize = 72;

impl SegmentCommand64 {
    pub fn name(&self) -> error::Result<&str> {
        Ok(self.segname.pread::<&str>(0)?)
    }
}
/// Fixed virtual memory shared libraries are identified by two things.  The
/// target pathname (the name of the library as found for execution), and the
/// minor version number.  The address of where the headers are loaded is in
/// header_addr. (THIS IS OBSOLETE and no longer supported).
#[repr(packed)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct Fvmlib {
    /// library's target pathname
    pub name: u32,
    /// library's minor version number
    pub minor_version: u32,
    /// library's header address
    pub header_addr:   u32,
}

pub const SIZEOF_FVMLIB: usize = 12;

/// A fixed virtual shared library (fipub constype == MH_FVMLIB in the mach header)
/// contains a fvmlib_command (cmd == LC_IDFVMLIB) to identify the library.
/// An object that uses a fixed virtual shared library also contains a
/// fvmlib_command (cmd == LC_LOADFVMLIB) for each library it uses.
/// (THIS IS OBSOLETE and no longer supported).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct FvmlibCommand {
    /// LC_IDFVMLIB or LC_LOADFVMLIB
    pub cmd: u32,
    /// includes pathname string
    pub cmdsize: u32,
    /// the library identification
    pub fvmlib: Fvmlib,
}

pub const SIZEOF_FVMLIB_COMMAND: usize = 20;

// /// Dynamicly linked shared libraries are identified by two things.  The
// /// pathname (the name of the library as found for execution), and the
// /// compatibility version number.  The pathname must match and the compatibility
// /// number in the user of the library must be greater than or equal to the
// /// library being used.  The time stamp is used to record the time a library was
// /// built and copied into user so it can be use to determined if the library used
// /// at runtime is exactly the same as used to built the program.
// struct dylib {
//     union lc_str  name;   // library's path name
//     uint32_t timestamp;   // library's build time stamp
//     uint32_t current_version;  // library's current version number
//     uint32_t compatibility_version; // library's compatibility vers number
// }

/// A dynamically linked shared library (fipub constype == MH_DYLIB in the mach header)
/// contains a dylib_command (cmd == LC_ID_DYLIB) to identify the library.
/// An object that uses a dynamically linked shared library also contains a
/// dylib_command (cmd == LC_LOAD_DYLIB, LC_LOAD_WEAK_DYLIB, or
/// LC_REEXPORT_DYLIB) for each library it uses.
#[repr(packed)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct Dylib {
    /// library's path name
    pub name: LcStr,
    /// library's build time stamp
    pub timestamp: u32,
    /// library's current version number
    pub current_version: u32,
    /// library's compatibility vers number
    pub compatibility_version: u32,
}

pub const SIZEOF_DYLIB: usize = 16;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct DylibCommand {
    /// LC_ID_DYLIB, LC_LOAD_DYLIB, LC_LOAD_WEAK_DYLIB, LC_REEXPORT_DYLIB
    pub cmd: u32,
    /// includes pathname string
    pub cmdsize: u32,
    /// the library identification
    pub dylib: Dylib,
  }

pub const SIZEOF_DYLIB_COMMAND: usize = 20;

/// A dynamically linked shared library may be a subframework of an umbrella
/// framework.  If so it will be linked with "-umbrella umbrella_name" where
/// Where "umbrella_name" is the name of the umbrella framework. A subframework
/// can only be linked against by its umbrella framework or other subframeworks
/// that are part of the same umbrella framework.  Otherwise the static link
/// editor produces an error and states to link against the umbrella framework.
/// The name of the umbrella framework for subframeworks is recorded in the
/// following structure.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct SubFrameworkCommand {
    /// LC_SUB_FRAMEWORK
    pub cmd:     u32,
    /// includes umbrella string
    pub cmdsize: u32,
    /// the umbrella framework name
    pub umbrella: u32,
}

pub const SIZEOF_SUB_FRAMEWORK_COMMAND: usize = 12;

/// For dynamically linked shared libraries that are subframework of an umbrella
/// framework they can allow clients other than the umbrella framework or other
/// subframeworks in the same umbrella framework.  To do this the subframework
/// is built with "-allowable_client client_name" and an LC_SUB_CLIENT load
/// command is created for each -allowable_client flag.  The client_name is
/// usually a framework name.  It can also be a name used for bundles clients
/// where the bundle is built with "-client_name client_name".
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct SubClientCommand {
    /// LC_SUB_CLIENT
    pub cmd:     u32,
    /// includes client string
    pub cmdsize: u32,
    /// the client name
    pub client: LcStr,
}

pub const SIZEOF_SUB_CLIENT_COMMAND: usize = 12;

/// A dynamically linked shared library may be a sub_umbrella of an umbrella
/// framework.  If so it will be linked with "-sub_umbrella umbrella_name" where
/// Where "umbrella_name" is the name of the sub_umbrella framework.  When
/// staticly linking when -twolevel_namespace is in effect a twolevel namespace
/// umbrella framework will only cause its subframeworks and those frameworks
/// listed as sub_umbrella frameworks to be implicited linked in.  Any other
/// dependent dynamic libraries will not be linked it when -twolevel_namespace
/// is in effect.  The primary library recorded by the static linker when
/// resolving a symbol in these libraries will be the umbrella framework.
/// Zero or more sub_umbrella frameworks may be use by an umbrella framework.
/// The name of a sub_umbrella framework is recorded in the following structure.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct SubUmbrellaCommand {
    /// LC_SUB_UMBRELLA
    pub cmd:     u32,
    /// includes sub_umbrella string
    pub cmdsize: u32,
    /// the sub_umbrella framework name
    pub sub_umbrella: LcStr,
}

pub const SIZEOF_SUB_UMBRELLA_COMMAND: usize = 12;

/// A dynamically linked shared library may be a sub_library of another shared
/// library.  If so it will be linked with "-sub_library library_name" where
/// Where "library_name" is the name of the sub_library shared library.  When
/// staticly linking when -twolevel_namespace is in effect a twolevel namespace
/// shared library will only cause its subframeworks and those frameworks
/// listed as sub_umbrella frameworks and libraries listed as sub_libraries to
/// be implicited linked in.  Any other dependent dynamic libraries will not be
/// linked it when -twolevel_namespace is in effect.  The primary library
/// recorded by the static linker when resolving a symbol in these libraries
/// will be the umbrella framework (or dynamic library). Zero or more sub_library
/// shared libraries may be use by an umbrella framework or (or dynamic library).
/// The name of a sub_library framework is recorded in the following structure.
/// For example /usr/lib/libobjc_profile.A.dylib would be recorded as "libobjc".
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct SubLibraryCommand {
    /// LC_SUB_LIBRARY
    pub cmd:     u32,
    /// includes sub_library string
    pub cmdsize: u32,
    /// the sub_library name
    pub sub_library: LcStr,
}

pub const SIZEOF_SUB_LIBRARY_COMMAND: usize = 12;

/// A program (type == MH_EXECUTE) that is
/// prebound to its dynamic libraries has one of these for each library that
/// the static linker used in prebinding.  It contains a bit vector for the
/// modules in the library.  The bits indicate which modules are bound (1) and
/// which are not (0) from the library.  The bit for module 0 is the low bit
/// of the first byte.  So the bit for the Nth module is:
/// (linked_modules[N/8] >> N%8) & 1
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct PreboundDylibCommand {
    /// LC_PREBOUND_DYLIB
    pub cmd:     u32,
    /// includes strings
    pub cmdsize: u32,
    /// library's path name
    pub name: LcStr,
    /// number of modules in library
    pub nmodules: u32,
    /// bit vector of linked modules
    // TODO: fixme
    pub linked_modules: LcStr,
}

pub const SIZEOF_PREBOUND_DYLIB_COMMAND: usize = 20;

/// The name of the dynamic linker
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct DylinkerCommand {
    pub cmd:     u32,
    pub cmdsize: u32,
    pub name:    LcStr,
}

pub const SIZEOF_DYLINKER_COMMAND: usize = 12;

/// Thread commands contain machine-specific data structures suitable for
/// use in the thread state primitives.  The machine specific data structures
/// follow the struct thread_command as follows.
/// Each flavor of machine specific data structure is preceded by an unsigned
/// long constant for the flavor of that data structure, an uint32_t
/// that is the count of longs of the size of the state data structure and then
/// the state data structure follows.  This triple may be repeated for many
/// flavors.  The constants for the flavors, counts and state data structure
/// definitions are expected to be in the header file <machine/thread_status.h>.
/// These machine specific data structures sizes must be multiples of
/// 4 bytes  The cmdsize reflects the total size of the thread_command
/// and all of the sizes of the constants for the flavors, counts and state
/// data structures.
///
/// For executable objects that are unix processes there will be one
/// thread_command (cmd == LC_UNIXTHREAD) created for it by the link-editor.
/// This is the same as a LC_THREAD, except that a stack is automatically
/// created (based on the shell's limit for the stack size).  CommandVariant arguments
/// and environment variables are copied onto that stack.
// unimplemented, see machine/thread_status.h for rest of values:
// uint32_t flavor		   flavor of thread state
// uint32_t count		   count of longs in thread state
// struct XXX_thread_state state   thread state for this flavor
// ...
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct ThreadCommand {
    /// LC_THREAD or  LC_UNIXTHREAD
    pub cmd:     u32,
    /// total size of this command
    pub cmdsize: u32,
    pub flavor: u32,
    pub count: u32,
    /// NOTE: this is actually, in classic mach-o style, a semi-tagged union-esque struct, and is _not_ properly handled here for arches other than i386... e.g., this is incorrect for powerpc, armv7, etc.
    /// TODO: We need to implement a simple getter for the thread state; or even better, a method that simply returns the entry point (which is what we usually want)
    pub thread_state: I386ThreadState,
}

/// Main thread state consists of
/// general registers, segment registers,
/// eip and eflags.
///
#[repr(packed)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct I386ThreadState {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub esp: u32,
    pub ss: u32,
    pub eflags: u32,
    pub eip: u32,
    pub cs: u32,
    pub ds: u32,
    pub es: u32,
    pub fs: u32,
    pub gs: u32,
}

/// The routines command contains the address of the dynamic shared library
/// initialization routine and an index into the module table for the module
/// that defines the routine.  Before any modules are used from the library the
/// dynamic linker fully binds the module that defines the initialization routine
/// and then calls it.  This gets called before any module initialization
/// routines (used for C++ static constructors) in the library.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct RoutinesCommand32 {
    /// LC_ROUTINES
    pub cmd:         u32,
    /// total size of this command
    pub cmdsize:     u32,
    /// address of initialization routine
    pub init_address:u32,
    /// index into the module table that the init routine is defined in
    pub init_module: u32,
    pub reserved1:   u32,
    pub reserved2:   u32,
    pub reserved3:   u32,
    pub reserved4:   u32,
    pub reserved5:   u32,
    pub reserved6:   u32,
}

/// The 64-bit routines command.  Same use as above.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct RoutinesCommand64 {
    /// LC_ROUTINES_64
    pub cmd:          u32,
    /// total size of this command
    pub cmdsize:      u32,
    /// address of initialization routine
    pub init_address: u64,
    /// index into the module table that the init routine is defined in 8 bytes each
    pub init_module:  u64,
    pub reserved1:    u64,
    pub reserved2:    u64,
    pub reserved3:    u64,
    pub reserved4:    u64,
    pub reserved5:    u64,
    pub reserved6:    u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct SymtabCommand {
  pub cmd:     u32,
  pub cmdsize: u32,
  pub symoff:  u32,
  pub nsyms:   u32,
  pub stroff:  u32,
  pub strsize: u32,
}

pub const SIZEOF_SYMTAB_COMMAND: usize = 24;

/// This is the second set of the symbolic information which is used to support
/// the data structures for the dynamically link editor.
///
/// The original set of symbolic information in the symtab_command which contains
/// the symbol and string tables must also be present when this load command is
/// present.  When this load command is present the symbol table is organized
/// into three groups of symbols:
/// local symbols (static and debugging symbols) - grouped by module
/// defined external symbols - grouped by module (sorted by name if not lib)
/// undefined external symbols (sorted by name if MH_BINDATLOAD is not set,
///             and in order the were seen by the static
///        linker if MH_BINDATLOAD is set)
/// In this load command there are offsets and counts to each of the three groups
/// of symbols.
///
/// This load command contains a the offsets and sizes of the following new
/// symbolic information tables:
/// table of contents
/// module table
/// reference symbol table
/// indirect symbol table
/// The first three tables above (the table of contents, module table and
/// reference symbol table) are only present if the file is a dynamically linked
/// shared library.  For executable and object modules, which are files
/// containing only one module, the information that would be in these three
/// tables is determined as follows:
///  table of contents - the defined external symbols are sorted by name
/// module table - the file contains only one module so everything in the
///         file is part of the module.
/// reference symbol table - is the defined and undefined external symbols
///
/// For dynamically linked shared library files this load command also contains
/// offsets and sizes to the pool of relocation entries for all sections
/// separated into two groups:
/// external relocation entries
/// local relocation entries
/// For executable and object modules the relocation entries continue to hang
/// off the section structures.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct DysymtabCommand {
    pub cmd: u32,
    pub cmdsize: u32,
    /// index to local symbols
    pub ilocalsym:      u32,
    /// number of local symbols
    pub nlocalsym:      u32,
    /// index to externally defined symbols
    pub iextdefsym:     u32,
    /// number of externally defined symbols
    pub nextdefsym:     u32,
    /// index to undefined symbols
    pub iundefsym:      u32,
    /// number of undefined symbols
    pub nundefsym:      u32,
    /// file offset to table of contents
    pub tocoff:         u32,
    /// number of entries in table of contents
    pub ntoc:           u32,
    /// file offset to module table
    pub modtaboff:      u32,
    /// number of module table entries
    pub nmodtab:        u32,
    /// offset to referenced symbol table
    pub extrefsymoff:   u32,
    /// number of referenced symbol table entries
    pub nextrefsyms:    u32,
    /// file offset to the indirect symbol table
    pub indirectsymoff: u32,
    /// number of indirect symbol table entries
    pub nindirectsyms:  u32,
    /// offset to external relocation entries
    pub extreloff:      u32,
    /// number of external relocation entries
    pub nextrel:        u32,
    /// offset to local relocation entries
    pub locreloff:      u32,
    /// number of local relocation entries
    pub nlocrel:        u32,
}

pub const SIZEOF_DYSYMTAB_COMMAND: usize = 80;

// TODO: unimplemented
/// a table of contents entry
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct DylibTableOfContents {
    /// the defined external symbol (index into the symbol table)
    pub symbol_index: u32,
    /// index into the module table this symbol is defined in
    pub module_index: u32,
}

// TODO: unimplemented
/// a module table entry
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct DylibModule {
    /// the module name (index into string table)
    pub module_name: u32,
    ///index into externally defined symbols
    pub iextdefsym: u32,
    ///number of externally defined symbols
    pub nextdefsym: u32,
    /// index into reference symbol table
    pub irefsym: u32,
    ///number of reference symbol table entries
    pub nrefsym: u32,
    /// index into symbols for local symbols
    pub ilocalsym: u32,
    ///number of local symbols
    pub nlocalsym: u32,

    /// index into external relocation entries
    pub iextrel: u32,
    /// number of external relocation entries
    pub nextrel: u32,

    /// low 16 bits are the index into the init section, high 16 bits are the index into the term section
    pub iinit_iterm: u32,
    /// low 16 bits are the number of init section entries, high 16 bits are the number of term section entries
    pub ninit_nterm: u32,
    /// the (__OBJC,_module_info) section
    pub objc_module_info_addr: u32,
    /// the (__OBJC,__module_info) section
    pub objc_module_info_size: u32,
}

// TODO: unimplemented
/// a 64-bit module table entry
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct DylibModule64 {
    /// the module name (index into string table)
    pub module_name: u32,

    /// index into externally defined symbols
    pub iextdefsym: u32,
    /// number of externally defined symbols
    pub nextdefsym: u32,
    /// index into reference symbol table
    pub irefsym: u32,
    /// number of reference symbol table entries
    pub nrefsym: u32,
    /// index into symbols for local symbols
    pub ilocalsym: u32,
    /// number of local symbols
    pub nlocalsym: u32,

    /// index into external relocation entries
    pub iextrel: u32,
    /// number of external relocation entries
    pub nextrel: u32,

    /// low 16 bits are the index into the init section, high 16 bits are the index into the term section
    pub iinit_iterm: u32,
    /// low 16 bits are the number of init section entries, high 16 bits are the number of term section entries
    pub ninit_nterm: u32,

    /// the (__OBJC,__module_info) section
    pub objc_module_info_size: u32,
    /// the (__OBJC,__module_info) section
    pub objc_module_info_addr: u64,
}

/// The entries in the reference symbol table are used when loading the module
/// (both by the static and dynamic link editors) and if the module is unloaded
/// or replaced.  Therefore all external symbols (defined and undefined) are
/// listed in the module's reference table.  The flags describe the type of
/// reference that is being made.  The constants for the flags are defined in
/// <mach-o/nlist.h> as they are also used for symbol table entries.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct DylibReference {
    /// 24 bits bit-field index into the symbol table
    pub isym: [u8; 24],
    /// flags to indicate the type of reference
    pub flags: u64,
}

/// The twolevel_hints_command contains the offset and number of hints in the
/// two-level namespace lookup hints table.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct TwolevelHintsCommand {
    /// LC_TWOLEVEL_HINTS
    pub cmd: u32,
    /// sizeof(struct twolevel_hints_command)
    pub cmdsize: u32,
    /// offset to the hint table
    pub offset: u32,
    /// number of hints in the hint table
    pub nhints: u32,
}

/// The entries in the two-level namespace lookup hints table are twolevel_hint
/// structs.  These provide hints to the dynamic link editor where to start
/// looking for an undefined symbol in a two-level namespace image.  The
/// isub_image field is an index into the sub-images (sub-frameworks and
/// sub-umbrellas list) that made up the two-level image that the undefined
/// symbol was found in when it was built by the static link editor.  If
/// isub-image is 0 the the symbol is expected to be defined in library and not
/// in the sub-images.  If isub-image is non-zero it is an index into the array
/// of sub-images for the umbrella with the first index in the sub-images being
/// 1. The array of sub-images is the ordered list of sub-images of the umbrella
/// that would be searched for a symbol that has the umbrella recorded as its
/// primary library.  The table of contents index is an index into the
/// library's table of contents.  This is used as the starting point of the
/// binary search or a directed linear search.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct TwolevelHint {
    /// index into the sub images
    pub isub_image: u64,
    /// 24 bit field index into the table of contents
    pub itoc: [u8; 24],
}

/// The prebind_cksum_command contains the value of the original check sum for
/// prebound files or zero.  When a prebound file is first created or modified
/// for other than updating its prebinding information the value of the check sum
/// is set to zero.  When the file has it prebinding re-done and if the value of
/// the check sum is zero the original check sum is calculated and stored in
/// cksum field of this load command in the output file.  If when the prebinding
/// is re-done and the cksum field is non-zero it is left unchanged from the
/// input file.
// TODO: unimplemented
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct PrebindCksumCommand {
    /// LC_PREBIND_CKSUM
    pub cmd: u32,
    /// sizeof(struct prebind_cksum_command)
    pub cmdsize: u32,
    /// the check sum or zero
    pub cksum: u32,
}

/// The uuid load command contains a single 128-bit unique random number that
/// identifies an object produced by the static link editor.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct UuidCommand {
    /// LC_UUID
    pub cmd: u32,
    /// sizeof(struct uuid_command)
    pub cmdsize: u32,
    /// 16 bytes the 128-bit uuid
    pub uuid: [u8; 16],
}

pub const SIZEOF_UUID_COMMAND: usize = 24;

/// The rpath_command contains a path which at runtime should be added to
/// the current run path used to find @rpath prefixed dylibs.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct RpathCommand {
    /// LC_RPATH
    pub cmd: u32,
    /// includes string
    pub cmdsize: u32,
    /// path to add to run path
    pub path: LcStr,
}

pub const SIZEOF_RPATH_COMMAND: usize = 12;

/// The linkedit_data_command contains the offsets and sizes of a blob
/// of data in the __LINKEDIT segment.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct LinkeditDataCommand {
    /// LC_CODE_SIGNATURE, LC_SEGMENT_SPLIT_INFO, LC_FUNCTION_STARTS, LC_DATA_IN_CODE, LC_DYLIB_CODE_SIGN_DRS or LC_LINKER_OPTIMIZATION_HINT.
    pub cmd: u32,
    /// sizeof(struct linkedit_data_command)
    pub cmdsize: u32,
    /// file offset of data in __LINKEDIT segment
    pub dataoff: u32,
    /// file size of data in __LINKEDIT segment
    pub datasize: u32,
}

pub const SIZEOF_LINKEDIT_DATA_COMMAND: usize = 16;

/// The encryption_info_command contains the file offset and size of an
/// of an encrypted segment.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct EncryptionInfoCommand32 {
    /// LC_ENCRYPTION_INFO
    pub cmd: u32,
    /// sizeof(struct encryption_info_command)
    pub cmdsize: u32,
    /// file offset of encrypted range
    pub cryptoff: u32,
    /// file size of encrypted range
    pub cryptsize: u32,
    /// which enryption system, 0 means not-encrypted yet
    pub cryptid: u32,
}

pub const SIZEOF_ENCRYPTION_INFO_COMMAND_32: usize = 20;

/// The encryption_info_command_64 contains the file offset and size of an
/// of an encrypted segment (for use in x86_64 targets).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct EncryptionInfoCommand64 {
    /// LC_ENCRYPTION_INFO_64
    pub cmd: u32,
    /// sizeof(struct encryption_info_command_64)
    pub cmdsize: u32,
    /// file offset of encrypted range
    pub cryptoff: u32,
    /// file size of encrypted range
    pub cryptsize: u32,
    /// which enryption system, 0 means not-encrypted yet
    pub cryptid: u32,
    /// padding to make this struct's size a multiple of 8 bytes
    pub pad: u32,
}

pub const SIZEOF_ENCRYPTION_INFO_COMMAND_64: usize = 24;

/// The version_min_command contains the min OS version on which this
/// binary was built to run.
///
/// LC_VERSION_MIN_MACOSX or LC_VERSION_MIN_IPHONEOS
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct VersionMinCommand {
    pub cmd: u32,
    pub cmdsize: u32,
    /// X.Y.Z is encoded in nibbles xxxx.yy.zz
    pub version: u32,
    /// X.Y.Z is encoded in nibbles xxxx.yy.zz
    pub sdk: u32,
}

pub const SIZEOF_VERSION_MIN_COMMAND: usize = 16;

#[repr(C)]
#[derive(Default, Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct DyldInfoCommand {
    /// LC_DYLD_INFO or LC_DYLD_INFO_ONLY
    pub cmd: u32,
    /// sizeof(struct dyld_info_command)
    pub cmdsize: u32,
    /// file offset to rebase info
    pub rebase_off: u32,
    /// size of rebase info
    pub rebase_size: u32,
    /// file offset to binding info
    pub bind_off: u32,
    /// size of binding info
    pub bind_size: u32,
    /// file offset to weak binding info
    pub weak_bind_off: u32,
    /// size of weak binding info
    pub weak_bind_size: u32,
    /// file offset to lazy binding info
    pub lazy_bind_off: u32,
    /// size of lazy binding infs
    pub lazy_bind_size: u32,
    /// file offset to lazy binding info
    pub export_off: u32,
    /// size of lazy binding infs
    pub export_size: u32,
}

pub const SIZEOF_DYLIB_INFO_COMMAND: usize = 48;

/// The linker_option_command contains linker options embedded in object files.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct LinkerOptionCommand {
    /// LC_LINKER_OPTION only used in MH_OBJECT fipub constypes
    pub cmd: u32,
    pub cmdsize: u32,
    /// number of strings concatenation of zero terminated UTF8 strings. Zero filled at end to align
    pub count: u32,
}

pub const SIZEOF_LINKER_OPTION_COMMAND: usize = 12;

/// The symseg_command contains the offset and size of the GNU style
/// symbol table information as described in the header file <symseg.h>.
/// The symbol roots of the symbol segments must also be aligned properly
/// in the file.  So the requirement of keeping the offsets aligned to a
/// multiple of a 4 bytes translates to the length field of the symbol
/// roots also being a multiple of a long.  Also the padding must again be
/// zeroed. (THIS IS OBSOLETE and no longer supported).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct SymsegCommand {
    /// LC_SYMSEG
    pub cmd: u32,
    /// sizeof(struct symseg_command)
    pub cmdsize: u32,
    /// symbol segment offset
    pub offset: u32,
    /// symbol segment size in bytes
    pub size: u32,
}

pub const SIZEOF_SYMSEG_COMMAND: usize = 16;

/// The ident_command contains a free format string table following the
/// ident_command structure.  The strings are null terminated and the size of
/// the command is padded out with zero bytes to a multiple of 4 bytes/
/// (THIS IS OBSOLETE and no longer supported).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct IdentCommand {
    /// LC_IDENT
    pub cmd: u32,
    /// strings that follow this command
    pub cmdsize: u32,
}

pub const SIZEOF_IDENT_COMMAND: usize = 8;

/// The fvmfile_command contains a reference to a file to be loaded at the
/// specified virtual address.  (Presently, this command is reserved for
/// internal use.  The kernel ignores this command when loading a program into
/// memory).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct FvmfileCommand {
    /// LC_FVMFILE
    pub cmd: u32,
    /// includes pathname string
    pub cmdsize: u32,
    /// files pathname
    pub name: LcStr,
    /// files virtual address
    pub header_addr: u32,
}

pub const SIZEOF_FVMFILE_COMMAND: usize = 16;

/// The entry_point_command is a replacement for thread_command.
/// It is used for main executables to specify the location (file offset)
/// of main().  If -stack_size was used at link time, the stacksize
/// field will contain the stack size need for the main thread.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct EntryPointCommand {
    pub cmd: u32,
    pub cmdsize: u32,
    /// uint64_t file __TEXT offset of main
    pub entryoff: u64,
    /// uint64_t if not zero, initial stack size
    pub stacksize: u64,
}

pub const SIZEOF_ENTRY_POINT_COMMAND: usize = 24;

/// The source_version_command is an optional load command containing
/// the version of the sources used to build the binary.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct SourceVersionCommand {
    /// LC_SOURCE_VERSION
    pub cmd: u32,
    pub cmdsize: u32,
    /// A.B.C.D.E packed as a24.b10.c10.d10.e10
    pub version: u64,
}

/// The LC_DATA_IN_CODE load commands uses a linkedit_data_command
/// to point to an array of data_in_code_entry entries. Each entry
/// describes a range of data in a code section.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pread, Pwrite, SizeWith)]
pub struct DataInCodeEntry {
    /// from mach_header to start of data range
    pub offset: u32,
    /// number of bytes in data range
    pub length: u16,
    /// a DICE_KIND_* value
    pub kind: u16,
}

///////////////////////////////////////
// Constants, et. al
///////////////////////////////////////

pub const LC_REQ_DYLD: u32 = 0x80000000;
pub const LC_LOAD_WEAK_DYLIB: u32 = 0x18 | LC_REQ_DYLD;
pub const LC_RPATH: u32 = 0x1c | LC_REQ_DYLD;
pub const LC_REEXPORT_DYLIB: u32 = 0x1f | LC_REQ_DYLD;
pub const LC_DYLD_INFO_ONLY: u32 = 0x22 | LC_REQ_DYLD;
pub const LC_LOAD_UPWARD_DYLIB: u32 = 0x23 | LC_REQ_DYLD;
pub const LC_MAIN: u32 = 0x28 | LC_REQ_DYLD;
pub const LC_SEGMENT: u32 = 0x1;
pub const LC_SYMTAB: u32 = 0x2;
pub const LC_SYMSEG: u32 = 0x3;
pub const LC_THREAD: u32 = 0x4;
pub const LC_UNIXTHREAD: u32 = 0x5;
pub const LC_LOADFVMLIB: u32 = 0x6;
pub const LC_IDFVMLIB: u32 = 0x7;
pub const LC_IDENT: u32 = 0x8;
pub const LC_FVMFILE: u32 = 0x9;
pub const LC_PREPAGE: u32 = 0xa;
pub const LC_DYSYMTAB: u32 = 0xb;
pub const LC_LOAD_DYLIB: u32 = 0xc;
pub const LC_ID_DYLIB: u32 = 0xd;
pub const LC_LOAD_DYLINKER: u32 = 0xe;
pub const LC_ID_DYLINKER: u32 = 0xf;
pub const LC_PREBOUND_DYLIB: u32 = 0x10;
pub const LC_ROUTINES: u32 = 0x11;
pub const LC_SUB_FRAMEWORK: u32 = 0x12;
pub const LC_SUB_UMBRELLA: u32 = 0x13;
pub const LC_SUB_CLIENT: u32 = 0x14;
pub const LC_SUB_LIBRARY: u32 = 0x15;
pub const LC_TWOLEVEL_HINTS: u32 = 0x16;
pub const LC_PREBIND_CKSUM: u32 = 0x17;
pub const LC_SEGMENT_64: u32 = 0x19;
pub const LC_ROUTINES_64: u32 = 0x1a;
pub const LC_UUID: u32 = 0x1b;
pub const LC_CODE_SIGNATURE: u32 = 0x1d;
pub const LC_SEGMENT_SPLIT_INFO: u32 = 0x1e;
pub const LC_LAZY_LOAD_DYLIB: u32 = 0x20;
pub const LC_ENCRYPTION_INFO: u32 = 0x21;
pub const LC_DYLD_INFO: u32 = 0x22;
pub const LC_VERSION_MIN_MACOSX: u32 = 0x24;
pub const LC_VERSION_MIN_IPHONEOS: u32 = 0x25;
pub const LC_FUNCTION_STARTS: u32 = 0x26;
pub const LC_DYLD_ENVIRONMENT: u32 = 0x27;
pub const LC_DATA_IN_CODE: u32 = 0x29;
pub const LC_SOURCE_VERSION: u32 = 0x2A;
pub const LC_DYLIB_CODE_SIGN_DRS: u32 = 0x2B;
pub const LC_ENCRYPTION_INFO_64: u32 = 0x2C;
pub const LC_LINKER_OPTION: u32 = 0x2D;
pub const LC_LINKER_OPTIMIZATION_HINT: u32 = 0x2E;

pub fn cmd_to_str(cmd: u32) -> &'static str {
    match cmd {
        LC_SEGMENT => "LC_SEGMENT",
        LC_SYMTAB => "LC_SYMTAB",
        LC_SYMSEG => "LC_SYMSEG",
        LC_THREAD => "LC_THREAD",
        LC_UNIXTHREAD => "LC_UNIXTHREAD",
        LC_LOADFVMLIB => "LC_LOADFVMLIB",
        LC_IDFVMLIB => "LC_IDFVMLIB",
        LC_IDENT => "LC_IDENT",
        LC_FVMFILE => "LC_FVMFILE",
        LC_PREPAGE => "LC_PREPAGE",
        LC_DYSYMTAB => "LC_DYSYMTAB",
        LC_LOAD_DYLIB => "LC_LOAD_DYLIB",
        LC_ID_DYLIB => "LC_ID_DYLIB",
        LC_LOAD_DYLINKER => "LC_LOAD_DYLINKER",
        LC_ID_DYLINKER => "LC_ID_DYLINKER",
        LC_PREBOUND_DYLIB => "LC_PREBOUND_DYLIB",
        LC_ROUTINES => "LC_ROUTINES",
        LC_SUB_FRAMEWORK => "LC_SUB_FRAMEWORK",
        LC_SUB_UMBRELLA => "LC_SUB_UMBRELLA",
        LC_SUB_CLIENT => "LC_SUB_CLIENT",
        LC_SUB_LIBRARY => "LC_SUB_LIBRARY",
        LC_TWOLEVEL_HINTS => "LC_TWOLEVEL_HINTS",
        LC_PREBIND_CKSUM => "LC_PREBIND_CKSUM",
        LC_LOAD_WEAK_DYLIB => "LC_LOAD_WEAK_DYLIB",
        LC_SEGMENT_64 => "LC_SEGMENT_64",
        LC_ROUTINES_64 => "LC_ROUTINES_64",
        LC_UUID => "LC_UUID",
        LC_RPATH => "LC_RPATH",
        LC_CODE_SIGNATURE => "LC_CODE_SIGNATURE",
        LC_SEGMENT_SPLIT_INFO => "LC_SEGMENT_SPLIT_INFO",
        LC_REEXPORT_DYLIB => "LC_REEXPORT_DYLIB",
        LC_LAZY_LOAD_DYLIB => "LC_LAZY_LOAD_DYLIB",
        LC_ENCRYPTION_INFO => "LC_ENCRYPTION_INFO",
        LC_DYLD_INFO => "LC_DYLD_INFO",
        LC_DYLD_INFO_ONLY => "LC_DYLD_INFO_ONLY",
        LC_LOAD_UPWARD_DYLIB => "LC_LOAD_UPWARD_DYLIB",
        LC_VERSION_MIN_MACOSX => "LC_VERSION_MIN_MACOSX",
        LC_VERSION_MIN_IPHONEOS => "LC_VERSION_MIN_IPHONEOS",
        LC_FUNCTION_STARTS => "LC_FUNCTION_STARTS",
        LC_DYLD_ENVIRONMENT => "LC_DYLD_ENVIRONMENT",
        LC_MAIN => "LC_MAIN",
        LC_DATA_IN_CODE => "LC_DATA_IN_CODE",
        LC_SOURCE_VERSION => "LC_SOURCE_VERSION",
        LC_DYLIB_CODE_SIGN_DRS => "LC_DYLIB_CODE_SIGN_DRS",
        LC_ENCRYPTION_INFO_64 => "LC_ENCRYPTION_INFO_64",
        LC_LINKER_OPTION => "LC_LINKER_OPTION",
        LC_LINKER_OPTIMIZATION_HINT => "LC_LINKER_OPTIMIZATION_HINT",
        _ => "LC_UNKNOWN",
    }
}

///////////////////////////////////////////
// Typesafe Command Variants
///////////////////////////////////////////

#[derive(Debug)]
/// The various load commands as a cast-free variant/enum
pub enum CommandVariant {
    Segment32              (SegmentCommand32),
    Segment64              (SegmentCommand64),
    Uuid                   (UuidCommand),
    Symtab                 (SymtabCommand),
    Symseg                 (SymsegCommand),
    Thread                 (ThreadCommand),
    Unixthread             (ThreadCommand),
    LoadFvmlib             (FvmlibCommand),
    IdFvmlib               (FvmlibCommand),
    Ident                  (IdentCommand),
    Fvmfile                (FvmfileCommand),
    Prepage                (LoadCommandHeader),
    Dysymtab               (DysymtabCommand),
    LoadDylib              (DylibCommand),
    IdDylib                (DylibCommand),
    LoadDylinker           (DylinkerCommand),
    IdDylinker             (DylinkerCommand),
    PreboundDylib          (PreboundDylibCommand),
    Routines32             (RoutinesCommand32),
    Routines64             (RoutinesCommand64),
    SubFramework           (SubFrameworkCommand),
    SubUmbrella            (SubUmbrellaCommand),
    SubClient              (SubClientCommand),
    SubLibrary             (SubLibraryCommand),
    TwolevelHints          (TwolevelHintsCommand),
    PrebindCksum           (PrebindCksumCommand),
    LoadWeakDylib          (DylibCommand),
    Rpath                  (RpathCommand),
    CodeSignature          (LinkeditDataCommand),
    SegmentSplitInfo       (LinkeditDataCommand),
    ReexportDylib          (DylibCommand),
    LazyLoadDylib          (DylibCommand),
    EncryptionInfo32       (EncryptionInfoCommand32),
    EncryptionInfo64       (EncryptionInfoCommand64),
    DyldInfo               (DyldInfoCommand),
    DyldInfoOnly           (DyldInfoCommand),
    LoadUpwardDylib        (DylibCommand),
    VersionMinMacosx       (VersionMinCommand),
    VersionMinIphoneos     (VersionMinCommand),
    FunctionStarts         (LinkeditDataCommand),
    DyldEnvironment        (DylinkerCommand),
    Main                   (EntryPointCommand),
    DataInCode             (LinkeditDataCommand),
    SourceVersion          (SourceVersionCommand),
    DylibCodeSignDrs       (LinkeditDataCommand),
    LinkerOption           (LinkeditDataCommand),
    LinkerOptimizationHint (LinkeditDataCommand),
    Unimplemented          (LoadCommandHeader),
}

impl<'a> ctx::TryFromCtx<'a, Endian> for CommandVariant {
    type Error = error::Error;
    type Size = usize;
    fn try_from_ctx(bytes: &'a [u8], le: Endian) -> error::Result<(Self, Self::Size)> {
        use scroll::{Pread};
        use self::CommandVariant::*;
        let lc = bytes.pread_with::<LoadCommandHeader>(0, le)?;
        let size = lc.cmdsize as usize;
        //println!("offset {:#x} cmd: {:#x} size: {:?} ctx: {:?}", offset, lc.cmd, size, le);
        if size > bytes.len() { return Err(error::Error::Malformed(format!("{} has size larger than remainder of binary: {:?}", &lc, bytes.len()))) }
        match lc.cmd {
            LC_SEGMENT    => {              let comm = bytes.pread_with::<SegmentCommand32>       (0, le)?;  Ok((Segment32              (comm), size))},
            LC_SEGMENT_64 => {              let comm = bytes.pread_with::<SegmentCommand64>       (0, le)?;  Ok((Segment64              (comm), size))},
            LC_DYSYMTAB => {                let comm = bytes.pread_with::<DysymtabCommand>        (0, le)?;  Ok((Dysymtab               (comm), size))},
            LC_LOAD_DYLINKER => {           let comm = bytes.pread_with::<DylinkerCommand>        (0, le)?;  Ok((LoadDylinker           (comm), size))},
            LC_ID_DYLINKER => {             let comm = bytes.pread_with::<DylinkerCommand>        (0, le)?;  Ok((IdDylinker             (comm), size))},
            LC_UUID => {                    let comm = bytes.pread_with::<UuidCommand>            (0, le)?;  Ok((Uuid                   (comm), size))},
            LC_SYMTAB => {                  let comm = bytes.pread_with::<SymtabCommand>          (0, le)?;  Ok((Symtab                 (comm), size))},
            LC_SYMSEG => {                  let comm = bytes.pread_with::<SymsegCommand>          (0, le)?;  Ok((Symseg                 (comm), size))},
            LC_THREAD => {                  let comm = bytes.pread_with::<ThreadCommand>          (0, le)?;  Ok((Thread                 (comm), size))},
            LC_UNIXTHREAD => {              let comm = bytes.pread_with::<ThreadCommand>          (0, le)?;  Ok((Unixthread             (comm), size))},
            LC_LOADFVMLIB => {              let comm = bytes.pread_with::<FvmlibCommand>          (0, le)?;  Ok((LoadFvmlib             (comm), size))},
            LC_IDFVMLIB => {                let comm = bytes.pread_with::<FvmlibCommand>          (0, le)?;  Ok((IdFvmlib               (comm), size))},
            LC_IDENT => {                   let comm = bytes.pread_with::<IdentCommand>           (0, le)?;  Ok((Ident                  (comm), size))},
            LC_FVMFILE => {                 let comm = bytes.pread_with::<FvmfileCommand>         (0, le)?;  Ok((Fvmfile                (comm), size))},
            LC_PREPAGE => {                 let comm = bytes.pread_with::<LoadCommandHeader>      (0, le)?;  Ok((Prepage                (comm), size))},
            LC_LOAD_DYLIB => {              let comm = bytes.pread_with::<DylibCommand>           (0, le)?;  Ok((LoadDylib              (comm), size))},
            LC_ID_DYLIB => {                let comm = bytes.pread_with::<DylibCommand>           (0, le)?;  Ok((IdDylib                (comm), size))},
            LC_PREBOUND_DYLIB => {          let comm = bytes.pread_with::<PreboundDylibCommand>   (0, le)?;  Ok((PreboundDylib          (comm), size))},
            LC_ROUTINES => {                let comm = bytes.pread_with::<RoutinesCommand32>      (0, le)?;  Ok((Routines32             (comm), size))},
            LC_ROUTINES_64 => {             let comm = bytes.pread_with::<RoutinesCommand64>      (0, le)?;  Ok((Routines64             (comm), size))},
            LC_SUB_FRAMEWORK => {           let comm = bytes.pread_with::<SubFrameworkCommand>    (0, le)?;  Ok((SubFramework           (comm), size))},
            LC_SUB_UMBRELLA => {            let comm = bytes.pread_with::<SubUmbrellaCommand>     (0, le)?;  Ok((SubUmbrella            (comm), size))},
            LC_SUB_CLIENT => {              let comm = bytes.pread_with::<SubClientCommand>       (0, le)?;  Ok((SubClient              (comm), size))},
            LC_SUB_LIBRARY => {             let comm = bytes.pread_with::<SubLibraryCommand>      (0, le)?;  Ok((SubLibrary             (comm), size))},
            LC_TWOLEVEL_HINTS => {          let comm = bytes.pread_with::<TwolevelHintsCommand>   (0, le)?;  Ok((TwolevelHints          (comm), size))},
            LC_PREBIND_CKSUM => {           let comm = bytes.pread_with::<PrebindCksumCommand>    (0, le)?;  Ok((PrebindCksum           (comm), size))},
            LC_LOAD_WEAK_DYLIB => {         let comm = bytes.pread_with::<DylibCommand>           (0, le)?;  Ok((LoadWeakDylib          (comm), size))},
            LC_RPATH => {                   let comm = bytes.pread_with::<RpathCommand>           (0, le)?;  Ok((Rpath                  (comm), size))},
            LC_CODE_SIGNATURE => {          let comm = bytes.pread_with::<LinkeditDataCommand>    (0, le)?;  Ok((CodeSignature          (comm), size))},
            LC_SEGMENT_SPLIT_INFO => {      let comm = bytes.pread_with::<LinkeditDataCommand>    (0, le)?;  Ok((SegmentSplitInfo       (comm), size))},
            LC_REEXPORT_DYLIB => {          let comm = bytes.pread_with::<DylibCommand>           (0, le)?;  Ok((ReexportDylib          (comm), size))},
            LC_LAZY_LOAD_DYLIB => {         let comm = bytes.pread_with::<DylibCommand>           (0, le)?;  Ok((LazyLoadDylib          (comm), size))},
            LC_ENCRYPTION_INFO => {         let comm = bytes.pread_with::<EncryptionInfoCommand32>(0, le)?;  Ok((EncryptionInfo32       (comm), size))},
            LC_ENCRYPTION_INFO_64 => {      let comm = bytes.pread_with::<EncryptionInfoCommand64>(0, le)?;  Ok((EncryptionInfo64       (comm), size))},
            LC_DYLD_INFO => {               let comm = bytes.pread_with::<DyldInfoCommand>        (0, le)?;  Ok((DyldInfo               (comm), size))},
            LC_DYLD_INFO_ONLY => {          let comm = bytes.pread_with::<DyldInfoCommand>        (0, le)?;  Ok((DyldInfoOnly           (comm), size))},
            LC_LOAD_UPWARD_DYLIB => {       let comm = bytes.pread_with::<DylibCommand>           (0, le)?;  Ok((LoadUpwardDylib        (comm), size))},
            LC_VERSION_MIN_MACOSX => {      let comm = bytes.pread_with::<VersionMinCommand>      (0, le)?;  Ok((VersionMinMacosx       (comm), size))},
            LC_VERSION_MIN_IPHONEOS => {    let comm = bytes.pread_with::<VersionMinCommand>      (0, le)?;  Ok((VersionMinIphoneos     (comm), size))},
            LC_FUNCTION_STARTS => {         let comm = bytes.pread_with::<LinkeditDataCommand>    (0, le)?;  Ok((FunctionStarts         (comm), size))},
            LC_DYLD_ENVIRONMENT => {        let comm = bytes.pread_with::<DylinkerCommand>        (0, le)?;  Ok((DyldEnvironment        (comm), size))},
            LC_MAIN => {                    let comm = bytes.pread_with::<EntryPointCommand>      (0, le)?;  Ok((Main                   (comm), size))},
            LC_DATA_IN_CODE => {            let comm = bytes.pread_with::<LinkeditDataCommand>    (0, le)?;  Ok((DataInCode             (comm), size))},
            LC_SOURCE_VERSION => {          let comm = bytes.pread_with::<SourceVersionCommand>   (0, le)?;  Ok((SourceVersion          (comm), size))},
            LC_DYLIB_CODE_SIGN_DRS => {     let comm = bytes.pread_with::<LinkeditDataCommand>    (0, le)?;  Ok((DylibCodeSignDrs       (comm), size))},
            LC_LINKER_OPTION => {           let comm = bytes.pread_with::<LinkeditDataCommand>    (0, le)?;  Ok((LinkerOption           (comm), size))},
            LC_LINKER_OPTIMIZATION_HINT => {let comm = bytes.pread_with::<LinkeditDataCommand>    (0, le)?;  Ok((LinkerOptimizationHint (comm), size))},
            _ =>                                                                                             Ok((Unimplemented          (lc.clone()), size)),
        }
    }
}

impl CommandVariant {
    pub fn cmdsize(&self) -> usize {
        use self::CommandVariant::*;
        let cmdsize = match *self {
            Segment32              (comm) => comm.cmdsize,
            Segment64              (comm) => comm.cmdsize,
            Uuid                   (comm) => comm.cmdsize,
            Symtab                 (comm) => comm.cmdsize,
            Symseg                 (comm) => comm.cmdsize,
            Thread                 (comm) => comm.cmdsize,
            Unixthread             (comm) => comm.cmdsize,
            LoadFvmlib             (comm) => comm.cmdsize,
            IdFvmlib               (comm) => comm.cmdsize,
            Ident                  (comm) => comm.cmdsize,
            Fvmfile                (comm) => comm.cmdsize,
            Prepage                (comm) => comm.cmdsize,
            Dysymtab               (comm) => comm.cmdsize,
            LoadDylib              (comm) => comm.cmdsize,
            IdDylib                (comm) => comm.cmdsize,
            LoadDylinker           (comm) => comm.cmdsize,
            IdDylinker             (comm) => comm.cmdsize,
            PreboundDylib          (comm) => comm.cmdsize,
            Routines32             (comm) => comm.cmdsize,
            Routines64             (comm) => comm.cmdsize,
            SubFramework           (comm) => comm.cmdsize,
            SubUmbrella            (comm) => comm.cmdsize,
            SubClient              (comm) => comm.cmdsize,
            SubLibrary             (comm) => comm.cmdsize,
            TwolevelHints          (comm) => comm.cmdsize,
            PrebindCksum           (comm) => comm.cmdsize,
            LoadWeakDylib          (comm) => comm.cmdsize,
            Rpath                  (comm) => comm.cmdsize,
            CodeSignature          (comm) => comm.cmdsize,
            SegmentSplitInfo       (comm) => comm.cmdsize,
            ReexportDylib          (comm) => comm.cmdsize,
            LazyLoadDylib          (comm) => comm.cmdsize,
            EncryptionInfo32       (comm) => comm.cmdsize,
            EncryptionInfo64       (comm) => comm.cmdsize,
            DyldInfo               (comm) => comm.cmdsize,
            DyldInfoOnly           (comm) => comm.cmdsize,
            LoadUpwardDylib        (comm) => comm.cmdsize,
            VersionMinMacosx       (comm) => comm.cmdsize,
            VersionMinIphoneos     (comm) => comm.cmdsize,
            FunctionStarts         (comm) => comm.cmdsize,
            DyldEnvironment        (comm) => comm.cmdsize,
            Main                   (comm) => comm.cmdsize,
            DataInCode             (comm) => comm.cmdsize,
            SourceVersion          (comm) => comm.cmdsize,
            DylibCodeSignDrs       (comm) => comm.cmdsize,
            LinkerOption           (comm) => comm.cmdsize,
            LinkerOptimizationHint (comm) => comm.cmdsize,
            Unimplemented          (comm) => comm.cmdsize,
        };
        cmdsize as usize
    }
    pub fn cmd(&self) -> u32 {
        use self::CommandVariant::*;
        let cmd = match *self {
            Segment32              (comm) => comm.cmd,
            Segment64              (comm) => comm.cmd,
            Uuid                   (comm) => comm.cmd,
            Symtab                 (comm) => comm.cmd,
            Symseg                 (comm) => comm.cmd,
            Thread                 (comm) => comm.cmd,
            Unixthread             (comm) => comm.cmd,
            LoadFvmlib             (comm) => comm.cmd,
            IdFvmlib               (comm) => comm.cmd,
            Ident                  (comm) => comm.cmd,
            Fvmfile                (comm) => comm.cmd,
            Prepage                (comm) => comm.cmd,
            Dysymtab               (comm) => comm.cmd,
            LoadDylib              (comm) => comm.cmd,
            IdDylib                (comm) => comm.cmd,
            LoadDylinker           (comm) => comm.cmd,
            IdDylinker             (comm) => comm.cmd,
            PreboundDylib          (comm) => comm.cmd,
            Routines32             (comm) => comm.cmd,
            Routines64             (comm) => comm.cmd,
            SubFramework           (comm) => comm.cmd,
            SubUmbrella            (comm) => comm.cmd,
            SubClient              (comm) => comm.cmd,
            SubLibrary             (comm) => comm.cmd,
            TwolevelHints          (comm) => comm.cmd,
            PrebindCksum           (comm) => comm.cmd,
            LoadWeakDylib          (comm) => comm.cmd,
            Rpath                  (comm) => comm.cmd,
            CodeSignature          (comm) => comm.cmd,
            SegmentSplitInfo       (comm) => comm.cmd,
            ReexportDylib          (comm) => comm.cmd,
            LazyLoadDylib          (comm) => comm.cmd,
            EncryptionInfo32       (comm) => comm.cmd,
            EncryptionInfo64       (comm) => comm.cmd,
            DyldInfo               (comm) => comm.cmd,
            DyldInfoOnly           (comm) => comm.cmd,
            LoadUpwardDylib        (comm) => comm.cmd,
            VersionMinMacosx       (comm) => comm.cmd,
            VersionMinIphoneos     (comm) => comm.cmd,
            FunctionStarts         (comm) => comm.cmd,
            DyldEnvironment        (comm) => comm.cmd,
            Main                   (comm) => comm.cmd,
            DataInCode             (comm) => comm.cmd,
            SourceVersion          (comm) => comm.cmd,
            DylibCodeSignDrs       (comm) => comm.cmd,
            LinkerOption           (comm) => comm.cmd,
            LinkerOptimizationHint (comm) => comm.cmd,
            Unimplemented          (comm) => comm.cmd,
        };
        cmd
    }
}

#[derive(Debug)]
/// A tagged LoadCommand union
pub struct LoadCommand {
    /// The offset this load command occurs at
    pub offset: usize,
    /// Which load command this is inside a variant
    pub command: CommandVariant,
}

impl LoadCommand {
    /// Parse a load command from `bytes` at `offset` with the `le` endianness
    pub fn parse(bytes: &[u8], mut offset: &mut usize, le: scroll::Endian) -> error::Result<Self> {
        let start = *offset;
        let command = bytes.pread_with::<CommandVariant>(start, le)?;
        let size = command.cmdsize();
        *offset = start + size;
        Ok(LoadCommand { offset: start, command: command })
    }
}

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

/// Generalized 32/64 bit Section, with attached section data
pub struct Section<'a> {
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
    /// The data inside this section
    pub data:      &'a [u8],
}

impl<'a> Section<'a> {
    /// The name of this section
    pub fn name(&self) -> scroll::Result<&str> {
        self.sectname.pread::<&str>(0)
    }
    /// The containing segment's name
    pub fn segname(&self) -> scroll::Result<&str> {
        self.segname.pread::<&str>(0)
    }
    pub fn iter_relocations(&self, ctx: scroll::Endian) -> RelocationIterator {
        RelocationIterator {
            offset: self.reloff as usize,
            nrelocs: self.nreloc as usize,
            count: 0,
            data: self.data,
            ctx: ctx,
        }
    }
}

impl<'a> fmt::Debug for Section<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Section")
            .field("sectname", &self.sectname.pread::<&str>(0).unwrap())
            .field("segname",  &self.segname.pread::<&str>(0).unwrap())
            .field("addr",     &self.addr)
            .field("size",     &self.size)
            .field("offset",   &self.offset)
            .field("align",    &self.align)
            .field("reloff",   &self.reloff)
            .field("nreloc",   &self.nreloc)
            .field("flags",    &self.flags)
            .field("data",     &self.data.len())
            .field("relocations",     &self.iter_relocations(scroll::LE).collect::<Vec<_>>())
            .finish()
    }
}

impl<'a> ctx::TryFromCtx<'a, Section32> for Section<'a> {
    type Error = scroll::Error;
    type Size = usize;
    fn try_from_ctx(bytes: &'a [u8], section: Section32) -> Result<(Self, Self::Size), Self::Error> {
        Ok((Section {
            sectname: section.sectname,
            segname:  section.segname,
            addr:     section.addr as u64,
            size:     section.size as u64,
            offset:   section.offset,
            align:    section.align,
            reloff:   section.reloff,
            nreloc:   section.nreloc,
            flags:    section.flags,
            data:     bytes
        }, SIZEOF_SECTION_32))
    }
}

impl<'a> TryFromCtx<'a, Section64> for Section<'a> {
    type Error = scroll::Error;
    type Size = usize;
    fn try_from_ctx(bytes: &'a [u8], section: Section64) -> Result<(Self, Self::Size), Self::Error> {
        Ok((Section {
            sectname: section.sectname,
            segname:  section.segname,
            addr:     section.addr,
            size:     section.size,
            offset:   section.offset,
            align:    section.align,
            reloff:   section.reloff,
            nreloc:   section.nreloc,
            flags:    section.flags,
            data:     bytes
        }, SIZEOF_SECTION_64))
    }
}

impl<'a> TryFromCtx<'a, container::Ctx> for Section<'a> {
    type Error = scroll::Error;
    type Size = usize;
    fn try_from_ctx(bytes: &'a [u8], ctx: container::Ctx) -> Result<(Self, Self::Size), Self::Error> {
        match ctx.container {
            container::Container::Little => {
                let section = Section::try_from_ctx(bytes, bytes.pread_with::<Section32>(0, ctx.le)?)?;
                Ok(section)
            },
            container::Container::Big    => {
                let section = Section::try_from_ctx(bytes, bytes.pread_with::<Section64>(0, ctx.le)?)?;
                Ok(section)
            },
        }
    }
}

impl<'a> ctx::SizeWith<container::Ctx> for Section<'a> {
    type Units = usize;
    fn size_with(ctx: &container::Ctx) -> usize {
        match ctx.container {
            container::Container::Little => SIZEOF_SECTION_32,
            container::Container::Big    => SIZEOF_SECTION_64,
        }
    }
}

pub struct SectionIterator<'a> {
    data: &'a [u8],
    count: usize,
    offset: usize,
    idx: usize,
    ctx: container::Ctx,
}

impl<'a> Iterator for SectionIterator<'a> {
    type Item = error::Result<Section<'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.count >= self.count {
            None
        } else {
            self.idx += 1;
            Some(self.data.gread_with(&mut self.offset, self.ctx).map_err(|e| e.into()))
            // match self.data.gread_with(&mut self.offset, self.ctx) {
            //     Ok(res) => Some(Ok(res)),
            //     Err(e) => Some(Err(e.into()))
            // }
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

impl<'a> Segment<'a> {
    /// Get the name of this segment
    pub fn name(&self) -> error::Result<&str> {
        Ok(self.segname.pread::<&str>(0)?)
    }
    /// Get the sections from this segment
    pub fn sections(&self) -> error::Result<Vec<Section<'a>>> {
        use scroll::Pread;
        let nsects = self.nsects as usize;
        let mut sections = Vec::with_capacity(nsects);
        let offset = &mut (self.offset + Self::size_with(&self.ctx));
        for _ in 0..nsects {
            let section = self.raw_data.gread_with::<Section<'a>>(offset, self.ctx)?;
            sections.push(section);
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

impl<'a> Segments<'a> {
    /// Construct a new generalized segment container from this `ctx`
    pub fn new(ctx: container::Ctx) -> Self {
        Segments {
            segments: Vec::new(),
            ctx: ctx,
        }
    }
    /// Get every section from every segment
    pub fn sections(&self) -> error::Result<Vec<Vec<Section<'a>>>> {
        let mut sections = Vec::new();
        for segment in &self.segments {
            sections.push(segment.sections()?);
        }
        Ok(sections)
    }
}
