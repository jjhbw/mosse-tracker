extern crate image;
extern crate imageproc;
extern crate rustfft;

use image::{imageops, GrayImage, ImageBuffer, Luma};
use imageproc::geometric_transformations::Projection;
use imageproc::geometric_transformations::{rotate_about_center, warp, Interpolation};
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::{Fft, FftPlanner};
use std::cmp::Ordering;
use std::f32;
use std::sync::Arc;

// TODO: use constant declarations wherever possible
// TODO: refactor the unwrap statement into match statements wherever we can't be certain a result exists.
// TODO: behaviour at edge of frame: target may not leave frame, but filter will screw up anyway due to cropping. Move target coord freely within template?
// TODO: improve initial filter quality: additional affine perturbations, like scaling (zooming)?
// TODO: 11x11 window around peak for PSR calculation is arbitrary and seems biased towards larger video feeds?
// TODO: make k (number of perturbarions) a hyperparameter. k = 0 should not be allowed as it is senseless.
// TODO: FFT objects may be thread safe (Arc), but are they blocking during concurrent calls? See https://docs.rs/crate/rustfft/2.1.0/source/examples/concurrency.rs
// TODO: Double check: prevent division by zero (everywhere)? Or use div_checked? Inf is not acceptable!!

// // OPTIMIZATIONS
// TODO: call add_target asynchronously to avoid blocking on the relatively long call to .train()?
// TODO: use stack-based variable length data types https://gist.github.com/jFransham/369a86eff00e5f280ed25121454acec1#use-stack-based-variable-length-datatypes
// TODO: something stack-allocated like arrayvec = "0.4.7"?
// TODO: training: preprocess is called for each perturbation. May be best to have preprocess return an image, or have it modify in place.
// TODO: carefully track data dependencies in predict function (but get a working version first!)
// TODO: in general: avoid .collect()'ing iterators where possible
// TODO: update routine can use more in-place modifications to reduce space complexity and allocs
// TODO: update routine: benchmark initialization of Gaussian peak on target coordinates.
// TODO: in general: remove allocating functions by reusing buffers where possible (such as self.prev's)

fn preprocess(image: &GrayImage) -> Vec<f32> {
    let mut prepped: Vec<f32> = image
        .pixels()
        // convert the pixel to u8 and then to f32
        .map(|p| p[0] as f32)
        // add 1, and take the natural logarithm
        .map(|p| (p + 1.0).ln())
        .collect();

    // normalize to mean = 0 (subtract image-wide mean from each pixel)
    let sum: f32 = prepped.iter().sum();
    let mean: f32 = sum / prepped.len() as f32;
    prepped.iter_mut().for_each(|p| *p = *p - mean);

    // normalize to norm = 1, if possible
    let u: f32 = prepped.iter().map(|a| a * a).sum();
    let norm = u.sqrt();
    if norm != 0.0 {
        prepped.iter_mut().for_each(|e| *e = *e / norm)
    }

    // multiply each pixel by a cosine window
    let (width, height) = image.dimensions();
    let mut position = 0;
    for i in 0..width {
        for j in 0..height {
            let cww = ((f32::consts::PI * i as f32) / (width - 1) as f32).sin();
            let cwh = ((f32::consts::PI * j as f32) / (height - 1) as f32).sin();
            prepped[position] = cww.min(cwh) * prepped[position];
            position += 1;
        }
    }

    return prepped;
}

type Identifier = u32;

pub struct MultiMosseTracker {
    // we also store the tracker's numeric ID, and the amount of times it did not make the PSR threshold.
    trackers: Vec<(Identifier, u32, MosseTracker)>,

    // the global tracker settings
    settings: MosseTrackerSettings,

    // how many times a tracker is allowed to fail the PSR threshold
    desperation_level: u32,
}

impl MultiMosseTracker {
    pub fn new(settings: MosseTrackerSettings, desperation_level: u32) -> MultiMosseTracker {
        return MultiMosseTracker {
            trackers: Vec::new(),
            settings: settings,
            desperation_level: desperation_level,
        };
    }

    pub fn add_target(&mut self, id: Identifier, coords: (u32, u32), frame: &GrayImage) {
        // Or do this in the caller?

        // create a new tracker for this target and train it
        let mut new_tracker = MosseTracker::new(&self.settings);
        new_tracker.train(frame, coords);

        match self.trackers.iter_mut().find(|tracker| tracker.0 == id) {
            Some(tuple) => {
                tuple.1 = 0;
                tuple.2 = new_tracker;
            }
            // add the tracker to the map
            _ => self.trackers.push((id, 0, new_tracker)),
        };
    }

    pub fn track(&mut self, frame: &GrayImage) -> Vec<(Identifier, Prediction)> {
        let mut predictions: Vec<(Identifier, Prediction)> = Vec::new();
        for (id, death_watch, tracker) in &mut self.trackers {
            // compute the location of the object in the new frame and save it
            let pred = tracker.track_new_frame(frame);
            predictions.push((*id, pred));

            // if the tracker made the PSR threshold, update it.
            // if not, we increment its death ticker.
            if tracker.last_psr > self.settings.psr_threshold {
                tracker.update(frame);
                *death_watch = 0u32;
            } else {
                *death_watch += 1;
            }
        }

        // prune all filters with an expired death ticker
        let level = &self.desperation_level;
        self.trackers
            .retain(|(_id, death_count, _tracker)| death_count < level);

        return predictions;
    }

    pub fn dump_filter_reals(&self) -> Vec<GrayImage> {
        return self.trackers.iter().map(|t| t.2.dump_filter().0).collect();
    }

    pub fn size(&self) -> usize {
        self.trackers.len()
    }
}

pub struct Prediction {
    pub location: (u32, u32),
    pub psr: f32,
}

pub struct MosseTracker {
    filter: Vec<Complex<f32>>,

    // constants frame height
    frame_width: u32,
    frame_height: u32,

    // stores dimensions of tracking window and its center
    // window is square for now, this variable contains the size of the square edge
    window_size: u32,
    current_target_center: (u32, u32), // represents center in frame

    // the 'target' (G). A single Gaussian peak centered at the tracking window.
    target: Vec<Complex<f32>>,

    // constants: learning rate and PSR threshold
    eta: f32,
    regularization: f32, // not super important for MOSSE: see paper fig 4.

    // the previous Ai and Bi
    last_top: Vec<Complex<f32>>,
    last_bottom: Vec<Complex<f32>>,

    // the previous psr
    pub last_psr: f32,

    // thread-safe FFT objects containing precomputed parameters for this input data size.
    fft: Arc<dyn Fft<f32>>,
    inv_fft: Arc<dyn Fft<f32>>,
}

pub struct MosseTrackerSettings {
    pub width: u32,
    pub height: u32,
    pub window_size: u32,
    pub learning_rate: f32,
    pub psr_threshold: f32,
    pub regularization: f32,
}

impl MosseTracker {
    pub fn new(settings: &MosseTrackerSettings) -> MosseTracker {
        // parameterize the FFT objects
        let mut planner = FftPlanner::new();
        let mut inv_planner = FftPlanner::new();

        // NOTE: we initialize the FFTs based on the size of the window
        let length = (settings.window_size * settings.window_size) as usize;
        let fft = planner.plan_fft_forward(length);
        let inv_fft = inv_planner.plan_fft_inverse(length);

        // initialize the filter and its top and bottom parts with zeroes.
        let filter = vec![Complex::zero(); length];
        let top = vec![Complex::zero(); length];
        let bottom = vec![Complex::zero(); length];

        // initialize the target output map (G), with a compact Gaussian peak centered on the target object.
        // In the Bolme paper, this map is called gi.
        let mut target: Vec<Complex<f32>> =
            build_target(settings.window_size, settings.window_size)
                .into_iter()
                .map(|p| Complex::new(p as f32, 0.0))
                .collect();
        fft.process(&mut target);

        return MosseTracker {
            filter: filter,
            last_top: top,
            last_bottom: bottom,
            last_psr: 0.0,
            eta: settings.learning_rate,
            regularization: settings.regularization,
            target: target,
            fft: fft,
            inv_fft: inv_fft,
            frame_width: settings.width,
            frame_height: settings.height,
            window_size: settings.window_size,
            current_target_center: (0, 0),
        };
    }

    fn compute_2dfft(&self, imagedata: Vec<f32>) -> Vec<Complex<f32>> {
        let mut buffer: Vec<Complex<f32>> = imagedata
            .into_iter()
            .map(|p| Complex::new(p as f32, 0.0))
            .collect();

        // fft.process() CONSUMES the input buffer as scratch space, make sure it is not reused
        self.fft.process(&mut buffer);

        return buffer;
    }

    // Train a new filter on the first frame in which the object occurs
    pub fn train(&mut self, input_frame: &GrayImage, target_center: (u32, u32)) {
        // store the target center as the current
        self.current_target_center = target_center;

        // cut out the training template by cropping
        let window = &window_crop(
            input_frame,
            self.window_size,
            self.window_size,
            target_center,
        );

        #[cfg(debug_assertions)]
        {
            window.save("WINDOW.png").unwrap();
        }

        // build an iterator that produces training frames that have been slightly rotated according to a theta value.
        let rotated_frames = [
            0.02, -0.02, 0.05, -0.05, 0.07, -0.07, 0.09, -0.09, 1.1, -1.1, 1.3, -1.3, 1.5, -1.5,
            2.0, -2.0,
        ]
        .iter()
        .map(|rad| {
            // Rotate an image clockwise about its center by theta radians.
            let training_frame =
                rotate_about_center(window, *rad, Interpolation::Nearest, Luma([0]));

            #[cfg(debug_assertions)]
            {
                training_frame
                    .save(format!("training_frame_rotated_theta_{}.png", rad))
                    .unwrap();
            }

            return training_frame;
        });

        // build an iterator that produces training frames that have been slightly scaled to various degrees ('zoomed')
        let scaled_frames = [0.8, 0.9, 1.1, 1.2].into_iter().map(|scalefactor| {
            let scale = Projection::scale(scalefactor, scalefactor);

            let scaled_training_frame = warp(&window, &scale, Interpolation::Nearest, Luma([0]));

            #[cfg(debug_assertions)]
            {
                scaled_training_frame
                    .save(format!("training_frame_scaled_{}.png", scalefactor))
                    .unwrap();
            }

            return scaled_training_frame;
        });

        // Chain these iterators together.
        // Note that we add the initial, unperturbed training frame as first in line.
        let training_frames = std::iter::once(window)
            .cloned()
            .chain(rotated_frames)
            .chain(scaled_frames);
        // TODO: scaling is not ready yet
        // .chain(scaled_frames);

        let mut training_frame_count = 0;
        for training_frame in training_frames {
            // preprocess the training frame using preprocess()
            let vectorized = preprocess(&training_frame);

            // calculate the 2D FFT of the preprocessed frame: FFT(fi) = Fi
            let Fi = self.compute_2dfft(vectorized);

            //  compute the complex conjugate of Fi, Fi*.
            let Fi_star: Vec<Complex<f32>> = Fi.iter().map(|e| e.conj()).collect();

            // compute the initial filter
            let top = self.target.iter().zip(Fi_star.iter()).map(|(g, f)| g * f);
            let bottom = Fi.iter().zip(Fi_star.iter()).map(|(f, f_star)| f * f_star);

            // // add the values to the running sum
            self.last_top
                .iter_mut()
                .zip(top)
                .for_each(|(running, new)| *running += new);

            self.last_bottom
                .iter_mut()
                .zip(bottom)
                .for_each(|(running, new)| *running += new);

            training_frame_count += 1
        }

        // divide the values of the top and bottom filters by the number of training perturbations used
        self.last_top
            .iter_mut()
            .for_each(|e| *e /= training_frame_count as f32);

        self.last_bottom
            .iter_mut()
            .for_each(|e| *e /= training_frame_count as f32);

        // compute the filter by dividing Ai and Bi elementwise
        // note that we add a small quantity to avoid dividing by zero, which would yield NaN's.
        self.filter = self
            .last_top
            .iter()
            .zip(&self.last_bottom)
            .map(|(a, b)| a / b + self.regularization)
            .collect();

        #[cfg(debug_assertions)]
        {
            println!(
                "current center of target in frame: x={}, y={}",
                self.current_target_center.0, self.current_target_center.1
            );
        }
    }

    pub fn track_new_frame(&mut self, frame: &GrayImage) -> Prediction {
        // cut out the training template by cropping
        let window = window_crop(
            frame,
            self.window_size,
            self.window_size,
            self.current_target_center,
        );

        // preprocess the image using preprocess()
        let vectorized = preprocess(&window);

        // calculate the 2D FFT of the preprocessed image: FFT(fi) = Fi
        let Fi = self.compute_2dfft(vectorized);

        // elementwise multiplication of F with filter H gives Gi
        let mut corr_map_gi: Vec<Complex<f32>> =
            Fi.iter().zip(&self.filter).map(|(a, b)| a * b).collect();

        // NOTE: Gi is garbage after this call
        self.inv_fft.process(&mut corr_map_gi);

        // find the max value of the filtered image 'gi', along with the position of the maximum
        let (maxind, max_complex) = corr_map_gi
            .iter()
            .enumerate()
            .max_by(|a, b| {
                // filtered (gi) is still complex at this point, we only care about the real part
                a.1.re.partial_cmp(&b.1.re).unwrap_or(Ordering::Equal)
            })
            .unwrap(); // we can unwrap the result of max_by(), as we are sure filtered.len() > 0

        // convert the array index of the max to the coordinates in the window
        let max_coord_in_window = index_to_coords(self.window_size, maxind as u32);

        let window_half = (self.window_size / 2) as i32;
        let x_delta = max_coord_in_window.0 as i32 - window_half;
        let y_delta = max_coord_in_window.1 as i32 - window_half;
        let x_max = self.frame_width as i32 - window_half;
        let y_max = self.frame_height as i32 - window_half;

        #[cfg(debug_assertions)]
        {
            println!(
                "distance of new in-window max from window center: x = {}, y = {}",
                x_delta, y_delta,
            );
        }

        // compute the max coord in the frame by looking at the shift of the window center
        let new_x = (self.current_target_center.0 as i32 + x_delta)
            .min(x_max)
            .max(window_half);

        let new_y = (self.current_target_center.1 as i32 + y_delta)
            .min(y_max)
            .max(window_half);

        self.current_target_center = (new_x as u32, new_y as u32);

        // compute PSR
        // Note that we re-use the computed max and its coordinate for downstream simplicity
        self.last_psr = compute_psr(
            &corr_map_gi,
            self.window_size,
            self.window_size,
            max_complex.re,
            max_coord_in_window,
        );

        return Prediction {
            location: self.current_target_center,
            psr: self.last_psr,
        };
    }

    // update the filter
    fn update(&mut self, frame: &GrayImage) {
        // cut out the training template by cropping
        let window = window_crop(
            frame,
            self.window_size,
            self.window_size,
            self.current_target_center,
        );

        // preprocess the image using preprocess()
        let vectorized = preprocess(&window);

        // calculate the 2D FFT of the preprocessed image: FFT(fi) = Fi
        let new_Fi = self.compute_2dfft(vectorized);

        //// Update the filter using the prediction
        //  compute the complex conjugate of Fi, Fi*.
        let Fi_star: Vec<Complex<f32>> = new_Fi.iter().map(|e| e.conj()).collect();

        // compute Ai (top) and Bi (bottom) using F*, G, and the learning rate (see paper)
        let one_minus_eta = 1.0 - self.eta;

        // update the 'top' of the filter update equation
        self.last_top = self
            .target
            .iter()
            .zip(&Fi_star)
            .zip(&self.last_top)
            .map(|((g, f), prev)| self.eta * (g * f) + (one_minus_eta * prev))
            .collect();

        // update the 'bottom' of the filter update equation
        self.last_bottom = new_Fi
            .iter()
            .zip(&Fi_star)
            .zip(&self.last_bottom)
            .map(|((f, f_star), prev)| self.eta * (f * f_star) + (one_minus_eta * prev))
            .collect();

        // compute the new filter H* by dividing Ai and Bi elementwise
        self.filter = self
            .last_top
            .iter()
            .zip(&self.last_bottom)
            .map(|(a, b)| a / b)
            .collect();
    }

    // debug method to dump the latest filter to an inspectable image
    pub fn dump_filter(
        &self,
    ) -> (
        ImageBuffer<Luma<u8>, Vec<u8>>,
        ImageBuffer<Luma<u8>, Vec<u8>>,
    ) {
        // get the filter out of fourier space
        // NOTE: input is garbage after this call to inv_fft.process(), so we clone the filter first.
        let mut h = self.filter.clone();
        self.inv_fft.process(&mut h);

        // turn the real and imaginary values of the filter into separate grayscale images
        let realfilter = h.iter().map(|c| c.re).collect();
        let imfilter = h.iter().map(|c| c.im).collect();

        return (
            to_imgbuf(&realfilter, self.window_size, self.window_size),
            to_imgbuf(&imfilter, self.window_size, self.window_size),
        );
    }
}

fn window_crop(
    input_frame: &GrayImage,
    window_width: u32,
    window_height: u32,
    center: (u32, u32),
) -> GrayImage {
    let window = imageops::crop(
        &mut input_frame.clone(),
        center
            .0
            .saturating_sub(window_width / 2)
            .min(input_frame.width() - window_width),
        center
            .1
            .saturating_sub(window_height / 2)
            .min(input_frame.height() - window_height),
        window_width,
        window_height,
    )
    .to_image();

    return window;
}

fn build_target(window_width: u32, window_height: u32) -> Vec<f32> {
    let mut target_gi = vec![0f32; (window_width * window_height) as usize];

    // Optional: let the sigma depend on the window size (Galoogahi et al. (2015). Correlation Filters with Limited Boundaries)
    // let sigma = ((window_width * window_height) as f32).sqrt() / 16.0;
    // let variance = sigma * sigma;
    let variance = 2.0;

    // create gaussian peak at the center coordinates
    let center_x = window_width / 2;
    let center_y = window_height / 2;
    for x in 0..window_width {
        for y in 0..window_height {
            let distx: f32 = x as f32 - center_x as f32;
            let disty: f32 = y as f32 - center_y as f32;

            // apply a crude univariate Gaussian density function
            target_gi[((y * window_width) + x) as usize] =
                (-((distx * distx) + (disty * disty) / variance)).exp()
        }
    }

    return target_gi;
}

// function for debugging the shape of the target
// output only depends on the provided target_coords
pub fn dump_target(window_width: u32, window_height: u32) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    let trgt = build_target(window_width, window_height);

    let normalized = trgt.iter().map(|a| a * 255.0).collect();

    return to_imgbuf(&normalized, window_width, window_height);
}

fn compute_psr(
    predicted: &Vec<Complex<f32>>,
    width: u32,
    height: u32,
    max: f32,
    maxpos: (u32, u32),
) -> f32 {
    // uses running updates of standard deviation and mean
    let mut running_sum = 0.0;
    let mut running_sd = 0.0;
    for e in predicted {
        running_sum += e.re;
        running_sd += e.re * e.re;
    }

    // subtract the values of a 11*11 window around the max from the running sd and sum
    // TODO: look up: why 11*11, and not something simpler like 12*12?
    let max_x = maxpos.0 as i32;
    let max_y = maxpos.1 as i32;
    let window_left = (max_x - 5).max(0);
    let window_right = (max_x + 6).min(width as i32);
    let window_top = (max_y - 5).min(0); // note: named according to CG conventions
    let window_bottom = (max_y + 6).min(height as i32);
    for x in window_left..window_right {
        for y in window_bottom..window_top {
            let ind = (y * width as i32 + x) as usize;
            let val = predicted[ind].re;
            running_sd -= val * val;
            running_sum -= val;
        }
    }

    // we need to subtract 11*11 window from predicted.len() to get the sidelobe_size
    let sidelobe_size = (predicted.len() - (11 * 11)) as f32;
    let mean_sl = running_sum / sidelobe_size;
    let sd_sl = ((running_sd / sidelobe_size) - (mean_sl * mean_sl)).sqrt();
    let psr = (max - mean_sl) / sd_sl;

    return psr;
}

fn index_to_coords(width: u32, index: u32) -> (u32, u32) {
    // modulo/remainder ops are theoretically O(1)
    // checked_rem returns None if rhs == 0, which would indicate an upstream error (width == 0).
    let x = index.checked_rem(width).unwrap();

    // checked sub returns None if overflow occurred, which is also a panicable offense.
    // checked_div returns None if rhs == 0, which would indicate an upstream error (width == 0).
    let y = (index.checked_sub(x).unwrap()).checked_div(width).unwrap();
    return (x, y);
}

pub fn to_imgbuf(buf: &Vec<f32>, width: u32, height: u32) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    ImageBuffer::from_vec(width, height, buf.iter().map(|c| *c as u8).collect()).unwrap()
}

// TODO: below tests are used as a scratch pad and for syntax experiments, not serious unit testing.
#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn sanity_test_max_by() {
        let filtered: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let maxel = filtered
            .iter()
            .enumerate()
            .max_by(|a, b| {
                // filtered (gi) is still complex at this point, we only care about the real part
                a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal)
            })
            .unwrap();
        assert_eq!(maxel, (4usize, &5.0f32));
    }

    #[test]
    fn am_i_still_sane() {
        assert_eq!(
            Complex::new(1.0, -3.0) * Complex::new(2.0, 5.0),
            Complex::new(17.0, -1.0)
        );
    }

    #[test]
    fn unique_identifier() {
        let width = 64;
        let height = 64;
        let frame = GrayImage::new(width, height);
        let settings = MosseTrackerSettings {
            window_size: 16,
            width,
            height,
            regularization: 0.001,
            learning_rate: 0.05,
            psr_threshold: 7.0,
        };
        let mut multi_tracker = MultiMosseTracker::new(settings, 3);
        assert_eq!(multi_tracker.size(), 0);
        multi_tracker.add_target(0, (0, 0), &frame);

        assert_eq!(multi_tracker.size(), 1);
        assert_eq!(
            multi_tracker
                .trackers
                .iter()
                .find(|t| t.0 == 0)
                .unwrap()
                .2
                .current_target_center,
            (0, 0)
        );

        multi_tracker.add_target(1, (10, 0), &frame);

        assert_eq!(multi_tracker.size(), 2);

        multi_tracker.add_target(0, (10, 0), &frame);

        assert_eq!(multi_tracker.size(), 2);
        assert_eq!(
            multi_tracker
                .trackers
                .iter()
                .find(|t| t.0 == 0)
                .unwrap()
                .2
                .current_target_center,
            (10, 0)
        );
    }
}
