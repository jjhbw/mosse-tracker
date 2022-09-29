# MOSSE tracker in Rust

A Rust implementation of  the Minimum Output Sum of Squared Error (MOSSE) tracking algorithm, as presented in the 2010 paper [Visual Object Tracking using Adaptive Correlation Filters](https://www.cs.colostate.edu/~vision/publications/bolme_cvpr10.pdf) by David S. Bolme et al.

![example](example.gif)

For a bit of extra context, check out the accompanying blog post at https://barkeywolf.consulting/posts/mosse-tracker/.

## Running it

### Cut up a video into frames

```bash
ffmpeg -i ./testdata/traffic.mp4 -vf fps=30 ./testdata/traffic/img%04d.png
```

### Run the example binary

Running a debug build (not using the `--release` flag) will dump the state of the filter at each frame to a file and will output additional debug information. Note that the image filenames need to be provided in order. Below commands should result in `test_tracking.mp4`.

```bash
cargo run --release --example demo $(ls ./testdata/traffic/img0*.png) &&\
ffmpeg -y -framerate 30 -i ./predicted_image_%4d.png -pix_fmt yuv420p test_tracking.mp4 &&\
rm *.png
```

### Run web example

```bash
wasm-pack build --no-default-features --target web
python3 -m http.server
```

Open [http://localhost:8000](http://localhost:8000) and allow webcam access.

### Run against object tracking benchmark dataset

First, download some example sequences from the [Visual Tracker Benchmark](http://cvlab.hanyang.ac.kr/tracker_benchmark/datasets.html) website and unpack them into the `testdata` directory. This is dataset is also known as TB-50 TB-100, or OTB-2015.

You can download the datasets like this (if you have [pup](https://github.com/ericchiang/pup) installed and quite a lot of patience):

```bash
# Download the zipped example sequences.
curl http://cvlab.hanyang.ac.kr/tracker_benchmark/datasets.html |
    pup 'table.seqtable:nth-child(5) > tbody:nth-child(1) > tr > td > a attr{href}' |
    sed s:seq/:: |
    while read zipfile; do
        echo $zipfile;
        if [ -f "testdata/$zipfile" ] && unzip -t "testdata/$zipfile"; then
            echo "already downloaded $zipfile"
            continue
        fi
        curl "http://cvlab.hanyang.ac.kr/tracker_benchmark/seq/$zipfile" --output "testdata/$zipfile";
    done

# Extract the sequence from each zip file.
(
    cd testdata
    for zip in *.zip; do
        unzip -f $zip || break
    done
)

# Run the benchmark example against each sequence.
for dir in ./testdata/*/; do
    echo $dir
    if [ ! -d "$dir/img" ]; then
        echo "$dir does not contain an img directory. Skipping."
        continue
    fi

    cargo run --release --example benchmark $dir
done
```

Note that MOSSE does not handle occlusion, so we expect to lose track in sequences with the OCC attribute.
