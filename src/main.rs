mod scraper;
use std::error::Error;

use crate::scraper::WikipediaScraper;

fn main() -> Result<(), Box<dyn Error>> {

    let mut scraper = WikipediaScraper::new(
        "https://en.wikipedia.org/wiki/Crocodile", 
        3
    );
    scraper.scrape()?;
    
    scraper.links().for_each(|link| println!("{:?}", link));
    // scraper.pages().for_each(|node| println!("{:?}", node));

    Ok(())
}
