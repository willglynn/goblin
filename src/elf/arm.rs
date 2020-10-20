//! Implements Arm extensions for ELF.
//!
//! These data structures are primarily defined by [ELF for the Arm Architecture].
//!
//! [ELF for the Arm Architecture]: https://developer.arm.com/documentation/ihi0044/h/?lang=en

use crate::elf::build_attributes::aeabi::Aeabi;
use crate::elf::header::EM_ARM;
use core::num::NonZeroU8;

// Table 4-2, Arm-specific e_flags

/// This masks an 8-bit version number, the version of the ABI to which this ELF file conforms. A
/// value of 0 denotes unknown conformance.
pub const EF_ARM_ABIMASK: u32 = 0xFF000000;
/// The ELF file contains BE-8 code, suitable for execution on an Arm Architecture v6 processor.
/// This flag must only be set on an executable file.
pub const EF_ARM_BE8: u32 = 0x00800000;
/// Legacy code (ABI version 4 and earlier) generated by gcc-arm-xxx might use these bits.
pub const EF_ARM_GCCMASK: u32 = 0x00400FFF;
/// Set in executable file headers (`e_type` = `ET_EXEC` or `ET_DYN`) to note that the executable
/// file was built to conform to the hardware floating-point procedure-call standard.
///
/// Compatible with legacy (pre version 5) gcc use as EF_ARM_VFP_FLOAT.
pub const EF_ARM_ABI_FLOAT_HARD: u32 = 0x00000400;
/// Set in executable file headers (`e_type` = `ET_EXEC` or `ET_DYN`) to note explicitly that the
/// executable file was built to conform to the software floating-point procedure-call standard (the
/// base standard). If both `EF_ARM_ABI_FLOAT_XXXX` bits are clear, conformance to the base
/// procedure-call standard is implied.
///
/// Compatible with legacy (pre version 5) gcc use as EF_ARM_SOFT_FLOAT.
pub const EF_ARM_ABI_FLOAT_SOFT: u32 = 0x00000200;

pub trait HeaderExt {
    /// If this ELF header provides Arm extensions, return an `ArmElfHeader`.
    fn arm(&self) -> Option<ArmElfHeader>;
}

macro_rules! header {
    ($t:ty) => {
        impl HeaderExt for $t {
            fn arm(&self) -> Option<ArmElfHeader> {
                if self.e_machine == EM_ARM {
                    Some(ArmElfHeader {
                        e_entry_mod_4: (self.e_entry % 4) as u8,
                        e_flags: self.e_flags,
                    })
                } else {
                    None
                }
            }
        }
    };
}

#[cfg(all(feature = "elf32", feature = "elf64", feature = "endian_fd"))]
header!(super::Header);
header!(super::header::header32::Header);
header!(super::header::header64::Header);

/// Arm extensions to the ELF header, as documented in [ELF for the Arm Architecture] § 5.2.
///
/// [ELF for the Arm Architecture]: https://developer.arm.com/documentation/ihi0044/h/?lang=en
#[derive(Debug, Copy, Clone)]
pub struct ArmElfHeader {
    e_entry_mod_4: u8,
    e_flags: u32,
}

impl ArmElfHeader {
    /// The ABI version, if present.
    pub fn abi_version(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(((self.e_flags & EF_ARM_ABIMASK) >> 24) as u8)
    }

    /// To what kind of machine code does `e_entry` pointer point?
    pub fn entrypoint_contents(&self) -> EntrypointContents {
        match self.e_entry_mod_4 {
            0 => EntrypointContents::Arm,
            1 | 3 => EntrypointContents::Thumb,
            _ => EntrypointContents::Reserved,
        }
    }

    /// Is this explicitly using the hardware floating-point calling convention?
    ///
    /// ABI v5 specifies that this flag is set in executable file headers (`e_type` = `ET_EXEC` or
    /// `ET_DYN`) to note that the executable file was built to conform to the hardware
    /// floating-point procedure-call standard.
    ///
    /// Previous standards used `EF_ARM_VFP_FLOAT` which maps to this same field.
    ///
    /// If neither `is_hard_float()` nor `is_soft_float()` is true, one may reasonably assume that
    /// the executable conforms to the software floating-point procedure-call standard, since the
    /// software standard is the base standard.
    pub fn is_hard_float(&self) -> bool {
        (self.e_flags & EF_ARM_ABI_FLOAT_HARD) == EF_ARM_ABI_FLOAT_HARD
    }

    /// Is this explicitly using the software floating-point calling convention?
    ///
    /// ABI v5 specifies that this flag is set in executable file headers (`e_type` = `ET_EXEC` or
    /// `ET_DYN`) to note that the executable file was built to conform to the software
    /// floating-point procedure-call standard, i.e. the base standard.
    ///
    /// Previous standards used `EF_ARM_SOFT_FLOAT` which maps to this same field.
    ///
    /// If neither `is_hard_float()` nor `is_soft_float()` is true, one may reasonably assume that
    /// the executable conforms to the software floating-point procedure-call standard, since the
    /// software standard is the base standard.
    pub fn is_soft_float(&self) -> bool {
        (self.e_flags & EF_ARM_ABI_FLOAT_SOFT) == EF_ARM_ABI_FLOAT_SOFT
    }

    /// Does this executable file contain BE-8 code?
    pub fn contains_be8_code(&self) -> bool {
        (self.e_flags & EF_ARM_BE8) == EF_ARM_BE8
    }
}

/// The kind of machine code present at the `e_entry` code pointer.
///
/// Reference: [ELF for the Arm Architecture] § 5.2.
///
/// [ELF for the Arm Architecture]: https://developer.arm.com/documentation/ihi0044/h/?lang=en
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum EntrypointContents {
    /// The entrypoint contains Arm code.
    Arm,
    /// The entrypoint contains Thumb code.
    Thumb,
    /// This value is reserved.
    Reserved,
}

// Table 5.3: Table 4-4, Processor specific section types
/// Exception Index table
pub const SHT_ARM_EXIDX: u32 = 0x70000001;
/// BPABI DLL dynamic linking pre-emption map
pub const SHT_ARM_PREEMPTMAP: u32 = 0x70000002;
/// Object file compatibility attributes
pub const SHT_ARM_ATTRIBUTES: u32 = 0x70000003;
pub const SHT_ARM_DEBUGOVERLAY: u32 = 0x70000004;
pub const SHT_ARM_OVERLAYSECTION: u32 = 0x70000005;

// Table 5.4: Table 4-5, Processor specific section attribute flags
/// The contents of this section contains only program instructions and no program data.
pub const SHF_ARM_PURECODE: u32 = 0x20000000;

/// A kind of Arm special section, as documented in [ELF for the Arm Architecture] § 5.3.4.
///
/// [ELF for the Arm Architecture]: https://developer.arm.com/documentation/ihi0044/h/?lang=en
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ArmSpecialSection {
    IndexForExceptionUnwinding,
    ExceptionUnwindingTable,
    PreemptionMap,
    BuildAttributes,
    DebugOverlay,
    OverlayTable,
}

impl ArmSpecialSection {
    fn from_sh_type_and_name(sh_type: u32, name: &str) -> Option<Self> {
        const SHT_PROGBITS: u32 = 1;

        // Table 4-6, Arm special sections
        Some(match (name, sh_type) {
            (_, SHT_ARM_EXIDX) if name.starts_with(".ARM.exidx") => {
                ArmSpecialSection::IndexForExceptionUnwinding
            }
            (_, SHT_PROGBITS) if name.starts_with(".ARM.extab") => {
                ArmSpecialSection::ExceptionUnwindingTable
            }
            (".ARM.preemptmap", SHT_ARM_PREEMPTMAP) => ArmSpecialSection::PreemptionMap,
            (".ARM.attributes", SHT_ARM_ATTRIBUTES) => ArmSpecialSection::BuildAttributes,
            ("ARM.debug_overlay", SHT_ARM_DEBUGOVERLAY) => ArmSpecialSection::DebugOverlay,
            ("ARM.overlay_table", SHT_ARM_OVERLAYSECTION) => ArmSpecialSection::OverlayTable,
            _ => return None,
        })
    }
}

pub trait SectionExt {
    /// Which kind of Arm special section this header describes, if any.
    ///
    /// Look up `sh_name` from a `Strtab` using whatever error handling technique is most
    /// appropriate for your application and pass in as `name`.
    fn arm_special_section(&self, name: &str) -> Option<ArmSpecialSection>;
}

macro_rules! section_header {
    ($t:ty) => {
        impl SectionExt for $t {
            fn arm_special_section(&self, name: &str) -> Option<ArmSpecialSection> {
                ArmSpecialSection::from_sh_type_and_name(self.sh_type, name)
            }
        }
    };
}

#[cfg(feature = "alloc")]
section_header!(super::section_header::SectionHeader);
section_header!(super::section_header::section_header32::SectionHeader);
section_header!(super::section_header::section_header64::SectionHeader);

pub trait ElfExt {
    /// Retrieve the `aeabi` build attributes.
    fn aeabi<'a>(&self, bytes: &'a [u8]) -> Result<Aeabi<'a>, AeabiError>;
}

#[derive(Debug)]
pub enum AeabiError {
    /// The ELF header indicates this is not an Arm executable.
    NotArmHeader,
    /// This executable does not contain a build attributes section.
    NoBuildAttributesSection,
    /// The build attributes section header refers to a portion of the executable which does not
    /// exist.
    SectionHeaderOutOfRange,
    /// The build attributes section contains invalid data.
    InvalidBuildAttributes(super::build_attributes::Error),
}

#[cfg(all(feature = "elf32", feature = "elf64", feature = "endian_fd"))]
impl ElfExt for super::Elf<'_> {
    fn aeabi<'a>(&self, bytes: &'a [u8]) -> Result<Aeabi<'a>, AeabiError> {
        use core::convert::TryFrom;

        let endianness = self
            .header
            .endianness()
            .expect("endianness() must succeed after parsing");

        let _arm_header = self.header.arm().ok_or(AeabiError::NotArmHeader)?;
        let build_attributes_section = self
            .section_headers
            .iter()
            .find(|h| {
                if let Some(Ok(name)) = self.shdr_strtab.get(h.sh_name) {
                    h.arm_special_section(name) == Some(ArmSpecialSection::BuildAttributes)
                } else {
                    false
                }
            })
            .ok_or(AeabiError::NoBuildAttributesSection)?;

        let build_attributes = match (
            usize::try_from(build_attributes_section.sh_offset).ok(),
            build_attributes_section
                .sh_offset
                .checked_add(build_attributes_section.sh_size)
                .and_then(|end| usize::try_from(end).ok())
                .filter(|end| *end < bytes.len()),
        ) {
            (Some(start), Some(end)) => &bytes[start..end],
            _ => return Err(AeabiError::SectionHeaderOutOfRange),
        };

        let build_attributes = super::build_attributes::Section::new(build_attributes, endianness)
            .map_err(|e| AeabiError::InvalidBuildAttributes(e.into()))?;

        Aeabi::try_from(build_attributes).map_err(AeabiError::InvalidBuildAttributes)
    }
}