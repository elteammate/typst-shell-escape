use std::ffi::OsString;
use std::io::Read;
use std::os::unix::ffi::OsStringExt;
use std::sync::mpsc;
use std::thread;
use serde_json::json;
use wait_timeout::ChildExt;

pub enum Command {
    Execute(Vec<u8>),
    TerminateAll,
}

pub enum ExecutionResult {
    Ran {
        error_code: i32,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    FailedToSpawn(std::io::Error),
    FailedToWait(std::io::Error),
}

pub struct FinishedExecution {
    command: Vec<u8>,
    pub(crate) result: ExecutionResult,
}

pub enum FinishedCommand {
    Execution(FinishedExecution),
    Termination,
}

impl FinishedExecution {
    pub fn summarize_into_json(&self) -> serde_json::Value {
        let result = match &self.result {
            ExecutionResult::Ran { error_code, .. } => json!({
                "ran": true,
                "error_code": error_code,
            }),
            ExecutionResult::FailedToSpawn(e) => json!({
                "ran": false,
                "error": "Failed to spawn",
                "message": e.to_string(),
            }),
            ExecutionResult::FailedToWait(e) => json!({
                "ran": false,
                "error": "Failed to wait",
                "message": e.to_string(),
            }),
        };

        json!({
            "command": String::from_utf8_lossy(&self.command).to_string(),
            "result": result,
        })
    }
}

pub struct Terminate;

/// Starts the main loop of the shell.
pub fn run(result_sender: mpsc::Sender<FinishedCommand>, command_receiver: mpsc::Receiver<Command>) {
    let mut workers = vec![];
    let mut termination_senders = vec![];

    loop {
        let command = command_receiver.recv().expect("Failed to receive command");

        match command {
            Command::Execute(command) => {
                let (termination_sender, termination_receiver) = mpsc::channel::<Terminate>();
                let result_sender = result_sender.clone();

                workers.push(thread::spawn(move || {
                    let result = run_one(command, termination_receiver);
                    result_sender.send(result).expect("Failed to send result");
                }));

                termination_senders.push(termination_sender);
            }
            Command::TerminateAll => {
                while let Some(termination_sender) = termination_senders.pop() {
                    // If this fails, it means that the command is already executed and
                    // there is no need to terminate it.
                    let _ = termination_sender.send(Terminate);
                }

                while let Some(worker) = workers.pop() {
                    worker.join().expect("Failed to join worker");
                }

                result_sender.send(FinishedCommand::Termination)
                    .expect("Failed to send termination");
            }
        }
    }
}

/// Runs a single command. Should be ran in a separate thread.
pub fn run_one(command: Vec<u8>, termination_receiver: mpsc::Receiver<Terminate>) -> FinishedCommand {
    let mut command = command.to_vec();
    command.push(b'\n');

    let mut child = match std::process::Command::new("sh")
        .arg("-c")
        .arg(OsString::from_vec(command.clone()))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn() {
        Ok(child) => child,
        Err(e) => return FinishedCommand::Execution(FinishedExecution {
            command,
            result: ExecutionResult::FailedToSpawn(e)
        }),
    };

    let result = loop {
        match child.wait_timeout(std::time::Duration::from_secs(1)) {
            Ok(Some(error_code)) => {
                let mut stdout = child.stdout.take().unwrap();
                let mut stderr = child.stderr.take().unwrap();

                let mut stdout_buffer = Vec::new();
                let mut stderr_buffer = Vec::new();

                stdout.read_to_end(&mut stdout_buffer).unwrap();
                stderr.read_to_end(&mut stderr_buffer).unwrap();

                break ExecutionResult::Ran {
                    error_code: error_code.code().expect("Failed to get error code"),
                    stdout: stdout_buffer,
                    stderr: stderr_buffer,
                };
            }
            Ok(None) => {
                if let Ok(_) = termination_receiver.try_recv() {
                    child.kill().unwrap();
                    child.wait().unwrap();
                }
            }
            Err(e) => break ExecutionResult::FailedToWait(e),
        }
    };

    FinishedCommand::Execution(FinishedExecution {
        command,
        result,
    })
}
