"""M8b EDT held-out test harness, round 2 (../../HARNESS-PLAN-2.md, with its section 9): round 1's harness copied and edited in
place for round 2 only; round 1's own is ../../harness/ and is never run again. numpy and pytest only.

Scratch and output stay on C: (C:\\tmp\\m8b-edt\\); nothing here writes bytecode next to the sources.
"""
import sys

sys.dont_write_bytecode = True
