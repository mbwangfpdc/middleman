#!/usr/bin/env python3
import random
import sys
from time import sleep

for i in range(5):
    heard = input()
    print(f"I heard {heard}", file=sys.stderr)
