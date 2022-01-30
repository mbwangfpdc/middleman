#!/usr/bin/env python3
import sys


def main():
    empty_positions, my_positions, their_positions = set(range(9)), set(), set()
    winning_positions = [
        {0, 1, 2},
        {3, 4, 5},
        {6, 7, 8},
        {0, 3, 6},
        {1, 4, 7},
        {2, 5, 8},
        {0, 4, 8},
        {2, 4, 6},
    ]

    def has_winning_position(positions: set):
        return any([pos.issubset(positions) for pos in winning_positions])

    def print_board():
        for pos in range(9):
            print(
                "X" if pos in my_positions else "O" if pos in their_positions else "-",
                end="\n" if pos % 3 == 2 else "",
                file=sys.stderr,  # So we don't confuse the manager
            )

    # Return a winning move or -1 if no winning move exists
    def move_that_wins() -> int:
        for move in empty_positions:
            my_future_positions = set(my_positions)
            my_future_positions.add(move)
            if has_winning_position(my_future_positions):
                return move
        return -1

    my_turn = bool(int(input()))
    while True:
        print_board()
        if has_winning_position(my_positions):
            print("I won!!", file=sys.stderr)
            sys.exit(0)
        elif has_winning_position(their_positions):
            print("I lost :(", file=sys.stderr)
            sys.exit(0)
        elif len(empty_positions) == 0:
            print("Tie :o", file=sys.stderr)
            sys.exit(0)
        if not my_turn:
            opponent_move = int(input())
            assert opponent_move in empty_positions
            empty_positions.remove(opponent_move)
            their_positions.add(opponent_move)
        else:
            move = move_that_wins()
            if move < 0:  # when no move that wins, get arbitrary move
                move = empty_positions.pop()
            my_positions.add(move)
            print(move, flush=True)
        my_turn = not my_turn


if __name__ == "__main__":
    main()
