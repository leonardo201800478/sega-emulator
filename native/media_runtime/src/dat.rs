use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion, escape};
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Version of the No-Intro SMS DAT currently versioned in this repository.
pub const SMS_DAT_VERSION: &str = "20261013-064924";

const SMS_DAT_XML: &str = include_str!(
    "../../../dats/Sega - Master System - Mark III (Parent-Clone) (20261013-064924).dat"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatRomEntry {
    pub name: String,
    pub size_bytes: usize,
    pub crc32: u32,
    pub md5: Option<String>,
    pub sha1: String,
    pub status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatGameEntry {
    pub name: String,
    pub roms: Vec<DatRomEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDat {
    pub name: Option<String>,
    pub version: Option<String>,
    pub entries: Vec<DatGameEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DatParseError {
    Xml(String),
    MissingAttribute {
        element: &'static str,
        attribute: &'static str,
    },
    InvalidInteger {
        field: &'static str,
        value: String,
    },
    InvalidHex {
        field: &'static str,
        value: String,
    },
    RomOutsideGame,
}

impl Display for DatParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Xml(error) => write!(formatter, "invalid No-Intro XML: {error}"),
            Self::MissingAttribute { element, attribute } => {
                write!(
                    formatter,
                    "DAT {element} is missing required attribute {attribute}"
                )
            }
            Self::InvalidInteger { field, value } => {
                write!(formatter, "invalid DAT integer {field}={value}")
            }
            Self::InvalidHex { field, value } => {
                write!(formatter, "invalid DAT hexadecimal {field}={value}")
            }
            Self::RomOutsideGame => write!(formatter, "DAT ROM entry appears outside a game"),
        }
    }
}

impl Error for DatParseError {}

impl NoIntroDat {
    /// Parse a No-Intro XML DAT without relying on file names as ROM identity.
    pub fn parse(xml: &str) -> Result<Self, DatParseError> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut name = None;
        let mut version = None;
        let mut header_field = None;
        let mut in_header = false;
        let mut current_game: Option<DatGameEntry> = None;
        let mut current_rom: Option<DatRomEntry> = None;
        let mut entries = Vec::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(element)) => match element.name().as_ref() {
                    b"header" => in_header = true,
                    b"name" if in_header => header_field = Some("name"),
                    b"version" if in_header => header_field = Some("version"),
                    b"game" => {
                        let game_name =
                            required_attribute(&element, reader.decoder(), "game", "name")?;
                        current_game = Some(DatGameEntry {
                            name: game_name,
                            roms: Vec::new(),
                        });
                    }
                    b"rom" => {
                        current_rom = Some(parse_rom(&element, reader.decoder())?);
                    }
                    _ => {}
                },
                Ok(Event::Empty(element)) => match element.name().as_ref() {
                    b"rom" => {
                        let rom = parse_rom(&element, reader.decoder())?;
                        current_game
                            .as_mut()
                            .ok_or(DatParseError::RomOutsideGame)?
                            .roms
                            .push(rom);
                    }
                    _ => {}
                },
                Ok(Event::Text(text)) => {
                    if let Some(field) = header_field {
                        let decoded = text
                            .decode()
                            .map_err(|error| DatParseError::Xml(error.to_string()))?;
                        let unescaped = escape::unescape(&decoded)
                            .map_err(|error| DatParseError::Xml(error.to_string()))?
                            .into_owned();
                        match field {
                            "name" => name = Some(unescaped),
                            "version" => version = Some(unescaped),
                            _ => {}
                        }
                    }
                }
                Ok(Event::End(element)) => match element.name().as_ref() {
                    b"header" => in_header = false,
                    b"name" | b"version" => header_field = None,
                    b"rom" => {
                        if let Some(rom) = current_rom.take() {
                            current_game
                                .as_mut()
                                .ok_or(DatParseError::RomOutsideGame)?
                                .roms
                                .push(rom);
                        }
                    }
                    b"game" => {
                        if let Some(game) = current_game.take() {
                            entries.push(game);
                        }
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(error) => return Err(DatParseError::Xml(error.to_string())),
            }
        }

        Ok(Self {
            name,
            version,
            entries,
        })
    }

    /// Parse the exact SMS DAT version committed under the repository's dats directory.
    pub fn sms_default() -> Result<Self, DatParseError> {
        Self::parse(SMS_DAT_XML)
    }

    /// Find a ROM by SHA-1, returning its game and ROM metadata.
    pub fn find_sha1(&self, sha1: &str) -> Option<(&DatGameEntry, &DatRomEntry)> {
        self.entries.iter().find_map(|game| {
            game.roms
                .iter()
                .find(|rom| rom.sha1.eq_ignore_ascii_case(sha1))
                .map(|rom| (game, rom))
        })
    }
}

fn required_attribute(
    element: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    element_name: &'static str,
    attribute_name: &'static str,
) -> Result<String, DatParseError> {
    optional_attribute(element, decoder, attribute_name)?.ok_or(DatParseError::MissingAttribute {
        element: element_name,
        attribute: attribute_name,
    })
}

fn optional_attribute(
    element: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    attribute_name: &'static str,
) -> Result<Option<String>, DatParseError> {
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|error| DatParseError::Xml(error.to_string()))?;
        if attribute.key.as_ref() == attribute_name.as_bytes() {
            let value = attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
                .map_err(|error| DatParseError::Xml(error.to_string()))?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

fn parse_rom(
    element: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) -> Result<DatRomEntry, DatParseError> {
    let name = required_attribute(element, decoder, "rom", "name")?;
    let size_text = required_attribute(element, decoder, "rom", "size")?;
    let crc_text = required_attribute(element, decoder, "rom", "crc")?;
    let sha1 = required_attribute(element, decoder, "rom", "sha1")?.to_ascii_lowercase();
    let md5 = optional_attribute(element, decoder, "md5")?;
    let status = optional_attribute(element, decoder, "status")?;

    let size_bytes = size_text
        .parse::<usize>()
        .map_err(|_| DatParseError::InvalidInteger {
            field: "size",
            value: size_text,
        })?;
    let crc32 = u32::from_str_radix(&crc_text, 16).map_err(|_| DatParseError::InvalidHex {
        field: "crc",
        value: crc_text,
    })?;
    if sha1.len() != 40 || !sha1.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(DatParseError::InvalidHex {
            field: "sha1",
            value: sha1,
        });
    }

    Ok(DatRomEntry {
        name,
        size_bytes,
        crc32,
        md5,
        sha1,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::NoIntroDat;

    #[test]
    fn parses_game_rom_hashes_and_header_version() {
        let xml = r#"
            <datafile>
              <header><name>Example</name><version>2026.01</version></header>
              <game name="Example Game">
                <rom name="example.sms" size="3" crc="352441C2"
                     md5="900150983cd24fb0d6963f7d28e17f72"
                     sha1="a9993e364706816aba3e25717850c26c9cd0d89d"
                     status="verified" />
              </game>
            </datafile>
        "#;

        let dat = NoIntroDat::parse(xml).unwrap();
        assert_eq!(dat.name.as_deref(), Some("Example"));
        assert_eq!(dat.version.as_deref(), Some("2026.01"));
        let (game, rom) = dat
            .find_sha1("A9993E364706816ABA3E25717850C26C9CD0D89D")
            .unwrap();
        assert_eq!(game.name, "Example Game");
        assert_eq!(rom.size_bytes, 3);
        assert_eq!(rom.crc32, 0x3524_41C2);
    }

    #[test]
    fn parses_repository_sms_dat_version() {
        let dat = NoIntroDat::sms_default().unwrap();
        assert_eq!(dat.version.as_deref(), Some(super::SMS_DAT_VERSION));
        assert!(dat.entries.len() > 500);
        assert!(
            dat.find_sha1("fbda0486b393708a89756bb57d116ad6007484e4")
                .is_some()
        );
    }
}
