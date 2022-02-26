use anyhow::{bail, Result};
use env_logger::{Builder, Target};
use futures::future::{join_all, select_all, FutureExt};
use log::{debug, error, info, trace, LevelFilter};
use std::env;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc::{channel, Receiver, Sender};

type ChildStdoutReader = Lines<BufReader<ChildStdout>>;
type ChildStdinWriter = BufWriter<ChildStdin>;

fn make_child_stdout_reader(child: &mut Child) -> ChildStdoutReader {
    BufReader::new(child.stdout.take().unwrap()).lines()
}
fn make_child_stdin_writer(child: &mut Child) -> ChildStdinWriter {
    BufWriter::new(child.stdin.take().unwrap())
}

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
    mut line_reader: ChildStdoutReader,
    sender: Sender<String>,
    tag: usize,
    delim: char,
) -> Result<()> {
    info!("{tag}: start tagging and echoing stdout");
    while let Some(line) = line_reader.next_line().await? {
        trace!("{tag}: tagging and forwarding '{line}'");
        sender.send(format!("{}{}{}\n", tag, delim, line)).await?;
        trace!("{line} sent to channel");
    }
    info!("{tag}: done tagging and echoing stdout");
    Ok(())
}

// Uses line_reader to read stdout line-by-line, then parse out tag from message using delim
// Uses tag to feed the parsed message to the correct channel
async fn echo_tagged_stdout_to_channel(
    mut line_reader: ChildStdoutReader,
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
        let mut message = spliterator.next().unwrap().to_string();
        message.push('\n');
        senders[recipient].send(message).await?;
        trace!("{line} sent to {recipient}");
    }
    info!("Tagged stdout echoes done");
    Ok(())
}

// Dump all strings from the channel into the given stdin
async fn echo_channel_to_stdin(
    mut writer: ChildStdinWriter,
    mut receiver: Receiver<String>,
) -> Result<()> {
    info!("Start echoing to stdin");
    while let Some(message) = receiver.recv().await {
        trace!("Received {message}, echoing line to stdin");
        writer.write_all(message.as_bytes()).await?;
        writer.flush().await?;
        trace!("{message} sent to stdin");
    }
    receiver.close();
    info!("Done echoing to stdin");
    Ok(())
}

// Uses line_reader to read stdout line-by-line, then parse out tag from message using delim
// Uses tag to feed the parsed message to the correct channel
async fn route_and_echo_tagged_messages(
    mut line_reader: ChildStdoutReader,
    mut stdins: Vec<ChildStdinWriter>,
    delim: char,
) -> Result<()> {
    info!("Start forwarding manager messages to players...");
    while let Some(mut line) = line_reader.next_line().await? {
        trace!("Forwarding '{line}' to a player");
        line.push('\n');
        let mut spliterator = line.split(delim);
        let prefix = spliterator.next().unwrap();
        trace!("Trying to parse {prefix} as a usize");
        let recipient = prefix.parse::<usize>().unwrap();
        // TODO(mbwang): 0 index children or 1 index them? 1 indexing allows us
        //               to have the manager as 0 (or maybe the visualizer/log?)
        stdins[recipient]
            .write_all(spliterator.next().unwrap().as_bytes())
            .await?;
        stdins[recipient].flush().await?;
        trace!("Sent to {recipient}");
    }
    info!("Done forwarding manager messages to players!");
    Ok(())
}

async fn wait_for_next_segment_tagged(
    mut line_reader: ChildStdoutReader,
    tag: usize,
) -> Result<(Option<String>, ChildStdoutReader, usize)> {
    Ok((line_reader.next_line().await?, line_reader, tag))
}

// Given a delim, echo all stdout from line_readers to stdin, after tagging messages with their sender
async fn tag_and_echo_messages(
    line_readers: Vec<ChildStdoutReader>,
    mut stdin: ChildStdinWriter,
    delim: char,
) -> Result<()> {
    info!("Start tagging and forwarding player messages to manager");
    let mut read_coroutines = line_readers
        .into_iter()
        .enumerate()
        .map(|(idx, reader)| Box::pin(wait_for_next_segment_tagged(reader, idx)))
        .collect::<Vec<_>>();
    while !read_coroutines.is_empty() {
        let (result, _, mut waiting_futures) = select_all(read_coroutines).await;
        let (maybe_data, reader, user_id) = result?;
        if let Some(data) = maybe_data {
            trace!("Message from {user_id}: {data}");
            stdin
                .write_all(format!("{user_id}{delim}{data}\n").as_bytes())
                .await?;
            stdin.flush().await?;
            trace!("Message sent to manager");
            waiting_futures.push(Box::pin(wait_for_next_segment_tagged(reader, user_id)));
        } else {
            info!("{user_id} sent no data, closing their connection");
        }
        read_coroutines = waiting_futures;
    }
    info!("Done tagging and forwarding player messages to manager");
    Ok(())
}

// Do the thing
// TODO(mbwang): is line reader ok? what if someone tries to crash metamanager with huge invalid messages?
async fn run(mut processes: Vec<Child>) -> Result<()> {
    // TODO(mbwang): parametrize this delim?
    const DELIM: char = ':';
    // TODO(mbwang): arbitrary channel size, 32 is probably big enough
    const CHAN_SIZE: usize = 32;
    debug!("Running with {} processes", processes.len());
    let mut tasks = Vec::new();
    // TODO(mbwang): test channel-less implementation
    let use_channels = false;
    {
        let (p2m_sender, p2m_receiver) = channel::<String>(CHAN_SIZE);
        let mut piterator = processes.iter_mut();
        let manager = piterator.next().unwrap();
        let manager_stdin = make_child_stdin_writer(manager);
        let manager_stdout = make_child_stdout_reader(manager);
        if use_channels {
            let mut m2p_senders = Vec::new();
            for (idx, process) in piterator.enumerate() {
                trace!("Process {} tasks has pid {}", idx, process.id().unwrap());
                let (m2p_sender, m2p_receiver) = channel::<String>(CHAN_SIZE);
                m2p_senders.push(m2p_sender);
                tasks.push(
                    echo_channel_to_stdin(make_child_stdin_writer(process), m2p_receiver).boxed(),
                );
                tasks.push(
                    tag_and_echo_stdout_to_channel(
                        make_child_stdout_reader(process),
                        p2m_sender.clone(),
                        // TODO(mbwang): see above: 0 or 1-index?
                        idx, // 0 indexed
                        DELIM,
                    )
                    .boxed(),
                );
            }
            tasks.push(echo_channel_to_stdin(manager_stdin, p2m_receiver).boxed());
            tasks.push(echo_tagged_stdout_to_channel(manager_stdout, m2p_senders, DELIM).boxed());
        } else {
            let mut child_stdins: Vec<ChildStdinWriter> = Vec::new();
            let mut child_stdouts: Vec<ChildStdoutReader> = Vec::new();
            for process in piterator {
                child_stdins.push(make_child_stdin_writer(process));
                child_stdouts.push(make_child_stdout_reader(process));
            }
            tasks.push(route_and_echo_tagged_messages(manager_stdout, child_stdins, DELIM).boxed());
            tasks.push(tag_and_echo_messages(child_stdouts, manager_stdin, DELIM).boxed());
        }
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
        .filter_level(LevelFilter::Trace)
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
