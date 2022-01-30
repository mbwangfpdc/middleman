use anyhow::Result;
use env_logger::{Builder, Target};
use futures::future::{join_all, FutureExt};
use log::{debug, error, info, trace, LevelFilter};
use std::env;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc::{channel, Receiver, Sender};

// Spawn all processes specified by exe_paths and return a list of them as running processes
fn processes_from_paths(exe_paths: &[String]) -> Vec<Child> {
    exe_paths
        .iter()
        .map(|path| {
            Command::new(path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect::<Vec<_>>()
}

// Given a delim and a tag, tag every line from line_reader and send it through sender
async fn tag_and_echo_stdout_to_channel(
    mut line_reader: Lines<BufReader<ChildStdout>>,
    sender: Sender<String>,
    tag: usize,
    delim: char,
) -> Result<()> {
    info!("{tag}: start tagging and echoing stdout");
    while let Some(line) = line_reader.next_line().await? {
        trace!("{tag}: tagging and forwarding '{line}'");
        let tagged_message = format!("{}{}{}", tag, delim, line);
        sender.send(tagged_message).await?;
        trace!("{line} sent to channel");
    }
    info!("{tag}: done tagging and echoing stdout");
    Ok(())
}

// Uses line_reader to read stdout line-by-line, then parse out tag from message using delim
// Uses tag to feed the parsed message to the correct channel
async fn echo_tagged_stdout_to_channel(
    mut line_reader: Lines<BufReader<ChildStdout>>,
    senders: Vec<Sender<String>>,
    delim: char,
) -> Result<()> {
    info!("Tagged stdout echoes starting");
    while let Some(line) = line_reader.next_line().await? {
        let mut spliterator = line.split(delim);
        let prefix = spliterator.next().unwrap();
        let recipient = prefix.parse::<usize>().unwrap();
        // TODO(mbwang): unnecessary allocation here with to_string but w/e
        // Process list is 1-indexed since the manager is process 0
        trace!("Read tagged {line}, untagging and forwarding to {recipient}");
        senders[recipient - 1]
            .send(spliterator.next().unwrap().to_string())
            .await?;
        trace!("{line} sent to {recipient}");
    }
    info!("Tagged stdout echoes done");
    Ok(())
}

// Dump all strings from the channel into the given stdin
async fn echo_channel_to_stdin(
    mut writer: BufWriter<ChildStdin>,
    mut receiver: Receiver<String>,
) -> Result<()> {
    info!("Start echoing to stdin");
    while let Some(mut message) = receiver.recv().await {
        trace!("Received {message}, echoing line to stdin");
        message.push('\n');
        writer.write_all(message.as_bytes()).await?;
        writer.flush().await?;
        trace!("{message} sent to stdin");
    }
    receiver.close();
    info!("Done echoing to stdin");
    // TODO(mbwang): The manager is hanging here for some reason
    Ok(())
}

// Do the thing
// TODO(mbwang): is line reader ok? what if someone tries to crash metamanager with huge invalid messages?
async fn run(mut processes: Vec<Child>) -> Result<()> {
    // TODO(mbwang): parametrize this delim?
    let delim = ':';
    let mut tasks = Vec::new();
    {
        // TODO(mbwang): arbitrary channel size, 32 is probably big enough
        let (p2m_sender, p2m_receiver) = channel::<String>(32);
        let mut piterator = processes.iter_mut();
        let manager = piterator.next().unwrap();
        tasks.push(
            echo_channel_to_stdin(BufWriter::new(manager.stdin.take().unwrap()), p2m_receiver)
                .boxed(),
        );
        let mut m2p_senders = Vec::new();
        for (idx, process) in piterator.enumerate() {
            trace!("Process {} tasks has pid {}", idx, process.id().unwrap());
            let (m2p_sender, m2p_receiver) = channel::<String>(32);
            m2p_senders.push(m2p_sender);
            tasks.push(
                echo_channel_to_stdin(BufWriter::new(process.stdin.take().unwrap()), m2p_receiver)
                    .boxed(),
            );
            tasks.push(
                tag_and_echo_stdout_to_channel(
                    BufReader::new(process.stdout.take().unwrap()).lines(),
                    p2m_sender.clone(),
                    idx + 1, // We start iterating at 1 after the manager
                    delim,
                )
                .boxed(),
            );
        }
        tasks.push(
            echo_tagged_stdout_to_channel(
                BufReader::new(manager.stdout.take().unwrap()).lines(),
                m2p_senders,
                delim,
            )
            .boxed(),
        );
    }
    // Note: if the block above is in the same scope as this join_all call,
    // the original p2m_sender is still alive here and join_all will never finish
    // since p2m_receiver waits for the sender to be dropped before closing
    join_all(tasks.into_iter()).await;
    info!("All tasks resolved");
    Ok(())
}

fn usage() {
    error!(
        "Usage: {} path_to_executable_manager path_to_executable_player_1 path_to_executable_player_2...",
        env::args()
            .next()
            .expect("Arg 0 must be the executable name")
    );
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // TODO(mbwang): configure logging outside of the binary
    Builder::new()
        .target(Target::Stdout)
        .filter_level(LevelFilter::Info)
        .init();
    let args = env::args().collect::<Vec<_>>();
    debug!("{} called with arguments: [{}]", args[0], args.join(", "),);
    // TODO(mbwang): debugging, allowing just one input program
    if args.len() < 2
    /*3*/
    {
        usage();
        // TODO: for some reason I can't return an error here
        // without messing up the inferred return value of this function,
        // maybe a problem with anywho or tokio?
        return Ok(());
    }

    run(processes_from_paths(&args[1..])).await?;
    return Ok(());
}
