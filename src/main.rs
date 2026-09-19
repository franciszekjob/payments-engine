use std::fs::File;
use std::io;

use payments_engine::run;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("missing input file")?;

    if args.next().is_some() {
        return Err("expected exactly one input file".into());
    }

    let file = File::open(path)?;

    run(file, io::stdout())?;

    Ok(())
}
