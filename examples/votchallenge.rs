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
use std::fs::File;
use std::io::{stdin, Write};
use std::time::Instant;
use trax_protocol::{
    ChannelType, ImageType, RegionType, ServerState, TraxMessageFromClient, TraxMessageFromServer,
};

/// This module implements the trax protocol as described in https://trax.readthedocs.io/en/latest/protocol.html
// FIXME: split this out into its own crate?
#[allow(dead_code)]
mod trax_protocol {
    use std::{fmt::Display, str::FromStr};

    /// messages defined by https://trax.readthedocs.io/en/latest/protocol.html#protocol-messages-and-states
    pub enum TraxMessageFromServer {
        Hello {
            /// Specifies the supported version of the protocol. If not present, version 1 is assumed.
            version: i32,
            /// Specifies the name of the tracker. The name can be used by the client to verify that the correct algorithm is executed.
            name: String,
            /// Specifies the identifier of the current implementation. The identifier can be used to determine the version of the tracker.
            identifier: String,
            /// Specifies the supported image format. See Section Image formats for the list of supported formats. By default it is assumed that the tracker can accept file paths as image source.
            image: ImageType,
            /// Specifies the supported region format. See Section Region formats for the list of supported formats. By default it is assumed that the tracker can accept rectangles as region specification.
            region: RegionType,
            /// Specifies support for multi-modal images. See Section Image channels for more information
            channels: Vec<ChannelType>,
        },
        State {
            // FIXME: make a type for this
            region: String,
        },
        Quit,
    }
    impl Display for TraxMessageFromServer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "@@TRAX:")?;
            match self {
                TraxMessageFromServer::Hello {
                    version,
                    name,
                    identifier,
                    image,
                    region,
                    channels,
                } => {
                    let channels = channels
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",");
                    write!(f, "hello trax.version={version} trax.name={name} trax.identifier={identifier} trax.image={image} trax.region={region} trax.channels={channels}")
                }
                TraxMessageFromServer::State { region } => todo!(),
                TraxMessageFromServer::Quit => todo!(),
            }
        }
    }
    pub enum TraxMessageFromClient {
        Initialize {
            // FIXME: make a type for this
            image: String,
            // FIXME: make a type for this
            region: String,
        },
        Frame {
            // FIXME: make a type for this
            images: Vec<String>,
        },
        Quit,
    }

    impl FromStr for TraxMessageFromClient {
        // we could probably use anyhow::Error or here
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let s = s.strip_prefix("@@TRAX:").unwrap();
            let (type_, rest) = s.split_once(' ').unwrap();
            let res = match type_ {
                "initialize" => {
                    let (image, region) = s.split_once(' ').unwrap();
                    // FIXME:
                    // * Make proper enums for image and region
                    // * strip out quotes and whitespace properly (tempdir might have spaces in on windows?)
                    // * strip out file:// from image path
                    // * parse region into a rectangle or something
                    Self::Initialize {
                        image: image.to_string(),
                        region: region.to_string(),
                    }
                }
                _ => panic!("don't understand message: {s:?}"),
            };
            Ok(res)
        }
    }

    /// In practice, we only plan to implement the `Path` image type in our server.
    pub enum ImageType {
        Path,
        Memory,
        Data,
        Url,
    }
    impl Display for ImageType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ImageType::Path => write!(f, "path"),
                ImageType::Memory => write!(f, "memory"),
                ImageType::Data => write!(f, "data"),
                ImageType::Url => write!(f, "url"),
            }
        }
    }

    /// In practice, we only plan to implement the `Rectangle` region type in our server.
    pub enum RegionType {
        Rectangle,
        Polygon,
    }
    impl Display for RegionType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                RegionType::Rectangle => write!(f, "rectangle"),
                RegionType::Polygon => write!(f, "polygon"),
            }
        }
    }

    /// In practice, we only plan to implement a single `Color` channel type in our server.
    pub enum ChannelType {
        Color,
        Depth,
        InfraRed,
    }
    impl Display for ChannelType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ChannelType::Color => write!(f, "color"),
                ChannelType::Depth => write!(f, "depth"),
                ChannelType::InfraRed => write!(f, "ir"),
            }
        }
    }

    // There would also be a ClientState, but we're not planning to implement the client.

    pub enum ServerState {
        Introduction,
        Initialization,
        Reporting,
        Termination,
    }
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

        println!("{}", self.hello());

        for line in stdin().lines() {
            let line = line.unwrap();
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
        todo!()
    }
}

fn main() {
    env_logger::init();

    let server = MosseTraxServer::default();
    server.run();
}
