#!/usr/bin/env python3
# The `/cmd` bot's command runner.
#
# Ported from polkadot-fellows/runtimes (.github/scripts/cmd/cmd.py), Apache-2.0, and adapted to
# FRAME Contrib: benchmarks run on the kitchensink runtime (`fc-kitchensink-runtime`), and write
# each pallet's default weights to `pallets/<pallet>/src/weights.rs`.
#
# It runs from the root of the checked-out repository (the pull request's head), so it works for
# every maintained line that has a kitchensink.

import argparse
import os
import subprocess
import sys

import _help

_HelpAction = _help._HelpAction

RUNTIME_PACKAGE = "fc-kitchensink-runtime"
PROFILE = "release"
TARGET_DIR = os.environ.get("CARGO_TARGET_DIR", "target")
WASM = f"{TARGET_DIR}/{PROFILE}/wbuild/{RUNTIME_PACKAGE}/{RUNTIME_PACKAGE.replace('-', '_')}.compact.compressed.wasm"
TEMPLATE = ".maintain/frame-weight-template.hbs"
HEADER = ".maintain/file_header.txt"
PALLET_PREFIX = "fc_pallet_"

common_args = {
    '--continue-on-fail': {"action": "store_true", "help": "Won't exit(1) on a failed command and continues with the "
                                                           "next steps. Helpful to push at least the successful "
                                                           "pallets, and then run the failed ones separately"},
    '--quiet': {"action": "store_true", "help": "Won't print start/end/failed messages in the pull request"},
    '--clean': {"action": "store_true", "help": "Cleans up the previous bot's and author's comments in the pull "
                                                "request which triggered /cmd"},
}

parser = argparse.ArgumentParser(prog="/cmd ", description='A command runner for the FRAME Contrib repo',
                                 add_help=False)
parser.add_argument('--help', action=_HelpAction, help='help for help if you need some help')  # help for help

subparsers = parser.add_subparsers(help='a command to run', dest='command')

"""
BENCH
"""

bench_example = '''**Examples**:

 > runs every benchmark, and updates each pallet's default weights

 %(prog)s

 > runs the benchmarks of fc_pallet_pass and fc_pallet_payments
 > --quiet makes it output nothing to the pull request but reactions

 %(prog)s --pallet fc_pallet_pass fc_pallet_payments --quiet

 > runs every benchmark, and continues even if some fail

 %(prog)s --continue-on-fail

 > does not output anything and cleans up the previous bot's and author's command triggering comments

 %(prog)s --pallet fc_pallet_pass --quiet --clean

 '''

parser_bench = subparsers.add_parser('bench', help="Runs benchmarks on the kitchensink runtime, and updates the "
                                                   "pallets' default weights",
                                     epilog=bench_example, formatter_class=argparse.RawDescriptionHelpFormatter)

for arg, config in common_args.items():
    parser_bench.add_argument(arg, **config)

parser_bench.add_argument('--pallet', help='Pallet(s) space separated (e.g. fc_pallet_pass); all by default',
                          nargs='*', default=[])
# Used by the bot, which builds the runtime and runs the benchmarks on different machines.
parser_bench.add_argument('--runtime', help=argparse.SUPPRESS, default=None)

"""
FMT
"""
parser_fmt = subparsers.add_parser('fmt', help='Formats code (cargo fmt, as CI checks it)')
for arg, config in common_args.items():
    parser_fmt.add_argument(arg, **config)


def pallet_dir(pallet):
    """Maps a benchmark name (e.g. `fc_pallet_referenda_tracks`) to its pallet directory."""
    if not pallet.startswith(PALLET_PREFIX):
        return None
    path = os.path.join("pallets", pallet[len(PALLET_PREFIX):].replace('_', '-'))
    return path if os.path.isdir(path) else None


def list_pallets(wasm):
    result = subprocess.run(
        ["frame-omni-bencher", "v1", "benchmark", "pallet", "--no-csv-header", "--all", "--list",
         f"--runtime={wasm}"],
        capture_output=True, text=True)
    if result.returncode != 0:
        print(f"Failed to list the benchmarks of the kitchensink runtime: {result.stderr}")
        sys.exit(1)
    return sorted({line.split(',')[0].strip() for line in result.stdout.splitlines() if line.strip()})


def bench(args):
    wasm = args.runtime
    if wasm is None:
        print(f'-- compiling {RUNTIME_PACKAGE} with runtime-benchmarks')
        result = subprocess.run(
            ["cargo", "build", "--locked", "-p", RUNTIME_PACKAGE, "--profile", PROFILE, "-q", "--features",
             "runtime-benchmarks"])
        if result.returncode != 0:
            print(f"Failed to build {RUNTIME_PACKAGE}")
            sys.exit(1)
        wasm = WASM
    print(f'-- using the runtime at {wasm}')

    available = list_pallets(wasm)
    print(f'-- pallets with benchmarks: {available}')

    pallets = args.pallet or available
    unknown = [p for p in pallets if p not in available]
    if unknown:
        print(f'❌ No benchmarks for {unknown} in the kitchensink runtime. Available: {available}')
        sys.exit(1)

    unmapped = [p for p in pallets if pallet_dir(p) is None]
    if unmapped:
        print(f'❌ Cannot find the directory of {unmapped} (expected `pallets/<name>` for `{PALLET_PREFIX}<name>`)')
        sys.exit(1)

    header = os.path.abspath(HEADER)
    template = os.path.abspath(TEMPLATE)
    failed, successful = [], []

    for pallet in pallets:
        output = os.path.join(pallet_dir(pallet), "src", "weights.rs")
        print(f'-- benchmarking {pallet} into {output}')

        status = subprocess.run(
            ["frame-omni-bencher", "v1", "benchmark", "pallet",
             f"--runtime={wasm}",
             f"--pallet={pallet}",
             "--extrinsic=*",
             "--steps=50",
             "--repeat=20",
             "--wasm-execution=compiled",
             "--heap-pages=4096",
             f"--template={template}",
             f"--header={header}",
             f"--output={output}",
             "--quiet"],
            env={**os.environ, "RUNTIME_LOG": "off"}).returncode

        if status != 0:
            failed.append(pallet)
            if not args.continue_on_fail:
                print(f'Failed to benchmark {pallet}')
                sys.exit(1)
        else:
            successful.append(pallet)

    if failed:
        print(f'❌ Failed benchmarks: {failed}')
    if successful:
        print(f'✅ Successful benchmarks: {successful}')
    # With `--continue-on-fail`, the successful benchmarks are still worth committing.
    if failed and not (args.continue_on_fail and successful):
        sys.exit(1)


def fmt(args):
    # Same check as CI (`cargo fmt --all -- --check`), with the stable toolchain.
    command = "cargo fmt --all"
    print(f'Formatting with `{command}`')
    if os.system(command) != 0:
        print('❌ Failed to format code')
        if not args.continue_on_fail:
            sys.exit(1)


if __name__ == '__main__':
    args, unknown = parser.parse_known_args()
    print(f'args: {args}')

    if args.command == 'bench':
        bench(args)
    elif args.command == 'fmt':
        fmt(args)
    else:
        parser.print_help()
        sys.exit(1)
