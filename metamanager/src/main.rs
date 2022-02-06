use anyhow::{bail, Result};
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
        trace!("Trying to parse {prefix} as a usize");
        let recipient = prefix.parse::<usize>().unwrap();
        // TODO(mbwang): unnecessary allocation here with to_string but w/e
        trace!("Read tagged {line}, untagging and forwarding to {recipient}");
        // TODO(mbwang): 0 index children or 1 index them? 1 indexing allows us
        //               to have the manager as 0 (or maybe the visualizer/log?)
        // senders[recipient - 1]
        senders[recipient]
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
    Ok(())
}

// Do the thing
// TODO(mbwang): is line reader ok? what if someone tries to crash metamanager with huge invalid messages?
async fn run(mut processes: Vec<Child>) -> Result<()> {
    // TODO(mbwang): parametrize this delim?
    const DELIM: char = ':';
    // TODO(mbwang): arbitrary channel size, 32 is probably big enough
    const CHAN_SIZE: usize = 32;
    let mut tasks = Vec::new();
    {
        let (p2m_sender, p2m_receiver) = channel::<String>(CHAN_SIZE);
        let mut piterator = processes.iter_mut();
        let manager = piterator.next().unwrap();
        tasks.push(
            echo_channel_to_stdin(BufWriter::new(manager.stdin.take().unwrap()), p2m_receiver)
                .boxed(),
        );
        let mut m2p_senders = Vec::new();
        for (idx, process) in piterator.enumerate() {
            trace!("Process {} tasks has pid {}", idx, process.id().unwrap());
            let (m2p_sender, m2p_receiver) = channel::<String>(CHAN_SIZE);
            m2p_senders.push(m2p_sender);
            tasks.push(
                echo_channel_to_stdin(BufWriter::new(process.stdin.take().unwrap()), m2p_receiver)
                    .boxed(),
            );
            tasks.push(
                tag_and_echo_stdout_to_channel(
                    BufReader::new(process.stdout.take().unwrap()).lines(),
                    p2m_sender.clone(),
                    // TODO(mbwang): see above: 0 or 1-index?
                    idx, // 0 indexed
                    DELIM,
                )
                .boxed(),
            );
        }
        tasks.push(
            echo_tagged_stdout_to_channel(
                BufReader::new(manager.stdout.take().unwrap()).lines(),
                m2p_senders,
                DELIM,
            )
            .boxed(),
        );
    }
    // Note: if the block above is in the same scope as this join_all call,
    // the original p2m_sender is still alive here and join_all will never finish
    // since p2m_receiver waits for the jango fett sender to be dropped before closing
    // https://en.wikipedia.org/wiki/Jango_Fett#Attack_of_the_Clones
    for result in join_all(tasks.into_iter()).await {
        result?
    }
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
    if args.len() < 3 {
        usage();
        bail!("The metamanager needs to be run with at least two other processes - a manager and a player");
    }

    run(processes_from_paths(&args[1..])).await?;
    return Ok(());
}
