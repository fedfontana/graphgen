mod scraper;
use std::error::Error;

use clap::Parser;

use crate::scraper::WikipediaScraper;
#[derive(Parser)]
struct Args {
    url: String,
    #[clap(short, long, default_value_t = 5, value_parser=clap::value_parser!(u64).range(1..))]
    depth: u64,
    #[clap(short, long, default_value_t = 1, value_parser=clap::value_parser!(u64).range(1..))]
    threads: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut scraper = WikipediaScraper::new(&args.url, args.depth);
    scraper.scrape()?;

    scraper.links().for_each(|link| println!("{:?}", link));
    // scraper.pages().for_each(|node| println!("{:?}", node));

    Ok(())
}
