use rand::Rng;
use std::collections::HashSet;
use std::fmt;
use std::result::Result::Ok;
use std::time::Duration;
use tokio::io;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::timeout;

struct Message {
    user_pid: u8,
    position: u8,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.user_pid, self.position)
    }
}

fn build_message(line: String) -> Option<Message> {
    let mut split = line.split(":");
    let user_pid = split.next()?.parse::<u8>().unwrap();
    let message = split.next()?.trim();
    match message.parse::<u8>() {
        Ok(position) => Some(Message { user_pid, position }),
        Err(_) => None,
    }
}

fn contains_winning_combination(moves: &HashSet<u8>) -> bool {
    let winning_combinations: [HashSet<u8>; 8] = [
        HashSet::from([0, 1, 2]),
        HashSet::from([3, 4, 5]),
        HashSet::from([6, 7, 8]),
        HashSet::from([0, 3, 6]),
        HashSet::from([1, 4, 7]),
        HashSet::from([2, 5, 8]),
        HashSet::from([0, 4, 8]),
        HashSet::from([6, 4, 2]),
    ];
    for combination in &winning_combinations {
        if combination.intersection(moves).count() == 3 {
            return true;
        }
    }
    return false;
}

// TODO: this is commented out because maybe players don't need
//       to know about how to deal with the game end, we can just
//       shut down their programs gracefully and save the result
//       in the manager's log (TODO).
//
//       also it was throwing a bunch of broken pipe errors lmao
// Send result to both
fn print_decisive_result(loser_pid: u8) {
    // println!("{}:{} {}", loser_pid, get_next_pid(loser_pid), loser_pid);
    // println!(
    //     "{}:{} {}",
    //     get_next_pid(loser_pid),
    //     get_next_pid(loser_pid),
    //     loser_pid
    // );
}

fn get_next_pid(pid: u8) -> u8 {
    return (pid + 1) % 2;
}

#[tokio::main]
async fn main() {
    let mut lines = BufReader::new(io::stdin()).lines();
    let mut current_pid: u8 = rand::thread_rng().gen_range(0..2);
    let mut possible_moves: HashSet<u8> = HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8]);
    let mut player_moves: [HashSet<u8>; 2] = [HashSet::new(), HashSet::new()];

    // Let the players know who's moving first.
    println!("{}:0", current_pid);
    current_pid = get_next_pid(current_pid);
    println!("{}:1", current_pid);
    current_pid = get_next_pid(current_pid);

    while !possible_moves.is_empty() {
        if let Ok(Ok(Some(line))) = timeout(Duration::from_secs(20), lines.next_line()).await {
            if let Some(message) = build_message(line) {
                // Player played out of turn which is invalid, user auto-loses.
                if message.user_pid != current_pid {
                    print_decisive_result(message.user_pid);
                    return;
                }
                // Player played an impossible move, user auto-loses.
                if !possible_moves.remove(&message.position) {
                    print_decisive_result(current_pid);
                    return;
                }
                let _ = &player_moves[current_pid as usize].insert(message.position);

                // Player current_pid played a winning move.
                if contains_winning_combination(&player_moves[current_pid as usize]) {
                    print_decisive_result(get_next_pid(current_pid));
                }

                // Notify the next player of the move.
                current_pid = get_next_pid(current_pid);
                println!(
                    "{}",
                    Message {
                        user_pid: current_pid,
                        position: message.position
                    }
                );
            } else {
                // Player current_pid provided invalid input.
                print_decisive_result(current_pid);
                return;
            }
        } else {
            // Player current_pid failed to produce output in time, user auto-loses.
            print_decisive_result(current_pid);
            return;
        }
    }
}
