use elf::ElfStream;
use elf::abi::SHT_NOBITS;
use elf::endian::AnyEndian;
use std::error::Error;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

// bundle ELF-related functionality in a struct so that we can keep the file along with the
// elf::ElfStream state around during the runtime's lifetime
// this way, we do not have to open the file more than once
pub(crate) struct AppImageElf {
    path: PathBuf,
    elf: ElfStream<AnyEndian, File>,
}

impl AppImageElf {
    pub(crate) fn new(path: PathBuf) -> Result<Self, Box<dyn Error>> {
        let file = File::open(&path)?;

        let elf = ElfStream::<AnyEndian, File>::open_stream(file)?;

        Ok(Self{path, elf})
    }

    pub(crate) fn size(&self) -> Result<u64, Box<dyn Error>> {
        // an ELF file is typically layed out like this:
        // - ELF header
        // - program header table
        // - section header table
        // - named sections (.text, .rodata, .data, optionally others like .upd_info which we add)
        // we cannot rely on this order, though
        // therefore we just check _every_ item's end and use the max value

        // let's start with the plain ELF header size
        let mut end = self.elf.ehdr.e_ehsize as u64;

        // next, check the program header table end
        if self.elf.ehdr.e_phoff != 0 && self.elf.ehdr.e_phnum != 0 {
            let ph_end = self.elf.ehdr.e_phoff
                .checked_add(
                    (self.elf.ehdr.e_phentsize as u64)
                        .checked_mul(self.elf.ehdr.e_phnum as u64)
                        .ok_or("program-header table size overflow")?,
                )
                .ok_or("program-header table end overflow")?;

            end = end.max(ph_end);
        }

        // next, check the section header table end
        if self.elf.ehdr.e_shoff != 0 && self.elf.ehdr.e_shnum != 0 {
            let sh_end = self.elf.ehdr.e_shoff
                .checked_add(
                    (self.elf.ehdr.e_shentsize as u64)
                        .checked_mul(self.elf.ehdr.e_shnum as u64)
                        .ok_or("section-header table size overflow")?,
                )
                .ok_or("section-header table end overflow")?;

            end = end.max(sh_end);
        }

        // next, check every named section's end
        for section in self.elf.section_headers().iter() {
            if section.sh_type == SHT_NOBITS {
                continue; // Occupies memory, but no bytes in the file.
            }

            let section_end = section
                .sh_offset
                .checked_add(section.sh_size)
                .ok_or("section end overflow")?;

            end = end.max(section_end);
        }

        Ok(end)
    }

    fn read_section(&mut self, name: &str) -> Option<String> {
        if let Some( header) = self.elf.section_header_by_name(name).unwrap_or_else(|error| {
            todo!()
        }) {
            let offset = header.sh_offset;
            let length = header.sh_size;

            // need to open our own file handle, we can't reuse the one from the elf::ElfStream
            // however, we don't just keep one around pointlessly
            // the sections may or may not be read, therefore just open on demand
            let file = File::open(&self.path).unwrap_or_else(|error| {
                println!("{}", error);
                // should never happen
                todo!()
            });

            let mut buf = vec![0u8; length as usize];

            file.read_exact_at(buf.as_mut_slice(), offset).unwrap_or_else(|error| {
                println!("{}", error);
                todo!()
            });

            // note: this function assumes that the data is a plain string
            // it's not safe to make that assumption generally, but for our purposes, it's fine
            // TODO
            Some(String::from_utf8(buf).expect("aaaaaaarghhhh"))
        } else {
            None
        }
    }

    pub(crate) fn update_information(&mut self) -> Option<String> {
        self.read_section(".upd_info")
    }

    pub(crate) fn signature(&mut self) -> Option<String> {
        self.read_section(".sha256_sig")
    }
}
