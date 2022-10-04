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
    Reporting { multi_tracker: MultiMosseTracker },
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
            TraxMessageFromClient::Frame { images } => {
                todo!("handle Frame {{ images: {images:?} }}")
            }
            // FIXME: return Result from this function, and make the outer loop print "quit" and exit on error?
            TraxMessageFromClient::Quit => panic!("client sent quit message"),
        }
    }
    fn process_init(&mut self, image: Image, region: Region) -> TraxMessageFromServer {
        assert!(matches!(self.state, ServerState::Introduction));

        let first = image.open().unwrap();

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
        let multi_tracker = MultiMosseTracker::new(settings, desperation_threshold);

        // FIXME: make this function return the new state, more like a redux store?
        self.state = ServerState::Reporting { multi_tracker };

        // if we were being honest, we would return the square region that we've
        // actually fed into the model, but it probably doesn't matter that much.
        TraxMessageFromServer::State { region }
    }
}

fn main() {
    env_logger::init();

    let server = MosseTraxServer::default();
    server.run();
}
