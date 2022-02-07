use rand::Rng;
use std::collections::HashSet;
use std::io::{self, BufRead};

fn get_input_as_u8() -> u8 {
    let stdin = io::stdin();
    return stdin
        .lock()
        .lines()
        .next()
        .unwrap()
        .unwrap()
        .parse::<u8>()
        .unwrap();
}

fn update_positions_and_moves(
    possible_moves: &mut HashSet<u8>,
    positions: &mut HashSet<u8>,
    position: u8,
) {
    possible_moves.remove(&position);
    positions.insert(position);
}

fn contains_winning_combination(positions: &HashSet<u8>) -> bool {
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
        if positions.is_superset(combination) {
            return true;
        }
    }
    return false;
}

fn main() {
    let mut possible_moves: HashSet<u8> = HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8]);
    let mut my_positions: HashSet<u8> = HashSet::new();
    let mut opponent_positions: HashSet<u8> = HashSet::new();

    let player_index = get_input_as_u8();
    if player_index == 1 {
        update_positions_and_moves(
            &mut possible_moves,
            &mut opponent_positions,
            get_input_as_u8(),
        );
    }
    let mut rng = rand::thread_rng();
    while possible_moves.len() != 0 {
        let mut my_move: u8 = rng.gen_range(0..9);
        while !possible_moves.contains(&my_move) {
            my_move = rng.gen_range(0..9);
        }
        println!("{}", my_move);
        update_positions_and_moves(&mut possible_moves, &mut my_positions, my_move);
        if contains_winning_combination(&my_positions) {
            // println!("I win!!");
            break;
        }
        if possible_moves.len() == 0 {
            break;
        }
        let opponent_move: u8 = get_input_as_u8();
        update_positions_and_moves(&mut possible_moves, &mut opponent_positions, opponent_move);
        if contains_winning_combination(&opponent_positions) {
            // println!("I lost :(");
            break;
        }
    }
    // println!("Tie :o");
}
