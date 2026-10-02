"""M8b EDT held-out test harness (../../HARNESS-PLAN.md). numpy and pytest only.

Scratch and output stay on C: (C:\\tmp\\m8b-edt\\); nothing here writes bytecode next to the sources.
"""
import sys

sys.dont_write_bytecode = True
