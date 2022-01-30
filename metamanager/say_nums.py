#!/usr/bin/env python3
import random
import sys
from time import sleep

for i in range(5):
    sleep(random.random() * 3)
    print(f"1:{i}", flush=True)
