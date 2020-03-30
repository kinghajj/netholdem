use std::error::Error;
use crate::cli::Args;

pub async fn run(args: Args) -> Result<(), Box<dyn Error>> {
    println!("Running client!");
    Ok(())
}
