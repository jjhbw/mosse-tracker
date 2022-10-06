#![allow(dead_code)]
///! This module implements the trax protocol as described in https://trax.readthedocs.io/en/latest/protocol.html
// FIXME: split this out into its own crate?
use std::{fmt::Display, path::PathBuf, str::FromStr};

use image::{DynamicImage, ImageError};

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
        region: Region,
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
            TraxMessageFromServer::State { region } => {
                write!(f, "state {region}")
            }
            TraxMessageFromServer::Quit => todo!(),
        }
    }
}
#[derive(Debug)]
pub enum TraxMessageFromClient {
    Initialize { image: Image, region: Region },
    Frame { images: Vec<Image> },
    Quit,
}

impl FromStr for TraxMessageFromClient {
    // we could probably use anyhow::Error or here
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim_end().strip_prefix("@@TRAX:").unwrap();
        let (type_, rest) = s.split_once(' ').unwrap();
        let res = match type_ {
            "initialize" => {
                // FIXME:
                // * strip out quotes and whitespace properly (tempdir might have spaces in on windows?)
                let (image, region) = rest.split_once(' ').unwrap();
                Self::Initialize {
                    image: Image::from_str(strip_quotes_from_ends(image)?)?,
                    region: Region::from_str(strip_quotes_from_ends(region)?)?,
                }
            }
            "frame" => Self::Frame {
                // FIXME: https://trax.readthedocs.io/en/latest/protocol.html#protocol-messages-and-states
                // says "or multiple images", which is why I made it a Vec, but I'm not sure how this
                // should be handled by the server (split it and treat it as if it were mutiple "frame" messages?)
                // so it might be better to flatten out the Vec<Image> into a single Image.
                images: vec![Image::from_str(strip_quotes_from_ends(rest)?)?],
            },
            _ => anyhow::bail!("don't understand message: {s:?}"),
        };
        Ok(res)
    }
}

// I feel like there should be something like this in the standard library somewhere, but this will do for now.
fn strip_quotes_from_ends(s: &str) -> anyhow::Result<&str> {
    s.strip_prefix('"')
        .ok_or(anyhow::anyhow!("no leading quote on {s:?}"))?
        .strip_suffix('"')
        .ok_or(anyhow::anyhow!("no trailing quote on {s:?}"))
}

#[derive(Debug, PartialEq)]
pub enum ImageType {
    /// In practice, we only plan to implement the `Path` image type in our server.
    Path,
    Memory,
    Data,
    Url,
}
impl Display for ImageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        assert_eq!(
            self,
            &ImageType::Path,
            "only `path` image type is supported for now",
        );
        match self {
            ImageType::Path => write!(f, "path"),
            ImageType::Memory => write!(f, "memory"),
            ImageType::Data => write!(f, "data"),
            ImageType::Url => write!(f, "url"),
        }
    }
}

// In practice, we only plan to implement the `Path` image type in our server, otherwise I would have made this an enum as well.
#[derive(Debug)]
pub struct Image {
    pub path: PathBuf,
}
impl FromStr for Image {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(rest) = s.strip_prefix("file://") {
            let path = PathBuf::from(rest);
            Ok(Self { path })
        } else {
            anyhow::bail!("could not decode path from {s}")
        }
    }
}
impl Image {
    pub fn open(&self) -> Result<DynamicImage, ImageError> {
        image::open(&self.path)
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

// In practice, we only plan to implement the `Rectangle` region type in our server, otherwise I would have made this an enum as well.
#[derive(Debug, Clone)]
pub struct Region {
    pub top: f64,
    pub left: f64,
    pub height: f64,
    pub width: f64,
}
impl FromStr for Region {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let [top, left, height, width]: [f64; 4] = s
            .split(|c| c == ',' || c == '\t')
            .map(|n| f64::from_str(n))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|v| anyhow::anyhow!("{v:?} could not be coerced into a [f64; 4]"))?;
        Ok(Self {
            top,
            left,
            height,
            width,
        })
    }
}
impl Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            top,
            left,
            height,
            width,
        } = self;
        write!(f, "\"{top:.3},{left:.3},{height:.3},{width:.3}\"")
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
