use futures::future::select_all;
use std::env;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWrite, BufReader, Error, Split};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

// Spawn all processes specified by exe_paths and return a list of them as running processes
fn processes_from_paths(exe_paths: Vec<String>) -> Vec<Child> {
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

// We have to echo some input data here in the return value
// because select_all might scramble the polled futures, and
// we'd have no way of knowing which future resolved (after a few scrambles)
async fn next_segment_tagged(
    split_stream: &mut Split<BufReader<ChildStdout>>,
    tag: usize,
) -> Result<(Option<Vec<u8>>, &mut Split<BufReader<ChildStdout>>, usize), Error> {
    Ok((split_stream.next_segment().await?, split_stream, tag))
}

// Monitor and echo the stdout of all given processes
// TODO(mbwang): stop echoing (or maybe keep echoing for logging?)
//               and send to stdin of destination processes
async fn echo_stdout(children: Vec<Child>) -> Result<(), Error> {
    let mut stdouts = Vec::new();
    // let mut stdins = Vec::new();
    for mut child in children {
        stdouts.push(BufReader::new(child.stdout.take().unwrap()).split(b'\n'));
        // stdins.push(child.stdin.take().unwrap());
    }
    // stdins[0].write("abc");

    let mut reading_futures = Vec::new();
    for (idx, stdout) in stdouts.iter_mut().enumerate() {
        reading_futures.push(Box::pin(next_segment_tagged(stdout, idx)));
    }

    while !reading_futures.is_empty() {
        // Rust is so feature rich and complex. It is enticing, intimidating, and exhilirating. Daddy rust.
        let (result, _, mut waiting_futures) = select_all(reading_futures).await;
        let (maybe_data, stdout, user_id) = result?;
        if let Some(data) = maybe_data {
            // TODO(mbwang): another place where we assume no panic because of good behavior by clients
            println!(
                "Message from {}: {}",
                user_id,
                String::from_utf8(data).unwrap()
            );
            // TODO:(mbwang): fix your brain this is so confusing rn
            waiting_futures.push(Box::pin(next_segment_tagged(stdout, user_id)));
        } else {
            println!("{} sent no data, not listening to them anymore", user_id);
        }
        reading_futures = waiting_futures;
    }
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), ()> {
    let args = env::args().collect::<Vec<_>>();
    println!("{} called with arguments: [{}]", args[0], args.join(", "),);
    // TODO(mbwang): debugging, allowing just one input program
    if args.len() < 2
    /*3*/
    {
        usage();
        return Err(());
    }

    let pfp = processes_from_paths(args[1..].to_vec());
    if let Err(e) = echo_stdout(pfp).await {
        println!("Error emitted to main loop: {}", e);
        return Err(());
    }

    Ok(())
}
