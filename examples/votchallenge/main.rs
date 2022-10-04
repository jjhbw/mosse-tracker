use std::io::stdin;
use trax_protocol::{
    ChannelType, ImageType, RegionType, ServerState, TraxMessageFromClient, TraxMessageFromServer,
};

mod trax_protocol;

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
            TraxMessageFromClient::Initialize { image, region } => {
                todo!("handle Initialize {{ image: {image:?}, region: {region:?} }}")
            }
            TraxMessageFromClient::Frame { images } => {
                todo!("handle Frame {{ images: {images:?} }}")
            }
            // FIXME: return Result from this function, and make the outer loop print "quit" and exit on error?
            TraxMessageFromClient::Quit => panic!("client sent quit message"),
        }
    }
}

fn main() {
    env_logger::init();

    let server = MosseTraxServer::default();
    server.run();
}
