use crate::errors::ScraperError;
use crate::worker::Worker;

use crossbeam_channel::{Receiver, Sender};
use flurry::{HashMap, HashSet};
use std::{
    io::Write,
    sync::{Arc, Mutex},
};

pub type ID = u64;

pub struct WikipediaScraper<'a> {
    url: &'a str,
    depth: u64,
    links: Arc<HashSet<(ID, ID)>>,
    pages: Arc<HashMap<String, ID>>,
    keywords: Option<Vec<String>>,
    num_threads: usize,
    undirected: bool,
    keep_external_links: bool,
}

impl<'a> WikipediaScraper<'a> {
    pub fn new(
        url: &'a str,
        depth: u64,
        num_threads: usize,
        keywords: Option<Vec<String>>,
        undirected: bool,
        keep_external_links: bool,
    ) -> WikipediaScraper<'a> {
        if depth == 0 {
            println!("[WARN] Depth must be greater than 0. Setting it to 1.");
        }
        if num_threads == 0 {
            println!("[WARN] Number of threads must be greater than 0. Setting it to 1.");
        }

        WikipediaScraper {
            url,
            depth: depth.max(1),
            links: Default::default(),
            pages: Default::default(),
            keywords,
            num_threads: num_threads.max(1),
            undirected,
            keep_external_links,
        }
    }

    pub fn num_links(&self) -> usize {
        self.links.len()
    }

    pub fn num_pages(&self) -> usize {
        self.pages.len()
    }

    pub fn worker(
        &self,
        id: usize,
        tx: Sender<(String, u64)>,
        rx: Receiver<(String, u64)>,
        stopped_threads: Arc<Mutex<Vec<bool>>>,
    ) -> Worker {
        Worker::new(
            id,
            self.links.clone(),
            self.pages.clone(),
            self.keywords.clone(),
            tx,
            rx,
            stopped_threads,
            self.keep_external_links,
        )
    }

    pub fn scrape(&mut self) -> Result<(), ScraperError> {
        let stopped_threads = Arc::new(Mutex::new(vec![false; self.num_threads]));
        let (tx, rx) = crossbeam_channel::unbounded::<(String, u64)>();

        tx.send((self.url.to_owned(), self.depth))?;

        let handles = (0..self.num_threads)
            .map(|thread_idx| {
                let tx = tx.clone();
                let rx = rx.clone();
                let stopped_threads = stopped_threads.clone();

                let worker = self.worker(thread_idx, tx, rx, stopped_threads);
                std::thread::spawn(move || {
                    worker.scrape()
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

        let links_guard = self.links.guard();
        let pages_guard = self.pages.guard();

        if !self.undirected {
            for (url, id) in self.pages.iter(&pages_guard) {
                nodes_file.write_all(format!("{},\"{}\"\n", id, url).as_bytes())?;
            }

            for (source, dest) in self.links.iter(&links_guard) {
                edges_file.write_all(format!("{},{}\n", source, dest).as_bytes())?;
            }
        } else {
            let mut visited_edges = std::collections::HashSet::new();
            let mut visited_nodes_set = std::collections::HashSet::new();
            let mut visited_nodes = std::collections::HashMap::new();

            for (source, dest) in self.links.iter(&links_guard) {
                if self.links.contains(&(*dest, *source), &links_guard) {
                    visited_edges.insert((source, dest));
                    visited_nodes_set.insert(source);
                    visited_nodes_set.insert(dest);
                }
            }

            for (url, id) in self.pages.iter(&pages_guard) {
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
