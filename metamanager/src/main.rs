use anyhow::Result;
use futures::future::{join_all, select_all, FutureExt};
use std::env;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::{sleep, Duration};

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
    while let Some(line) = line_reader.next_line().await? {
        let tagged_message = format!("{}{}{}", tag, delim, line);
        sender.send(tagged_message).await?;
    }
    Ok(())
}

// Uses line_reader to read stdout line-by-line, then parse out tag from message using delim
// Uses tag to feed the parsed message to the correct channel
async fn echo_tagged_stdout_to_channel(
    mut line_reader: Lines<BufReader<ChildStdout>>,
    senders: Vec<Sender<String>>,
    delim: char,
) -> Result<()> {
    while let Some(line) = line_reader.next_line().await? {
        let mut spliterator = line.split(delim);
        let prefix = spliterator.next().unwrap();
        let recipient = prefix.parse::<usize>().unwrap();
        // TODO(mbwang): unnecessary allocation here with to_string but w/e
        // Process list is 1-indexed since the manager is process 0
        senders[recipient - 1]
            .send(spliterator.next().unwrap().to_string())
            .await?;
    }
    Ok(())
}

// Dump all strings from the channel into the given stdin
async fn echo_channel_to_stdin(
    mut writer: BufWriter<ChildStdin>,
    mut receiver: Receiver<String>,
) -> Result<()> {
    while let Some(message) = receiver.recv().await {
        writer.write_all(message.as_bytes()).await?;
    }
    Ok(())
}

// Do the thing
// TODO(mbwang): is line reader ok? what if someone tries to crash metamanager with huge invalid messages?
async fn run(mut processes: Vec<Child>) -> Result<()> {
    // TODO(mbwang): parametrize this delim?
    let delim = ':';
    // TODO(mbwang): arbitrary channel size, 32 is probably big enough
    let mut tasks = Vec::new();
    let (p2m_sender, p2m_receiver) = channel::<String>(32);
    let mut piterator = processes.iter_mut();
    let manager = piterator.next().unwrap();
    tasks.push(
        echo_channel_to_stdin(BufWriter::new(manager.stdin.take().unwrap()), p2m_receiver).boxed(),
    );
    let mut m2p_senders = Vec::new();
    for (idx, process) in piterator.enumerate() {
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
                idx + 1,
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
    join_all(tasks.into_iter()).await;
    Ok(())
}

fn usage() {
    println!(
        "Usage: {} path_to_executable_manager path_to_executable_contestant_1 path_to_executable_contestant_2...",
        env::args()
            .next()
            .expect("Arg 0 must be the executable name")
    );
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    println!("{} called with arguments: [{}]", args[0], args.join(", "),);
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

// CODE BELOW HERE UNUSED
// TODO(mbwang): archive this? or just delete it?

// We have to echo some input data here in the return value
// because select_all might scramble the polled futures, and
// we'd have no way of knowing which future resolved (after a few scrambles)
#[allow(dead_code)]
async fn next_segment_tagged(
    mut line_reader: Lines<BufReader<ChildStdout>>,
    tag: usize,
) -> Result<(Option<String>, Lines<BufReader<ChildStdout>>, usize)> {
    Ok((line_reader.next_line().await?, line_reader, tag))
}

// Monitor and echo the stdout of all given processes
// TODO(mbwang): stop echoing (or maybe keep echoing for logging?)
//               and send to stdin of destination processes
#[allow(dead_code)]
async fn echo_stdout(child_stdouts: Vec<ChildStdout>) -> Result<()> {
    let mut reading_futures = Vec::new();
    // Futures are maybe scrambled after select_all so tag each LineReader with its 'process id'
    for (idx, child_stdout) in child_stdouts.into_iter().enumerate() {
        reading_futures.push(Box::pin(next_segment_tagged(
            BufReader::new(child_stdout).lines(),
            idx,
        )));
    }

    while !reading_futures.is_empty() {
        // Rust is so feature rich and complex. It is enticing, intimidating, and exhilirating. Daddy rust.
        let (result, _, mut waiting_futures) = select_all(reading_futures).await;
        let (maybe_data, line_reader, user_id) = result?;
        if let Some(data) = maybe_data {
            // TODO(mbwang): another place where we assume no panic because of good behavior by clients
            println!("Message from {}: {}", user_id, data);
            // TODO:(mbwang): fix your brain this is so confusing rn
            waiting_futures.push(Box::pin(next_segment_tagged(line_reader, user_id)));
        } else {
            println!("{} sent no data, not listening to them anymore", user_id);
        }
        reading_futures = waiting_futures;
    }
    Ok(())
}

// TODO(mbwang): feed_stdin reads and routes messages from a channel that the
//               stdout reader sends to
#[allow(dead_code)]
async fn feed_stdin(child_stdins: Vec<ChildStdin>) -> Result<()> {
    let mut writers = Vec::new();
    // Futures are maybe scrambled after select_all so tag each LineReader with its 'process id'
    for child_stdin in child_stdins.into_iter() {
        writers.push(BufWriter::new(child_stdin));
    }
    for _ in 0..5 {
        println!("Writing {} to first writer...", "nice");
        writers[0].write_all("nice\n".as_bytes()).await?;
        writers[0].flush().await?;
        sleep(Duration::from_millis(1500)).await;
    }
    Ok(())
}
