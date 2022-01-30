use anyhow::Result;
use futures::future::{join_all, select_all, FutureExt};
use log::{debug, error, info, trace, warn};
use std::env;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::{sleep, Duration};
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
            println!("Message from {user_id}: {data}");
            // TODO:(mbwang): fix your brain this is so confusing rn
            waiting_futures.push(Box::pin(next_segment_tagged(line_reader, user_id)));
        } else {
            println!("{user_id} sent no data, not listening to them anymore");
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
        println!("Writing 'nice' to first writer...");
        writers[0].write_all("nice\n".as_bytes()).await?;
        writers[0].flush().await?;
        sleep(Duration::from_millis(1500)).await;
    }
    Ok(())
}
