mod scraper;
use std::{error::Error, path};

use clap::Parser;

use crate::scraper::WikipediaScraper;

/// Simple wikipedia scraper
#[derive(Parser)]
struct Args {

    /// Url to scrape
    url: String,

    /// Depth of the scrape
    #[clap(short, long, default_value_t = 5, value_parser=clap::value_parser!(u64).range(1..))]
    depth: u64,

    /// Number of threads to use for scraping
    #[clap(short, long, default_value_t = 1, value_parser=clap::value_parser!(u64).range(1..))]
    threads: u64,

    /// Saves the scraped content to a file in csv format
    #[clap(short, long="output-file")]
    output_file: Option<String>,
}

//TODO fix error: sometimes there are commas in the url, escape them so that they don't break the out csv
//TODO add logging and progress bar

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if let Some(output_file_path) = &args.output_file {
        let out_file_path = path::Path::new(output_file_path);
        if out_file_path.exists() {
            return Err("File already exists. Delete it and run the program again if you want to use that path.".into());
        }
    }

    let mut scraper = WikipediaScraper::new(&args.url, args.depth);
    scraper.scrape()?;

    
    if let Some(output_file_path) = &args.output_file {
        scraper.save_to_file(output_file_path)?;
    } else {
        //TODO should this be logged?
        
        //TODO a single write
        println!("source,target");
        scraper.links().for_each(|link| println!("{:?}", link));
        // scraper.pages().for_each(|node| println!("{:?}", node));
    }

    Ok(())
}
