# MOSSE tracker in Rust
A Rust implementation of  the Minimum Output Sum of Squared Error (MOSSE) tracking algorithm, as presented in the 2010 paper [Visual Object Tracking using Adaptive Correlation Filters](https://www.cs.colostate.edu/~vision/publications/bolme_cvpr10.pdf) by David S. Bolme et al.

![example](example.gif)

For a bit of extra context, check out the accompanying blog post at https://barkeywolf.consulting/posts/mosse-tracker/.



# Running it

## Cut up a video into frames
```bash
ffmpeg -i ./testdata/traffic.mp4 -vf fps=30 ./testdata/traffic/img%04d.png
```

## Run the example binary

Running a debug build (not using the `--release` flag) will dump the state of the filter at each frame to a file and will output additional debug information. Note that the image filenames need to be provided in order. Below commands should result in `test_tracking.mp4`.

```bash
cargo run --release --example demo $(ls ./testdata/traffic/img0*.png) &&\
ffmpeg -y -framerate 30 -i ./predicted_image_%4d.png -pix_fmt yuv420p test_tracking.mp4 &&\
rm *.png
```

