//extern crate sudoers_reader;


use common::{config::{self, Storage}, database::{read_json_config, save_json}, read_effective};
use tracing::error;

mod cli;
#[path = "../mod.rs"]
mod common;




fn main() -> Result<(), Box<dyn std::error::Error>>{
    read_effective(true)?;
    let settings = config::get_settings();
    let config = match settings.storage_method {
        config::StorageMethod::JSON => {
            Storage::JSON(read_json_config(&settings)?)
        }
        _ => {
            error!("Unsupported storage method");
            std::process::exit(1);
        }
    };
    read_effective(false)?;

    if cli::main(&config)? {
        match config {
            Storage::JSON(config) => {
                save_json(&settings, config)?;
                Ok(())
            }
        }
    } else {
        Ok(())
    }
}