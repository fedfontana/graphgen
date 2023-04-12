mod scraper;
mod errors;

use std::{error::Error, path};

use clap::Parser;

use crate::scraper::WikipediaScraper;

/// Simple wikipedia scraper
#[derive(Parser)]
struct Args {

    /// Url to scrape
    url: String,

    /// Keywords to search for in the pages
    #[clap(short, long)]
    keywords: Option<Vec<String>>,

    /// Depth of the scrape
    #[clap(short, long, default_value_t = 5, value_parser=clap::value_parser!(u64).range(1..))]
    depth: u64,

    /// The first part of the name of the output files. The edges will be saved to <output-file>_edges.csv and the nodes will be saved to <output-file>_nodes.csv
    #[clap(short, long="output-file")]
    output_file: Option<String>,
}

//TODO add logging and progress bar

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if let Some(output_file_path) = &args.output_file {
        let edges_file_path = format!("{}_edges.csv", output_file_path);
        if path::Path::new(&edges_file_path).exists() {
            return Err(format!("File {edges_file_path} already exists. Delete it and run the program again if you want to use that path.").into());
        }
        
        let nodes_file_path = format!("{}_nodes.csv", output_file_path);
        if path::Path::new(&nodes_file_path).exists() {
            return Err(format!("File {nodes_file_path} already exists. Delete it and run the program again if you want to use that path.").into());
        }
    }

    let mut scraper = WikipediaScraper::new(&args.url, args.depth, args.keywords);
    scraper.scrape()?;

    
    if let Some(output_file_path) = &args.output_file {
        scraper.save_to_file(output_file_path)?;
    } else {
        //TODO should this be logged?
        //TODO a single write
        // println!("source,target");
        // scraper.links().for_each(|link| println!("{:?}", link));
        // scraper.pages().for_each(|node| println!("{:?}", node));
    }

    Ok(())
}
