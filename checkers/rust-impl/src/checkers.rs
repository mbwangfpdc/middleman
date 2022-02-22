use itertools::chain;
use std::iter::once;

// Structure inspired by/shamelessly copied from http://www.3dkingdoms.com/checkers/bitboards.htm

// There are 32 legal tiles in a checkers position
// A given bit on the first u32 is set if something is present there
// As bits go from most to least significant, you go left-right top-down on the board
// So, left shifts go left and up, right shifts go right and down
/*
  31  30  29  28
27  26  25  24
  23  22  21  20
19  18  17  16
  15  14  13  12
11  10  09  08
  07  06  05  04
03  02  01  00
*/
// For example, the following board is represented as 0b11101100000100000000000100010001
/*
-X-X-X--
X-X-----
-------X
--------
--------
------X-
-------X
------X-
*/
pub type BitBoard = u32;

pub struct Position {
  pub black: BitBoard,
  pub white: BitBoard,
  pub kings: BitBoard, // Although all men are kings :')
}

pub enum GameStatus {
  Illegal, // Game position is impossible, something went wrong
  Running,
  Draw,
  BlackWins,
  WhiteWins,
}

// TODO(mbwang): implement draw detection
pub fn get_game_status(position: &Position) -> GameStatus {
  if position.white | position.black | position.kings != position.white | position.black
    || position.white & position.black != 0
  {
    // white and black cannot share a tile, kings must be a subset of the other tiles
    return GameStatus::Illegal;
  }
  match (position.black == 0, position.white == 0) {
    (false, false) => GameStatus::Running,
    (false, true) => GameStatus::BlackWins,
    (true, false) => GameStatus::WhiteWins,
    (true, true) => GameStatus::Illegal,
  }
}

// Note this is relatively unperformant, mainly for debugging and visualization
impl std::fmt::Display for Position {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let status_msg = match get_game_status(self) {
      GameStatus::Illegal => "Something's wrong with the game, do not trust this visualization...",
      GameStatus::Running => "Still playing...",
      GameStatus::Draw => "The game is a draw.",
      GameStatus::BlackWins => "Black won!",
      GameStatus::WhiteWins => "White won!",
    };
    let symbol_at_pos = |pos: u32| -> String {
      let mut prefix = String::from(match pos % 8 {
        0..=2 | 4..=6 => "  ",
        3 => "\n",
        7 => "\n  ",
        _ => unreachable!(),
      });
      prefix.push_str(
        match (
          self.black & (1 << pos) != 0, // piece is black
          self.white & (1 << pos) != 0, // piece is white
          self.kings & (1 << pos) != 0, // piece is a king
        ) {
          (false, false, _) => "..",
          (false, true, false) => ".W",
          (false, true, true) => "WW",
          (true, false, false) => ".B",
          (true, false, true) => "BB",
          (true, true, _) => "??",
        },
      );
      prefix
    };
    let board: String = chain!(
      once(String::from(status_msg)),
      (0..32).rev().map(symbol_at_pos),
    )
    .collect();
    write!(f, "{board}")
  }
}

// Any given spot (bit) on the board has 4 adjacenct spots (bits)
// Two spots are always shifted left/right 4,
// and the other two are either left 3/right 5 or left 5/right 3.
// Novel bits shifted in are 0s (TODO(mbwang): right?)
static MASK_L3: BitBoard =
  1 << 5 | 1 << 6 | 1 << 7 | 1 << 13 | 1 << 14 | 1 << 15 | 1 << 21 | 1 << 22 | 1 << 23;
static MASK_R5: BitBoard = 1 << 5
  | 1 << 6
  | 1 << 7
  | 1 << 13
  | 1 << 14
  | 1 << 15
  | 1 << 21
  | 1 << 22
  | 1 << 23
  | 1 << 29
  | 1 << 30
  | 1 << 31;
static MASK_R3: BitBoard =
  1 << 8 | 1 << 9 | 1 << 10 | 1 << 16 | 1 << 17 | 1 << 18 | 1 << 24 | 1 << 25 | 1 << 26;
static MASK_L5: BitBoard = 1 << 0
  | 1 << 1
  | 1 << 2
  | 1 << 8
  | 1 << 9
  | 1 << 10
  | 1 << 16
  | 1 << 17
  | 1 << 18
  | 1 << 24
  | 1 << 25
  | 1 << 26;

// Given a BitBoard of empty tiles, return tiles adjacent to those tiles and to the south
fn valid_southern_origins(empty: BitBoard) -> BitBoard {
  empty >> 4 | ((empty & MASK_R3) >> 3) | ((empty & MASK_R5) >> 5)
}
// Given a BitBoard of empty tiles, return tiles adjacent to those tiles and to the north
fn valid_northern_origins(empty: BitBoard) -> BitBoard {
  empty << 4 | ((empty & MASK_L3) << 3) | ((empty & MASK_L5) << 5)
}

// Black pieces are at the bottom of the board moving up
pub fn black_movers(position: Position) -> BitBoard {
  let empty = !(position.black | position.white);
  let black_kings = position.black & position.kings;
  let mut movers = valid_southern_origins(empty);
  if black_kings != 0 {
    movers |= valid_northern_origins(empty) & black_kings;
  }
  movers & position.black
}

// White pieces are at the bottom of the board moving up
pub fn white_movers(position: Position) -> BitBoard {
  let empty = !(position.white | position.black);
  let white_kings = position.white & position.kings;
  let mut movers = valid_northern_origins(empty);
  if white_kings != 0 {
    movers |= valid_southern_origins(empty) & white_kings;
  }
  movers & position.white
}
