use std::fs::File;
use std::io;

use payments_engine::run;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("missing input file")?;

    let file = File::open(path)?;

    run(file, io::stdout())?;

    Ok(())
}
