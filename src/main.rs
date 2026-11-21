use std::{
    env, fs,
    io::{self, Read},
    process::ExitCode,
};

use sw_ga144::{Ga144, Image, NodeId};

const USAGE: &str = "Usage: ga144-sim TICKS NODE:ENTRY:LISTING [NODE:ENTRY:LISTING ...]\n\
Runs up to TICKS scheduler ticks. NODE is yxx, ENTRY is decimal, and LISTING is\n\
an assembler output file or - for standard input.";

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let ticks = match args.next().as_deref() {
        Some("--help" | "-h") => {
            println!("{USAGE}");
            return Ok(());
        }
        Some("--version" | "-V") => {
            println!("ga144-sim {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(ticks) => ticks.parse::<usize>()?,
        None => return Err(USAGE.into()),
    };
    let mut chip = Ga144::new();
    let mut loaded = Vec::new();
    let mut used_stdin = false;
    for spec in args {
        let mut fields = spec.splitn(3, ':');
        let number = fields.next().ok_or(USAGE)?.parse()?;
        let node =
            NodeId::from_number(number).ok_or("node must be within 000..717 (18 columns)")?;
        if loaded.contains(&node) {
            return Err(format!("node {node} was specified twice").into());
        }
        let entry = fields.next().ok_or(USAGE)?.parse()?;
        let listing = fields.next().ok_or(USAGE)?;
        let text = if listing == "-" {
            if used_stdin {
                return Err("standard input can supply only one listing".into());
            }
            used_stdin = true;
            let mut text = String::new();
            io::stdin().read_to_string(&mut text)?;
            text
        } else {
            fs::read_to_string(listing)?
        };
        let image: Image = text
            .parse()
            .map_err(|error| format!("{listing}: {error}"))?;
        chip.load_ram(node, &image);
        chip.start(node, entry)?;
        loaded.push(node);
    }

    if loaded.is_empty() {
        return Err(USAGE.into());
    }
    for _ in 0..ticks {
        if chip.tick()? == 0 {
            break;
        }
    }
    for node in loaded {
        let state = chip.inspect(node);
        let waiting = state
            .waiting_address
            .map_or_else(|| "-".to_owned(), |value| format!("x{value:03x}"));
        println!(
            "{node} P=x{:03x} slot={} T={} S={} R={} wait={waiting}",
            state.p, state.slot, state.t, state.s, state.r
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ga144-sim: {error}");
            ExitCode::FAILURE
        }
    }
}
