
#![windows_subsystem = "windows"]

pub mod cli_main;

mod fingerprinting {
    pub mod communication;
    pub mod algorithm;
    pub mod signature_format;
    mod user_agent;
    mod hanning;
}

mod core {
    pub mod microphone_thread;
    pub mod processing_thread;
    pub mod http_thread;
    pub mod thread_messages;
}

mod audio_controllers {
    pub mod audio_backend;
    pub mod cpal;
    #[cfg(feature = "pulse")]
    pub mod pulseaudio;
}

#[cfg(feature = "gui")]
mod gui {
    pub mod main_window;
    mod song_history_interface;
    pub mod preferences;
}

mod utils {
    pub mod csv_song_history;
    pub mod internationalization;

    #[cfg(feature = "gui")]
    pub mod filesystem_operations;

    #[cfg(feature = "ffmpeg")]
    pub mod ffmpeg_wrapper;

    #[cfg(feature = "mpris")]
    pub mod mpris_player;
}

use crate::fingerprinting::algorithm::SignatureGenerator;
use crate::fingerprinting::signature_format::DecodedSignature;
use crate::fingerprinting::communication::recognize_song_from_signature;

use crate::utils::internationalization::setup_internationalization;
#[cfg(feature = "gui")]
use crate::gui::main_window::gui_main;
use crate::cli_main::{cli_main, CLIParameters, CLIOutputType};

use std::error::Error;
use gettextrs::gettext;
use clap::{App, Arg};

macro_rules! base_app {
    () => {
    App::new("SongRec")
        .version("0.4.3")
        .about(gettext("An open-source Shazam client for Linux, written in Rust.").as_str())
        .subcommand(
            App::new("listen")
                .about(gettext("Run as a command-line program listening the microphone and printing recognized songs to stdout, exposing current song info via MPRIS").as_str())
                .arg(
                    Arg::with_name("audio-device")
                        .short("d")
                        .long("audio-device")
                        .takes_value(true)
                        .help(gettext("Specify the audio device to use").as_str())
                )
                .arg(
                    Arg::with_name("json")
                        .short("j")
                        .long("json")
                        .conflicts_with("csv")
                        .help(gettext("Enable printing full song info in JSON").as_str())
                )
                .arg(
                    Arg::with_name("csv")
                        .short("c")
                        .long("csv")
                        .help(gettext("Enable printing full song info in the CSV format").as_str())
                )
                .arg(
                    Arg::with_name("disable-mpris")
                        .long("disable-mpris")
                        .help(gettext("Disable MPRIS support").as_str())
                )
        )
        .subcommand(
            App::new("recognize")
                .about(gettext("Recognize one song from a sound file or microphone and print its info.").as_str())
                .arg(
                    Arg::with_name("audio-device")
                        .short("d")
                        .long("audio-device")
                        .takes_value(true)
                        .help(gettext("Specify the audio device to use").as_str())
                )
                .arg(
                    Arg::with_name("json")
                        .short("j")
                        .long("json")
                        .conflicts_with("csv")
                        .help(gettext("Enable printing full song info in JSON").as_str())
                )
                .arg(
                    Arg::with_name("csv")
                        .short("c")
                        .long("csv")
                        .help(gettext("Enable printing full song info in the CSV format").as_str())
                )
                .arg(
                    Arg::with_name("input_file")
                        .required(false)
                        .help(gettext("Recognize a file instead of using mic input").as_str())
                )
        )
        .subcommand(
            App::new("audio-file-to-recognized-song")
                .about(gettext("Generate a Shazam fingerprint from a sound file, perform song recognition towards Shazam's servers and print obtained information to the standard output.").as_str())
                .arg(
                    Arg::with_name("input_file")
                        .required(true)
                        .help(gettext("The audio file to recognize.").as_str())
                )
        )
        .subcommand(
            App::new("microphone-to-recognized-song")
                .about(gettext("Recognize a currently playing song using the microphone and print obtained information to the standard output").as_str())
                .arg(
                    Arg::with_name("audio-device")
                        .short("d")
                        .long("audio-device")
                        .takes_value(true)
                        .help(gettext("Specify the audio device to use").as_str())
                )
        )
        .subcommand(
            App::new("audio-file-to-fingerprint")
                .about(gettext("Generate a Shazam fingerprint from a sound file, and print it to the standard output.").as_str())
                .arg(
                    Arg::with_name("input_file")
                        .required(true)
                        .help(gettext("The .WAV or .MP3 file to generate an audio fingerprint for.").as_str())
                )
        )
        .subcommand(
            App::new("fingerprint-to-recognized-song")
                .about(gettext("Take a data-URI Shazam fingerprint, perform song recognition towards Shazam's servers and print obtained information to the standard output.").as_str())
                .arg(
                    Arg::with_name("fingerprint")
                        .required(true)
                        .help(gettext("The data-URI Shazam fingerprint to recognize.").as_str())
                )
        )
    };
}

#[cfg(feature="gui")]
macro_rules! gui_app {
    () => {
        base_app!()
        .subcommand(
            App::new("gui")
                .about(gettext("The default action. Display a GUI.").as_str())
                .arg(
                    Arg::with_name("input_file")
                        .required(false)
                        .help(gettext("An optional audio file to recognize on the launch of the application.").as_str())
                )
                .arg(
                    Arg::with_name("disable-mpris")
                        .long("disable-mpris")
                        .help(gettext("Disable MPRIS support").as_str())
                )
        )
        .subcommand(
            App::new("gui-norecording")
                .about(gettext("Launch the GUI, but don't recognize audio through the microphone as soon as it is launched (rather than expecting the user to click on a button).").as_str())
                .arg(
                    Arg::with_name("input_file")
                        .required(false)
                        .help(gettext("An optional audio file to recognize on the launch of the application.").as_str())
                )
                .arg(
                    Arg::with_name("disable-mpris")
                        .long("disable-mpris")
                        .help(gettext("Disable MPRIS support").as_str())
                )
        )
    };
}

#[cfg(feature="gui")]
macro_rules! app {
    () => { gui_app!() };
}

#[cfg(not(feature="gui"))]
macro_rules! app {
    () => { base_app!() };
}

fn main() -> Result<(), Box<dyn Error>> {

    // Set up the translation/internationalization part

    setup_internationalization();

    // Collect the program arguments
    let args = app!().get_matches();
    
    match args.subcommand_name() {
        Some("audio-file-to-recognized-song") => {            
            let subcommand_args = args.subcommand_matches("audio-file-to-recognized-song").unwrap();
            
            let input_file_string = subcommand_args.value_of("input_file").unwrap();
            
            println!("{}", serde_json::to_string_pretty(&recognize_song_from_signature(&SignatureGenerator::make_signature_from_file(input_file_string)?)?)?);
        },
        Some("audio-file-to-fingerprint") => {
            let subcommand_args = args.subcommand_matches("audio-file-to-fingerprint").unwrap();
            
            let input_file_string = subcommand_args.value_of("input_file").unwrap();
            
            println!("{}", SignatureGenerator::make_signature_from_file(input_file_string)?.encode_to_uri()?);
        },
        Some("fingerprint-to-recognized-song") => {
            let subcommand_args = args.subcommand_matches("fingerprint-to-recognized-song").unwrap();
            
            let fingerprint_string = subcommand_args.value_of("fingerprint").unwrap();
            
            println!("{}", serde_json::to_string_pretty(&recognize_song_from_signature(&DecodedSignature::decode_from_uri(fingerprint_string)?)?)?);
        },
        Some("listen") => {
            let subcommand_args = args.subcommand_matches("listen").unwrap();
            let audio_device = subcommand_args.value_of("audio-device").map(str::to_string);
            let enable_mpris = !subcommand_args.is_present("disable-mpris");
            let enable_json = subcommand_args.is_present("json");
            let enable_csv = subcommand_args.is_present("csv");

            cli_main(CLIParameters {
                enable_mpris,
                recognize_once: false,
                audio_device,
                input_file: None,
                output_type: if enable_json {
                    CLIOutputType::JSON
                }
                else if enable_csv {
                    CLIOutputType::CSV
                }
                else {
                    CLIOutputType::SongName
                }
            })?;
        },
        Some("recognize") => {
            let subcommand_args = args.subcommand_matches("recognize").unwrap();
            let audio_device = subcommand_args.value_of("audio-device").map(str::to_string);
            let input_file = subcommand_args.value_of("input_file").map(str::to_string);
            let enable_json = subcommand_args.is_present("json");
            let enable_csv = subcommand_args.is_present("csv");

            cli_main(CLIParameters {
                enable_mpris: false,
                recognize_once: true,
                audio_device,
                input_file,

                output_type: if enable_json {
                    CLIOutputType::JSON
                }
                else if enable_csv {
                    CLIOutputType::CSV
                }
                else {
                    CLIOutputType::SongName
                }
            })?;
        },
        Some("microphone-to-recognized-song") => {
            let subcommand_args = args.subcommand_matches("microphone-to-recognized-song").unwrap();
            let audio_device = subcommand_args.value_of("audio-device").map(str::to_string);

            cli_main(CLIParameters {
                enable_mpris: false,
                recognize_once: true,
                audio_device,
                input_file: None,
                output_type: CLIOutputType::JSON
            })?;
        },
        #[cfg(feature="gui")]
        Some("gui-norecording") => {
            let subcommand_args = args.subcommand_matches("gui-norecording").unwrap();

            gui_main(false,
                 subcommand_args.value_of("input_file"),
                 !subcommand_args.is_present("disable-mpris"),
            )?;
        },
        #[cfg(feature="gui")]
        Some("gui") | None => {
            if let Some(subcommand_args) = args.subcommand_matches("gui") {
                gui_main(true,
                     subcommand_args.value_of("input_file"),
                     !subcommand_args.is_present("disable-mpris"),
                )?;
            }
            else {
                gui_main(true, None, true)?;
            }
        },
        #[cfg(not(feature="gui"))]
        None => {
            cli_main(CLIParameters {
                enable_mpris: true,
                recognize_once: false,
                audio_device: None,
                input_file: None,
                output_type: CLIOutputType::SongName
            })?;
        },
        _ => unreachable!()
    }
    
    Ok(())
    
}
