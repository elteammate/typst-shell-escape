mod shell;
mod fs;
mod decode;

use std::path::Path;
use std::sync::mpsc;
use std::thread;
use fuser::*;
use fs::ShellEscapeFs;

fn main() {
    let (command_sender, command_receiver) = mpsc::channel::<shell::Command>();
    let (result_sender, result_receiver) = mpsc::channel::<shell::FinishedCommand>();

    let fs = ShellEscapeFs::new(command_sender, result_receiver);

    let mount_point = Path::new("/tmp/typst-shell-escape/shell-escape");

    if !mount_point.exists() {
        std::fs::create_dir_all(&mount_point).expect("Failed to create mount point");
    } else if !mount_point.is_dir() {
        panic!("Mount point is not a directory");
    }

    thread::spawn(move || {
        mount2(fs, &mount_point, &[
            MountOption::AutoUnmount,
            MountOption::RO,
            MountOption::AllowOther,
        ]).expect("Failed to mount filesystem");
    });

    shell::run(result_sender, command_receiver);
}
