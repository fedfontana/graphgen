use crossbeam_channel::Receiver;

use crate::errors::ScraperError;
use crate::worker::Worker;

use std::{
    collections::{HashMap, HashSet},
    io::Write,
    sync::{Arc, Mutex},
};

pub type ID = u64;

//TODO not all errors should stop the whole program

pub struct WikipediaScraper<'a> {
    url: &'a str,
    depth: u64,
    links: Arc<Mutex<HashSet<(ID, ID)>>>,
    pages: Arc<Mutex<HashMap<String, ID>>>,
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
            eprintln!("[WARN] Depth must be greater than 0. Setting it to 1.");
        }
        if num_threads == 0 {
            eprintln!("[WARN] Number of threads must be greater than 0. Setting it to 1.");
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
        self.links.lock().unwrap().len()
    }

    pub fn num_pages(&self) -> usize {
        self.pages.lock().unwrap().len()
    }

    pub fn worker(
        &self,
        thread_idx: usize,
        stopped_threads: Arc<Mutex<Vec<bool>>>,
        rx: Receiver<(String, u64)>,
        tx: crossbeam_channel::Sender<(String, u64)>,
    ) -> Worker {
        //TODO maybe change to Arc<RwLock>?
        let stopped_threads = stopped_threads.clone();

        Worker::new(
            thread_idx,
            self.links.clone(),
            self.pages.clone(),
            self.keywords.clone(),
            rx,
            tx,
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
                let stopped_threads = stopped_threads.clone();
                let worker = self.worker(thread_idx, stopped_threads, rx.clone(), tx.clone());
                std::thread::spawn(move || worker.scrape())
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
            //TODO: FIX: this somehow generates undirected graphs that are not completely connected
            //TODO: FIX: the script doesn't check that the keywords are present in the last pages linked at the edges of the graph
            let mut visited_edges = HashSet::new();
            let mut visited_pages_set = HashSet::new();
            let mut visited_pages = HashMap::new();

            for (source, dest) in own_links.iter() {
                // If the edge (a,b) has already been inserted, then do not check for (b,a)
                // since we do not want to add duplicate edges
                //TODO only one of these calls to `contains` is necessary
                if visited_edges.contains(&(source, dest))
                    || visited_edges.contains(&(dest, source))
                {
                    continue;
                }
                if own_links.contains(&(*dest, *source)) {
                    visited_edges.insert((source, dest));
                    visited_pages_set.insert(source);
                    visited_pages_set.insert(dest);
                }
            }

            for (url, id) in own_pages.iter() {
                if visited_pages_set.contains(id) {
                    visited_pages.insert(id, url);
                }
            }

            for (id, url) in visited_pages.iter() {
                nodes_file.write_all(format!("{},\"{}\"\n", id, url).as_bytes())?;
            }

            for (source, dest) in visited_edges.iter() {
                edges_file.write_all(format!("{},{}\n", source, dest).as_bytes())?;
            }
        }
        Ok(())
    }
}
