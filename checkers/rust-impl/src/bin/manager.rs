use checkers;
fn main() {
    let position = checkers::Position {
        white: 0b11101100000100000000000000000000,
        black: 0b00000000000000000000000100010001,
        kings: 0b00000000000100000000000100010000,
        // black: 0,
        // kings: 0b00000000000100000000000000000000,
    };
    println!("{position}");
    let position = checkers::Position {
        white: 0b11101100000100000000000000000000,
        black: 0b00000000000000000000000000000000,
        kings: 0b00000000000100000000000000000000,
    };
    println!("{position}");
}
