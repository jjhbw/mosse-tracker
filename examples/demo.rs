extern crate image;
extern crate imageproc;
extern crate mosse;
extern crate rusttype;
extern crate time;

use image::Rgba;
use imageproc::drawing::{draw_cross_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use mosse::{MosseTrackerSettings, MultiMosseTracker};
use rusttype::{Font, Scale};
use std::env;
use std::time::Instant;

fn main() {
    // Collect all elements in the iterator that contains the command line arguments
    let args: Vec<String> = env::args().collect();

    // remove the first element from the list of arguments, which is the call to the binary
    let inputfiles = &args[1..];

    if inputfiles.len() == 0 {
        panic!("no input files specified");
    }
    let mut images = inputfiles.iter().map(|path| image::open(path).unwrap());
    let first = images.next().unwrap();

    // initialize a new model
    let (width, height) = first.to_rgb8().dimensions();
    let window_size = 64; //size of the tracking window
    let psr_thresh = 7.0; // how high the psr must be before prediction is considered succesful.
    let settings = MosseTrackerSettings {
        window_size: window_size,
        width,
        height,
        regularization: 0.001,
        learning_rate: 0.05,
        psr_threshold: psr_thresh,
    };
    let desperation_threshold = 3; // how many frames the tracker should try to re-acquire the target until we consider it failed
    let mut multi_tracker = MultiMosseTracker::new(settings, desperation_threshold);

    // coordinates of the target objects to track in the intial frame
    let target_coords = vec![
        (200, 420),
        (560, 300)
    ];

    // Add all the targets  on the first image to the multitracker
    let first_img = first.to_luma8();
    for (i, coords) in target_coords.into_iter().enumerate() {
        let start = Instant::now();
        multi_tracker.add_target(i as u32, coords, &first_img);
        println!(
            "Added object on initial frame to multi-tracker in {} ms",
            start.elapsed().as_millis()
        );
    }

    for (i, dyn_img) in images.enumerate() {
        // add leading zeroes for easier downstream proc with ffmpeg
        let img_id = format!("{:<04}", i + 1);

        // track the objects on the new frame
        let start = Instant::now();
        let predictions = multi_tracker.track(&dyn_img.to_luma8());

        println!(
            "Processed sample image no. {} in {} ms. Active trackers: {}.",
            img_id,
            start.elapsed().as_millis(),
            multi_tracker.size(),
        );

        let mut img_copy = dyn_img;
        for (obj_id, pred) in predictions.iter() {
            // color changes when psr is low
            let mut color = Rgba([125u8, 255u8, 0u8, 0u8]);
            if pred.psr < psr_thresh {
                color = Rgba([255u8, 0u8, 0u8, 0u8])
            }

            // Indicate the locations of the predictions by drawing on the image.
            draw_cross_mut(
                &mut img_copy,
                Rgba([255u8, 0u8, 0u8, 0u8]),
                pred.location.0 as i32,
                pred.location.1 as i32,
            );
            draw_hollow_rect_mut(
                &mut img_copy,
                Rect::at(
                    pred.location.0.saturating_sub(window_size / 2) as i32,
                    pred.location.1.saturating_sub(window_size / 2) as i32,
                )
                .of_size(window_size, window_size),
                color,
            );

            let font_data = include_bytes!("./Arial.ttf");
            let font = Font::try_from_bytes(font_data as &[u8]).unwrap();

            const FONT_SCALE: f32 = 10.0;

            // render the object ID
            draw_text_mut(
                &mut img_copy,
                Rgba([125u8, 255u8, 0u8, 0u8]),
                pred.location.0 - (window_size / 2),
                pred.location.1 - (window_size / 2),
                Scale::uniform(FONT_SCALE),
                &font,
                &format!("#{}", obj_id),
            );

            // render the PSR on top of the rectangle
            draw_text_mut(
                &mut img_copy,
                color,
                pred.location.0 - (window_size / 2),
                pred.location.1 - (window_size / 2) + FONT_SCALE as u32,
                Scale::uniform(FONT_SCALE),
                &font,
                &format!("PSR: {:.2}", pred.psr),
            );

            println!("Object {} PSR: {}", obj_id, pred.psr)
        }

        // additional debug info
        #[cfg(debug_assertions)]
        {
            // save the filters
            multi_tracker
                .dump_filter_reals()
                .iter()
                .enumerate()
                .for_each(|(i, f)| {
                    f.save(format!("filter_real_obj{}_fig{}.png", i, img_id))
                        .unwrap()
                })
        }

        img_copy
            .save(format!("predicted_image_{}.png", img_id))
            .unwrap();

        // Break off multi tracker if all targets lost
        if multi_tracker.size() == 0 {
            println!("No more active trackers. Stopping demo.");
            break;
        }
    }
}
