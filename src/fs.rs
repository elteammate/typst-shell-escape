use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use fuser::{FileAttr, Filesystem, FileType, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request};
use std::ffi::OsStr;
use std::ops::Range;
use std::os::unix::ffi::OsStrExt;
use std::sync::mpsc;
use crate::decode::hex_decode;
use crate::shell::{Command, FinishedCommand, ExecutionResult};

const TTL: Duration = Duration::from_secs(1);

const FILE_INODE_OFFSET: u64 = 256;

const SUCCESS_MESSAGE: &[u8] = b"!";

#[derive(Clone, Debug)]
enum FsEntry {
    ExecFile(),
    WaitFile(),
    ResetFile(),
    AppendDataFile(Vec<u8>),
    ResultFile(Vec<u8>),
}

#[derive(Clone, Debug)]
struct RealizedFsEntry {
    inode: u64,
    entry: FsEntry,
}

impl RealizedFsEntry {
    /// Filesystem attributes of a file. Size is the most important one.
    fn get_attrs(&self) -> FileAttr {
        let size = match &self.entry {
            FsEntry::ExecFile() | FsEntry::WaitFile() | FsEntry::ResetFile() =>
                SUCCESS_MESSAGE.len(),
            FsEntry::AppendDataFile(..) => SUCCESS_MESSAGE.len(),
            FsEntry::ResultFile(data) => data.len(),
        };

        FileAttr {
            ino: self.inode,
            size: size as u64,
            blocks: 0,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind: FileType::RegularFile,
            perm: 0o444,
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: 512,
            flags: 0,
        }
    }

    /// If the entry is a [FsEntry::ResultFile], write the given data to it.
    fn write_result(&mut self, data: Vec<u8>) {
        match &mut self.entry {
            FsEntry::ResultFile(result) => {
                result.clear();
                result.extend_from_slice(&data);
            }
            _ => panic!("Can't write result to non-result file"),
        }
    }

    /// If the entry is a [FsEntry::ResultFile], append the given data to it.
    fn append_result(&mut self, data: Vec<u8>) {
        match &mut self.entry {
            FsEntry::ResultFile(result) => {
                result.extend_from_slice(&data);
            }
            _ => panic!("Can't write result to non-result file"),
        }
    }
}

// TODO: There is too many boilerplate here.
// TODO: add special files for
//     - [ ] list of commands being executed
//     - [ ] lookahead for the command queue
//     - [ ] sleep file, which sends content after some time from a different thread
//     - [ ] random file, which sends random hex string every read

pub struct ShellEscapeFs {
    /// The command buffer, already decoded from hex.
    decoded_command_buffer: Vec<u8>,

    // Inodes of the special files.
    // Those files should be recreated from scratch after every use,
    // because otherwise kernel caching will ruin our life.
    exec_file_inode: u64,
    wait_file_inode: u64,
    reset_file_inode: u64,

    diagnostics_file_inode: u64,
    stdout_file_inode: u64,
    stderr_file_inode: u64,
    log_file_inode: u64,

    /// All the files in the filesystem. Technically causes a memory leak, but
    /// It's going to be a small number of entries anyway
    inodes: HashMap<u64, RealizedFsEntry>,

    /// A channel provided for sending commands to the shell.
    command_channel: mpsc::Sender<Command>,

    /// A channel provided for receiving results from the shell.
    results_channel: mpsc::Receiver<FinishedCommand>,
}

impl ShellEscapeFs {
    pub fn new(
        command_channel: mpsc::Sender<Command>,
        results_channel: mpsc::Receiver<FinishedCommand>,
    ) -> Self {
        let mut inodes = HashMap::new();

        let mut make_entry = |entry: FsEntry| {
            let inode = inodes.len() as u64 + FILE_INODE_OFFSET;
            let result = RealizedFsEntry { inode, entry };
            inodes.insert(inode, result.clone());
            inode
        };

        let exec_file_inode = make_entry(FsEntry::ExecFile());
        let wait_file_inode = make_entry(FsEntry::WaitFile());
        let reset_file_inode = make_entry(FsEntry::ResetFile());
        let diagnostics_file_inode = make_entry(FsEntry::ResultFile(Vec::new()));
        let stdout_file_inode = make_entry(FsEntry::ResultFile(Vec::new()));
        let stderr_file_inode = make_entry(FsEntry::ResultFile(Vec::new()));
        let log_file_inode = make_entry(FsEntry::ResultFile(Vec::new()));

        Self {
            decoded_command_buffer: Vec::new(),
            inodes,
            exec_file_inode,
            wait_file_inode,
            reset_file_inode,
            diagnostics_file_inode,
            stdout_file_inode,
            stderr_file_inode,
            log_file_inode,
            command_channel,
            results_channel,
        }
    }

    // A little boilerplate
    fn exec_file(&mut self) -> &mut RealizedFsEntry {
        self.inodes.get_mut(&self.exec_file_inode)
            .expect("Can't find exec file, should be impossible")
    }

    fn wait_file(&mut self) -> &mut RealizedFsEntry {
        self.inodes.get_mut(&self.wait_file_inode)
            .expect("Can't find wait file, should be impossible")
    }

    fn reset_file(&mut self) -> &mut RealizedFsEntry {
        self.inodes.get_mut(&self.reset_file_inode)
            .expect("Can't find reset file, should be impossible")
    }

    fn diagnostics_file(&mut self) -> &mut RealizedFsEntry {
        self.inodes.get_mut(&self.diagnostics_file_inode)
            .expect("Can't find diagnostics file, should be impossible")
    }

    fn stdout_file(&mut self) -> &mut RealizedFsEntry {
        self.inodes.get_mut(&self.stdout_file_inode)
            .expect("Can't find stdout file, should be impossible")
    }

    fn stderr_file(&mut self) -> &mut RealizedFsEntry {
        self.inodes.get_mut(&self.stderr_file_inode)
            .expect("Can't find stderr file, should be impossible")
    }

    fn log_file(&mut self) -> &mut RealizedFsEntry {
        self.inodes.get_mut(&self.log_file_inode)
            .expect("Can't find log file, should be impossible")
    }

    /// Write a message to a log file. The buffer content is added to the message.
    fn log(&mut self, message: &str) {
        let message = format!(
            "[buf={}] {}\n",
            String::from_utf8_lossy(&self.decoded_command_buffer),
            message
        );

        self.log_file().append_result(message.as_bytes().to_vec());
    }

    /// Filesystem attributes of the root directory.
    fn root_attrs(&self) -> FileAttr {
        FileAttr {
            ino: 1,
            size: 0,
            blocks: 0,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind: FileType::Directory,
            perm: 0o555,
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: 512,
            flags: 0,
        }
    }

    /// Given a filesystem entry, adds it to the filesystem
    fn make_entry(&mut self, entry: FsEntry) -> &RealizedFsEntry {
        let inode = self.inodes.len() as u64 + FILE_INODE_OFFSET;
        let result = RealizedFsEntry {
            inode: self.inodes.len() as u64 + FILE_INODE_OFFSET,
            entry,
        };
        self.inodes.insert(inode, result);
        self.inodes.get(&inode).expect("Can't find inode we just inserted")
    }

    /// Given an inode, returns the filesystem entry associated with it
    fn get_entry(&self, inode: u64) -> Option<&RealizedFsEntry> {
        self.inodes.get(&inode)
    }

    /// Takes the contents of the command buffer and sends it to the shell.
    /// The command buffer is cleared, the exec file is reset
    fn do_exec(&mut self) {
        println!("Execute: {:?}", self.decoded_command_buffer);
        if self.decoded_command_buffer.is_empty() {
            self.log("Ignoring execution because buffer is empty");
            return;
        }

        self.log("Executing");

        let command = std::mem::replace(&mut self.decoded_command_buffer, Vec::new());
        self.command_channel.send(Command::Execute(command)).expect("Failed to send command");

        self.exec_file_inode = self.make_entry(FsEntry::ExecFile()).inode;
    }

    /// Waits for the shell to finish executing one command.
    /// This blocks the entire filesystem, which is clearly not ideal,
    /// but it works for now.
    /// This can cause deadlock if the command being executed
    /// and waited for tries to access the filesystem.
    /// TODO: fix (not going to be easy though, so don't bother)
    fn wait_one(&mut self) {
        self.log("Waiting");

        self.wait_file_inode = self.make_entry(FsEntry::WaitFile()).inode;

        let result = self.results_channel.recv().expect("Failed to receive result");
        self.log("Received result");

        self.diagnostics_file_inode = self.make_entry(FsEntry::ResultFile(Vec::new())).inode;
        self.stdout_file_inode = self.make_entry(FsEntry::ResultFile(Vec::new())).inode;
        self.stderr_file_inode = self.make_entry(FsEntry::ResultFile(Vec::new())).inode;

        let FinishedCommand::Execution(result) = result else {
            panic!("Received non-execution result");
        };

        let diagnostics_json = result.summarize_into_json().to_string().into_bytes();
        self.diagnostics_file().write_result(diagnostics_json);

        if let ExecutionResult::Ran { stdout, stderr, .. } = result.result {
            self.stdout_file().write_result(stdout);
            self.stderr_file().write_result(stderr);
        }
    }

    /// Terminates all running commands,
    /// clears the command buffer, and resets every file
    fn terminate_all(&mut self) {
        self.command_channel.send(Command::TerminateAll).expect("Failed to send command");
        self.log("Terminating");

        self.exec_file_inode = self.make_entry(FsEntry::ExecFile()).inode;
        self.wait_file_inode = self.make_entry(FsEntry::WaitFile()).inode;
        self.reset_file_inode = self.make_entry(FsEntry::ResetFile()).inode;
        self.diagnostics_file_inode = self.make_entry(FsEntry::ResultFile(Vec::new())).inode;
        self.stdout_file_inode = self.make_entry(FsEntry::ResultFile(Vec::new())).inode;
        self.stderr_file_inode = self.make_entry(FsEntry::ResultFile(Vec::new())).inode;

        self.decoded_command_buffer.clear();

        loop {
            match self.results_channel.recv().expect("Failed to receive result") {
                FinishedCommand::Termination => break,
                _ => (),
            }
        }
    }

    /// Appends the given bytes (hex-encoded) to the command buffer
    fn do_append(&mut self, encoded_bytes: Vec<u8>) {
        self.decoded_command_buffer.append(&mut hex_decode(encoded_bytes));
        self.log("Appended");
    }
}

fn is_allowed_char(c: u8) -> bool {
    match c {
        b'a'..=b'f' | b'0'..=b'9' => true,
        _ => false,
    }
}

/// Takes a subrange of a given slice and runs the given function on it if it's not empty,
/// returning the subrange
fn and_if_not_empty<T>(slice: &[T], range: Range<usize>, f: impl FnOnce(&[T])) -> &[T] {
    let clipped_range = range.start..range.end.min(slice.len());
    let clipped = &slice[clipped_range.clone()];
    if !clipped_range.is_empty() {
        f(clipped);
    }
    clipped
}

impl Filesystem for ShellEscapeFs {
    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        assert_eq!(parent, 1, "Parent inode must be root, should be impossible");
        eprintln!("Lookup: {:?}", name);

        let name = name.as_bytes();

        let name = if let Some((start_of_name, _)) = name.iter().enumerate().rfind(|(_, &c)| c == b'_') {
            &name[start_of_name + 1..]
        } else {
            name
        };

        if name == b"." {
            return reply.entry(&TTL, &self.root_attrs(), 0);
        }

        let name = if let Some((end_of_name, _)) = name.iter().enumerate().rfind(|(_, &c)| c == b'.') {
            &name[..end_of_name]
        } else {
            name
        };

        let reply_entry = |entry: Option<&RealizedFsEntry>| {
            match entry {
                Some(entry) => reply.entry(&TTL, &entry.get_attrs(), 0),
                None => reply.error(libc::ENOENT),
            }
        };

        match name {
            b"exec" => reply_entry(Some(&self.exec_file())),
            b"wait" => reply_entry(Some(&self.wait_file())),
            b"reset" => reply_entry(Some(&self.reset_file())),
            b"diagnostics" => reply_entry(Some(&self.diagnostics_file())),
            b"stdout" => reply_entry(Some(&self.stdout_file())),
            b"stderr" => reply_entry(Some(&self.stderr_file())),
            b"log" => reply_entry(Some(&self.log_file())),

            x if x.iter().all(|&c| is_allowed_char(c)) => {
                let fs_entry = FsEntry::AppendDataFile(x.into());
                reply_entry(Some(&self.make_entry(fs_entry)))
            }

            _ => reply_entry(None),
        }
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyAttr) {
        eprintln!("Getattr: {}", ino);

        if ino == 1 {
            reply.attr(&TTL, &self.root_attrs());
        } else if let Some(entry) = self.get_entry(ino) {
            reply.attr(&TTL, &entry.get_attrs());
        } else {
            reply.error(libc::ENOENT);
        }
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        eprintln!("Read: {} {} {}", ino, offset, size);

        let Some(RealizedFsEntry { entry, .. }) = self.get_entry(ino).cloned() else {
            reply.error(libc::ENOENT);
            return;
        };

        let slice = offset as usize..offset as usize + size as usize;

        match entry {
            FsEntry::ExecFile() => {
                reply.data(and_if_not_empty(
                    SUCCESS_MESSAGE,
                    slice,
                    |_| self.do_exec(),
                ));
            }
            FsEntry::WaitFile() => {
                reply.data(and_if_not_empty(
                    SUCCESS_MESSAGE,
                    slice,
                    |_| self.wait_one(),
                ));
            }
            FsEntry::ResetFile() => {
                reply.data(and_if_not_empty(
                    SUCCESS_MESSAGE,
                    slice,
                    |_| self.terminate_all(),
                ));
            }

            FsEntry::AppendDataFile(encoded_bytes) => {
                reply.data(and_if_not_empty(
                    SUCCESS_MESSAGE,
                    slice,
                    |_| self.do_append(encoded_bytes),
                ));
            }
            FsEntry::ResultFile(data) => reply.data(and_if_not_empty(&data, slice, |_| ())),
        }
    }

    fn readdir(&mut self, _req: &Request<'_>, ino: u64, fh: u64, offset: i64, mut reply: ReplyDirectory) {
        assert_eq!(ino, 1, "Inode must be root, should be impossible");
        assert_eq!(fh, 0, "File handle must be 0, should be impossible");
        eprintln!("Readdir: {}", offset);

        let entries = [
            (1, FileType::Directory, "."),
            (1, FileType::Directory, ".."),
            (self.exec_file_inode, FileType::RegularFile, "exec"),
            (self.wait_file_inode, FileType::RegularFile, "wait"),
            (self.reset_file_inode, FileType::RegularFile, "reset"),
            (self.diagnostics_file_inode, FileType::RegularFile, "diagnostics"),
            (self.stdout_file_inode, FileType::RegularFile, "stdout"),
            (self.stderr_file_inode, FileType::RegularFile, "stderr"),
            (self.log_file_inode, FileType::RegularFile, "log"),
        ];

        for (dir_offset, (inode, kind, name)) in entries.iter().skip(offset as usize).enumerate() {
            let reported_offset = dir_offset as i64 + 1;

            let full = reply.add(*inode, reported_offset, *kind, name);
            if full {
                break;
            }
        }

        reply.ok();
    }
}
