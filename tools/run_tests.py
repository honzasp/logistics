#!/usr/bin/env python3
import os.path
import subprocess
import time

TOOLS_DIR = os.path.dirname(__file__)
INPUTS_DIR = os.path.join(TOOLS_DIR, "..", "inputs")
OUTPUTS_DIR = os.path.join(TOOLS_DIR, "..", "outputs")
TARGET_DIR = os.path.join(TOOLS_DIR, "..", "target", "release")
GENERATOR_EXE = os.path.join(TOOLS_DIR, "Generator.exe")
EVALUATOR_EXE = os.path.join(TOOLS_DIR, "Evaluator.exe")
RUN_EXE = "mono"
LOGISTICS_BIN = os.path.join(TARGET_DIR, "logistics")

def gen_input(cities, depos, trucks, planes, parcels, output_file):
    cmd = [
        RUN_EXE, GENERATOR_EXE,
        str(cities), str(depos), str(trucks), str(planes), str(parcels),
        output_file,
    ]
    subprocess.run(cmd).check_returncode()

def run_test(name, cities, depos, trucks, planes, parcels):
    input_file = os.path.join(INPUTS_DIR, f"{name}.txt")
    output_file = os.path.join(OUTPUTS_DIR, f"{name}.txt")

    if not os.path.exists(input_file):
        gen_input(cities, depos, trucks, planes, parcels, input_file)

    print(f"TEST {name}")
    subprocess.run([LOGISTICS_BIN, input_file, output_file]).check_returncode()
    subprocess.run([RUN_EXE, EVALUATOR_EXE, input_file, output_file]).check_returncode()
    print()

if False:
    for n in [4, 8, 16, 32, 64, 128, 256]:
        run_test(f"e01_dense_{n}", n, n, n, max(1, n//8), 2*n*n)

if False:
    for n in [4, 8, 16, 32, 64, 128, 256, 512, 1024]:
        run_test(f"e01_sparse_{n}", n, n, n, max(1, n//8), 20*n)

if False:
    for n in [4, 8, 16, 32, 64, 128, 256]:
        run_test(f"e02_dense_{n}", 4, 4*n, n, 1, 2*n*n)

if False:
    for n in [4, 8, 16, 32, 64, 128, 256, 512, 1024]:
        run_test(f"e02_sparse_{n}", 4, 4*n, n, 1, 20*n)

if True:
    for n in [10, 100, 1000, 10*1000, 100*1000, 1000*1000]:
        run_test(f"e03_{n}", 100, 2000, 200, 10, n)

