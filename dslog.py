#!/usr/bin/env python
import sys
import shutil
from typing import Optional, List, Tuple, Dict

import typer
from rich import print
from rich.columns import Columns
from rich.console import Console
from rich.traceback import install

import re

def main(
    file: typer.FileText = typer.Argument(None, help="File to read, stdin otherwise"),
    n_columns: Optional[int] = typer.Option(None, "--columns", "-c"),

):
    # We can take input from a stdin (pipes) or from a file
    input_ = file if file else sys.stdin
    
    console = Console(color_system=None)
    width = console.size.width

    panic = False
    hart = 0
    last_hart = 0
    for line in input_:
        hartpat = r'(\d+\:\d+)'
        hartstr = re.findall(hartpat, line)
        if len(hartstr) != 0:
            hart = int(str(hartstr[0]).split(':')[0])
            if hart > n_columns:
                hartpat = r'(\x20\d\x20)'
                hartstr = re.findall(hartpat, line)
                if len(hartstr) != 0:
                    hart = int(str(hartstr[0]))
                    last_hart = hart
                else:
                    hart = last_hart
            last_hart = hart
        else:
            hartpat = r'(\x20\d\x20)'
            hartstr = re.findall(hartpat, line)
            if len(hartstr) != 0:
                hart = int(str(hartstr[0]))
                last_hart = hart
            else:
                hart = last_hart

        msg = line.replace('\n', '')
        cols = ["" for _ in range(n_columns)]
        msg = "" + msg
        cols[hart] = msg
        col_width = int(width / n_columns)
        cols = Columns(cols, width=col_width - 1, equal=True, expand=True)
        console.print(cols)

if __name__ == "__main__":
    typer.run(main)