# Running the votchallenge.net benchmarks against mosse-tracker

These instructions are adapted from https://www.votchallenge.net/howto/tutorial_python.html

All instructions assume that you are in this directory.

## Installing vot tool in a python virtualenv

First, you need to install the vot python tool, to run the benchmarks.

At the time of writing, vot does not work with python 3.10 on macos (import error when starting up), so you may need to use an older version.

```bash
cd examples/votchallenge # if you are not already here

python3.9 -m venv vot_venv
source vot_venv/bin/activate
pip install git+https://github.com/votchallenge/vot-toolkit-python
```

## Set things up for this project

```bash
cd examples/votchallenge # if you are not already here

cargo build --release --example=votchallenge
cp trackers.template.ini trackers.ini
```

Then change the last line of your new `trackers.ini`, to point at your
`target/release/examples/votchallenge` executable. This must be an absolute path.

## Check with a dummy sequence

```bash
cd examples/votchallenge && source vot_venv/bin/activate # if you are not already here

vot test MosseRust
```

## Run the full benchmark suite 

```bash
cd examples/votchallenge && source vot_venv/bin/activate # if you are not already here

vot test MosseRust
```

This only uses a couple of cores, but it will take at least 10 minutes, so go make yourself a cup of tea.

## Checking your scores

```bash
cd examples/votchallenge && source vot_venv/bin/activate # if you are not already here

vot analysis MosseRust
```
