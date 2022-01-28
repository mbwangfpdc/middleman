#!/usr/bin/env python3
import random
from time import sleep

for i in range(5):
    sleep(random.random() * 2)
    print(i, flush=True)
