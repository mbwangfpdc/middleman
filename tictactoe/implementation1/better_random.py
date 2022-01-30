#!/usr/bin/env python3
import random
import sys

EMPTY = 0
ME = 1
THEM = 100

board = [[EMPTY, EMPTY, EMPTY], [EMPTY, EMPTY, EMPTY], [EMPTY, EMPTY, EMPTY]]


def calculate_winner():
    lines = [row for row in board]
    for col in range(0, 3):
        lines.append([board[row][col] for row in range(0, 3)])
    lines.append([board[x][x] for x in range(0, 3)])
    lines.append([board[x][2 - x] for x in range(0, 3)])
    for line in lines:
        line_sum = sum(line)
        if line_sum == ME * 3:
            return ME
        elif line_sum == THEM * 3:
            return THEM
    return EMPTY


def num_to_symbol(num: int) -> str:
    if num == EMPTY:
        return " "
    elif num == ME:
        return "X"
    elif num == THEM:
        return "O"
    else:
        assert False


# Prints to stderr to not confuse the manager
def print_board():
    for row in range(0, 3):
        print("|".join([num_to_symbol(val) for val in board[row]]), file=sys.stderr)
        if row != 2:
            print("-----", file=sys.stderr)


def get_board(numpad_coord: int) -> int:
    return board[numpad_coord // 3][numpad_coord % 3]


def set_board(numpad_coord: int, value: int):
    board[numpad_coord // 3][numpad_coord % 3] = value


def numpad_coord_of(row: int, col: int) -> int:
    return row * 3 + col


# Return a winning move or -1 if no winning move exists
def move_that_wins() -> int:
    for row in range(0, 3):
        row_iterable = board[row]
        if sum(row_iterable) == 2:
            return numpad_coord_of(row, row_iterable.index(min(row_iterable)))
    for col in range(0, 3):
        col_iterable = [board[row][col] for row in range(0, 3)]
        if sum(col_iterable) == 2:
            return numpad_coord_of(col_iterable.index(min(col_iterable)), col)
    nw_se_iterable = [board[x][x] for x in range(0, 3)]
    if sum(nw_se_iterable) == 2:
        x = nw_se_iterable.index(min(nw_se_iterable))
        return numpad_coord_of(x, x)
    ne_sw_iterable = [board[x][2 - x] for x in range(0, 3)]
    if sum(ne_sw_iterable) == 2:
        x = ne_sw_iterable.index(min(ne_sw_iterable))
        return numpad_coord_of(x, 2 - x)
    return -1


my_turn = bool(int(input()))
while True:
    print_board()
    winner = calculate_winner()
    if winner == ME:
        print("I won!!", file=sys.stderr)
        sys.exit(0)
    elif winner == THEM:
        print("I lost :(", file=sys.stderr)
        sys.exit(0)
    if not my_turn:
        opponent_move = int(input())
        current_val = get_board(opponent_move)
        assert current_val == EMPTY
        set_board(opponent_move, THEM)
        my_turn = True
    else:
        move = move_that_wins()
        while move < 0 or get_board(move) != EMPTY:
            move = random.randint(0, 8)
        print(move, flush=True)
        set_board(move, ME)
        my_turn = False
