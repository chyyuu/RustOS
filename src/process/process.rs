use alloc::String;
use arch::interrupt::*;
use memory::{MemoryArea, MemoryAttr, MemorySet};
use xmas_elf::{ElfFile, header::HeaderPt2, program::{Flags, ProgramHeader}};

#[derive(Debug)]
pub struct Process {
    pub(in process) pid: Pid,
    pub(in process) parent: Pid,
    pub(in process) name: String,
    pub(in process) memory_set: MemorySet,
    pub(in process) status: Status,
    pub(in process) context: Context,
    pub(in process) is_user: bool,
}

pub type Pid = usize;
pub type ErrorCode = usize;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Status {
    Ready,
    Running,
    Waiting(Pid),
    Sleeping,
    Exited(ErrorCode),
}

impl Process {
    /// Make a new kernel thread
    pub fn new(name: &str, entry: extern fn(usize) -> !, arg: usize) -> Self {
        let ms = MemorySet::new(7);
        let data = InitStack::new_kernel_thread(entry, arg, ms.kstack_top());
        let context = unsafe { data.push_at(ms.kstack_top()) };

        Process {
            pid: 0,
            parent: 0,
            name: String::from(name),
            memory_set: ms,
            status: Status::Ready,
            context,
            is_user: false,
        }
    }

    /// Make the first kernel thread `initproc`
    /// Should be called only once
    pub fn new_init(ms: MemorySet) -> Self {
        assert_has_not_been_called!();
        Process {
            pid: 0,
            parent: 0,
            name: String::from("init"),
            memory_set: ms,
            status: Status::Running,
            context: unsafe { Context::null() }, // will be set at first schedule
            is_user: false,
        }
    }

    /// Make a new user thread
    /// The program elf data is placed at [begin, end)
    /// uCore x86 32bit program is planned to be supported.
    pub fn new_user(data: &[u8]) -> Self {
        // Parse elf
        let elf = ElfFile::new(data).expect("failed to read elf");
        let is32 = match elf.header.pt2 {
            HeaderPt2::Header32(_) => true,
            HeaderPt2::Header64(_) => false,
        };

        // User stack
        use consts::{USER_STACK_OFFSET, USER_STACK_SIZE, USER_TCB_OFFSET};
        let (user_stack_buttom, user_stack_top) = match is32 {
            true => (USER_TCB_OFFSET, USER_TCB_OFFSET + USER_STACK_SIZE),
            false => (USER_STACK_OFFSET, USER_STACK_OFFSET + USER_STACK_SIZE),
        };

        // Make page table
        let mut memory_set = MemorySet::from(&elf);
        memory_set.push(MemoryArea::new(user_stack_buttom, user_stack_top, MemoryAttr::default().user(), "user_stack"));
        trace!("{:#x?}", memory_set);

        let entry_addr = match elf.header.pt2 {
            HeaderPt2::Header32(header) => header.entry_point as usize,
            HeaderPt2::Header64(header) => header.entry_point as usize,
        };

        // Temporary switch to it, in order to copy data
        memory_set.with(|| {
            for ph in elf.program_iter() {
                let (virt_addr, offset, file_size) = match ph {
                    ProgramHeader::Ph32(ph) => (ph.virtual_addr as usize, ph.offset as usize, ph.file_size as usize),
                    ProgramHeader::Ph64(ph) => (ph.virtual_addr as usize, ph.offset as usize, ph.file_size as usize),
                };
                use core::slice;
                let target = unsafe { slice::from_raw_parts_mut(virt_addr as *mut u8, file_size) };
                target.copy_from_slice(&data[offset..offset + file_size]);
            }
            if is32 {
                unsafe {
                    // TODO: full argc & argv
                    *(user_stack_top as *mut u32).offset(-1) = 0; // argv
                    *(user_stack_top as *mut u32).offset(-2) = 0; // argc
                }
            }
        });


        // Allocate kernel stack and push trap frame
        let data = InitStack::new_user_thread(entry_addr, user_stack_top - 8, is32);
        let context = unsafe { data.push_at(memory_set.kstack_top()) };

        Process {
            pid: 0,
            parent: 0,
            name: String::new(),
            memory_set,
            status: Status::Ready,
            context,
            is_user: true,
        }
    }

    /// Fork
    pub fn fork(&self, tf: &TrapFrame) -> Self {
        assert!(self.is_user);

        // Clone memory set, make a new page table
        let memory_set = self.memory_set.clone(7);

        // Copy data to temp space
        use alloc::Vec;
        let datas: Vec<Vec<u8>> = memory_set.iter().map(|area| {
            Vec::from(unsafe { area.as_slice() })
        }).collect();

        // Temporary switch to it, in order to copy data
        memory_set.with(|| {
            for (area, data) in memory_set.iter().zip(datas.iter()) {
                unsafe { area.as_slice_mut() }.copy_from_slice(data.as_slice())
            }
        });

        // Allocate kernel stack and push trap frame
        let data = InitStack::new_fork(tf);
        let context = unsafe { data.push_at(memory_set.kstack_top()) };

        Process {
            pid: 0,
            parent: self.pid,
            name: self.name.clone() + "_fork",
            memory_set,
            status: Status::Ready,
            context,
            is_user: true,
        }
    }

    pub fn exit_code(&self) -> Option<ErrorCode> {
        match self.status {
            Status::Exited(code) => Some(code),
            _ => None,
        }
    }
}

impl<'a> From<&'a ElfFile<'a>> for MemorySet {
    fn from(elf: &'a ElfFile<'a>) -> Self {
        let mut set = MemorySet::new(7);
        for ph in elf.program_iter() {
            let (virt_addr, mem_size, flags) = match ph {
                ProgramHeader::Ph32(ph) => (ph.virtual_addr as usize, ph.mem_size as usize, ph.flags),
                ProgramHeader::Ph64(ph) => (ph.virtual_addr as usize, ph.mem_size as usize, ph.flags),
            };
            set.push(MemoryArea::new(virt_addr, virt_addr + mem_size, MemoryAttr::from(flags), ""));
        }
        set
    }
}

impl From<Flags> for MemoryAttr {
    fn from(elf_flags: Flags) -> Self {
        let mut flags = MemoryAttr::default().user();
        // TODO: handle readonly
        if elf_flags.is_execute() { flags = flags.execute(); }
        flags
    }
}