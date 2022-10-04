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

        println!("{}", self.hello());

        for line in stdin().lines() {
            let line = line.unwrap();
            log::trace!("handling line: {line:?}");
            let message: TraxMessageFromClient = line.parse().unwrap();
            let response = self.process_message(message);
            println!("{}", response);
        }
    }

    fn hello(&mut self) -> TraxMessageFromServer {
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
            TraxMessageFromClient::Quit => panic!("client sent quit message"),
        }
    }
}

fn main() {
    env_logger::init();

    let server = MosseTraxServer::default();
    server.run();
}
