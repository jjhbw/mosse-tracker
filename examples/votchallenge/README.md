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

This only uses a couple of cores, and take around 30 minutes, so go make yourself a cup of tea. You should see output like this:

```
Downloading sequence dataset "VOT2020" with 60 sequences.
 Downloading          |███████████████████████████████████████████████████████████████████████████| 100% [02:30<00:00]
Download completed
 Loading dataset      |███████████████████████████████████████████████████████████████████████████| 100% [00:00<00:00]
Loaded workspace in '/Users/alsuren/src/mosse-tracker/examples/votchallenge'
Found data for 1 trackers
Evaluating tracker MosseRust
 MosseRust/baseline   |███████████████████████████████████████████████████████████████████████████| 100% [13:24<00:00]
 MosseRust/realtime   |███████████████████████████████████████████████████████████████████████████| 100% [13:11<00:00]
 MosseRust/unsupervis |███████████████████████████████████████████████████████████████████████████| 100% [01:34<00:00]
Evaluation concluded successfuly
```

## Checking your scores

```bash
cd examples/votchallenge && source vot_venv/bin/activate # if you are not already here

vot analysis MosseRust
```

This is a bit quicker, and will give you something like:

```
 Loading dataset      |██████████████████████████████████████████████████████████████████████████████████| 100% [00:00<00:00]
Loaded workspace in '/Users/alsuren/src/mosse-tracker/examples/votchallenge'
Found data for 1 trackers
 Running analysis     |██████████████████████████████████████████████████████████████████████████████████| 100% [00:21<00:00]
Analysis successful, report available as 2022-10-06T22-54-20.997015
```

You can then open ./analysis/2022-10-06T22-54-20.997015/report.html in your web browser, to view the results.

## Rerunning the analysis after making a change

```bash
cd examples/votchallenge && source vot_venv/bin/activate # if you are not already here

cargo build --release --example=votchallenge
rm -rf cache/ results/
vot evaluate MosseRust && vot analysis MosseRust
```
