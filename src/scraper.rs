use crate::errors::ScraperError;
use crate::worker::Worker;

use std::{
    collections::{HashMap, HashSet},
    io::Write,
    sync::{Arc, Mutex},
};

pub type ID = u64;

//TODO add --remove-external-links flag to pass to get_completed_url
//TODO not all errors should stop the whole program

//TODO move logic to worker that also contains the thread_idx?
//TODO what happens if we add a --undirected flag that changes which links and nodes are added to the graph?

pub struct WikipediaScraper<'a> {
    url: &'a str,
    depth: u64,
    links: Arc<Mutex<HashSet<(ID, ID)>>>,
    pages: Arc<Mutex<HashMap<String, ID>>>,
    keywords: Option<Vec<String>>,
    num_threads: usize,
    undirected: bool,
}

impl<'a> WikipediaScraper<'a> {
    pub fn new(url: &'a str, depth: u64, num_threads: usize, keywords: Option<Vec<String>>, undirected: bool) -> WikipediaScraper<'a> {
        if depth == 0 {
            panic!("Depth must be greater than 0");
        }
        if num_threads == 0 {
            panic!("Number of threads must be greater than 0");
        }

        WikipediaScraper {
            url,
            depth,
            links: Default::default(),
            pages: Default::default(),
            keywords,
            num_threads,
            undirected,
        }
    }

    pub fn scrape(&mut self) -> Result<(), ScraperError> {
        let stopped_threads = Arc::new(Mutex::new(vec![false; self.num_threads]));
        let (tx, rx) = crossbeam_channel::unbounded::<(String, u64)>();

        tx.send((self.url.to_owned(), self.depth))?;

        let handles = (0..self.num_threads)
            .map(|thread_idx| {
                let links = self.links.clone();
                let pages = self.pages.clone();
                let keywords = self.keywords.clone();

                let tx = tx.clone();
                let rx = rx.clone();

                let stopped_threads = stopped_threads.clone();

                std::thread::spawn(move || {
                    let worker = Worker::new(thread_idx, links, pages, keywords);
                    worker.scrape(rx, tx, stopped_threads)
                })
            })
            .collect::<Vec<_>>();

        handles
            .into_iter()
            .for_each(|handle| handle.join().unwrap().unwrap());

        Ok(())
    }

    pub fn save_to_file(&self, output_file: impl AsRef<str>) -> Result<(), std::io::Error> {
        let edges_file_path = format!("{}_edges.csv", output_file.as_ref());
        let nodes_file_path = format!("{}_nodes.csv", output_file.as_ref());

        let mut edges_file = std::fs::File::create(edges_file_path)?;
        let mut nodes_file = std::fs::File::create(nodes_file_path)?;

        edges_file.write_all("source,target\n".as_bytes())?;
        nodes_file.write_all("node_id,url\n".as_bytes())?;

        let own_links = self.links.lock().unwrap();
        let own_pages = self.pages.lock().unwrap();

        if !self.undirected {
            for (url, id) in own_pages.iter() {
                nodes_file.write_all(format!("{},\"{}\"\n", id, url).as_bytes())?;
            }
            
            for (source, dest) in own_links.iter() {
                edges_file.write_all(format!("{},{}\n", source, dest).as_bytes())?;
            }
        } else {
            let mut visited_edges = HashSet::new();
            let mut visited_nodes_set = HashSet::new();
            let mut visited_nodes = HashMap::new();
            
            for (source, dest) in own_links.iter() {
                if own_links.contains(&(*dest, *source)) {
                    visited_edges.insert((source, dest));
                    visited_nodes_set.insert(source);
                    visited_nodes_set.insert(dest);
                }
            }

            for (url, id) in own_pages.iter() {
                if visited_nodes_set.contains(id) {
                    visited_nodes.insert(id, url);
                }
            }

            for (id, url) in visited_nodes.iter() {
                nodes_file.write_all(format!("{},\"{}\"\n", id, url).as_bytes())?;
            }
            
            for (source, dest) in visited_edges.iter() {
                edges_file.write_all(format!("{},{}\n", source, dest).as_bytes())?;
            }
        }
        Ok(())
    }
}