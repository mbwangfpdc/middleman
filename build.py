#!/usr/bin/env python3
"""
Please do not judge me, I'm just trying my best.
I'll make this better later, I promise.
"""

import os
import subprocess

for root, _, files in os.walk(os.path.dirname(os.path.realpath(__file__))):
    if "Cargo.toml" in files:
        cmd_str = f"cd {root} && cargo build"
        print(f"Building {root} using '{cmd_str}'...")
        completed_process = subprocess.run(cmd_str, capture_output=True, shell=True)
        print(f"stdout: {completed_process.stdout.decode('utf-8')}")
        print(f"stdout: {completed_process.stderr.decode('utf-8')}")
