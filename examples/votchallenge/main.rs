mod trax_protocol;

use std::io::stdin;

use mosse::{MosseTrackerSettings, MultiMosseTracker};

use crate::trax_protocol::{
    ChannelType, Image, ImageType, Region, RegionType, TraxMessageFromClient, TraxMessageFromServer,
};

#[derive(Debug)]
pub enum ServerState {
    Introduction,
    Initialization,
    Reporting {
        multi_tracker: MultiMosseTracker,
        first_region: Region,
    },
    Termination,
}

struct MosseTraxServer {
    state: ServerState,
}
impl Default for MosseTraxServer {
    fn default() -> Self {
        Self {
            state: ServerState::Introduction,
        }
    }
}
impl MosseTraxServer {
    fn run(mut self) {
        log::info!("starting run");

        println!("{}", self.make_hello_message());

        for line in stdin().lines() {
            let line = line.unwrap();
            log::trace!("handling line: {line:?}");
            let message: TraxMessageFromClient = line.parse().unwrap();
            let response = self.process_message(message);
            println!("{}", response);
        }
    }

    fn make_hello_message(&mut self) -> TraxMessageFromServer {
        TraxMessageFromServer::Hello {
            version: 1,
            name: "MosseRust".to_string(),
            identifier: "mosse-tracker-rust".to_string(),
            image: ImageType::Path,
            region: RegionType::Rectangle,
            channels: vec![ChannelType::Color],
        }
    }

    fn process_message(&mut self, message: TraxMessageFromClient) -> TraxMessageFromServer {
        match message {
            TraxMessageFromClient::Initialize { image, region } => self.process_init(image, region),
            TraxMessageFromClient::Frame { images } => self.process_frame(images),
            // FIXME: return Result from this function, and make the outer loop print "quit" and exit on error?
            TraxMessageFromClient::Quit => panic!("client sent quit message"),
        }
    }

    fn process_init(&mut self, image: Image, region: Region) -> TraxMessageFromServer {
        let first = image.open().unwrap();

        // initialize a new model
        let (width, height) = first.to_rgb8().dimensions();
        // FIXME: This tracks a square that entirely encloses the target region, so it may fixate
        // on the background for tall or wide targets.
        let window_size = f64::max(region.width, region.height) as u32; // size of the tracking window
        let psr_thresh = 7.0; // how high the psr must be before prediction is considered succesful.
        let settings = MosseTrackerSettings {
            window_size: window_size,
            width,
            height,
            regularization: 0.001,
            learning_rate: 0.05,
            psr_threshold: psr_thresh,
        };

        // FIXME: Could I get away with a single MosseTracker here? This would make things simpler,
        // but wouldn't change the results of the benchmark.
        let desperation_threshold = 300000; // how many frames the tracker should try to re-acquire the target until we consider it failed
        let mut multi_tracker = MultiMosseTracker::new(settings, desperation_threshold);

        let coords = (
            (region.x + region.width / 2.) as u32,
            (region.y + region.height / 2.) as u32,
        );
        multi_tracker.add_or_replace_target(0, coords, &first.to_luma8());

        self.state = ServerState::Reporting {
            multi_tracker,
            first_region: region.clone(),
        };

        // if we were being honest, we would return the square region that we've
        // actually fed into the model, but it probably doesn't matter that much.
        TraxMessageFromServer::State { region }
    }

    fn process_frame(&mut self, images: Vec<Image>) -> TraxMessageFromServer {
        assert_eq!(
            images.len(),
            1,
            "TODO: handle multiple images in the same frame message?"
        );

        // FIXME: use let...else for this when it becomes stable
        let (multi_tracker, first_region) = if let ServerState::Reporting {
            ref mut multi_tracker,
            ref first_region,
        } = self.state
        {
            (multi_tracker, first_region)
        } else {
            panic!("received `frame` message when not in the Reporting state")
        };

        let frame = &images[0].open().unwrap();
        let predictions = multi_tracker.track(&frame.to_luma8());
        assert_eq!(predictions.len(), 1);
        let (_obj_id, pred) = &predictions[0];

        let region = Region {
            x: pred
                .location
                .0
                .saturating_sub((first_region.width / 2.) as u32) as f64,
            y: pred
                .location
                .1
                .saturating_sub((first_region.height / 2.) as u32) as f64,
            height: first_region.height,
            width: first_region.width,
        };

        #[cfg(debug_assertions)]
        {
            let mut img_copy = frame.clone();
            imageproc::drawing::draw_hollow_rect_mut(
                &mut img_copy,
                imageproc::rect::Rect::at(region.x as i32, region.y as i32)
                    .of_size(region.width as u32, region.height as u32),
                image::Rgba([125u8, 255u8, 0u8, 0u8]),
            );
            img_copy
                .save(images[0].path.with_extension(".predicted.png"))
                .unwrap();
        }
        TraxMessageFromServer::State { region }
    }
}

fn main() {
    env_logger::init();

    let server = MosseTraxServer::default();
    server.run();
}
