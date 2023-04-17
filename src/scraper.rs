use crate::errors::ScraperError;
use crossbeam_channel::select;
use reqwest::blocking::get;
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    sync::{Arc, Mutex},
    vec, thread, time::Duration,
};

type ID = u64;

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

fn get_complete_url(url: &str) -> Option<String> {
    // All of the internal links start with a slash
    if !url.starts_with('/') {
        return None;
        // return Some(url.to_owned());
    }

    if url.contains(':') {
        return None;
    }

    if url.starts_with("/w") && !url.starts_with("/wiki") {
        return None;
    }

    if let Some((url, _tag)) = url.split_once("#") {
        return Some("https://en.wikipedia.org".to_owned() + url);
    }

    return Some("https://en.wikipedia.org".to_owned() + url);
}

impl<'a> WikipediaScraper<'a> {
    pub fn new(url: &'a str, depth: u64, num_threads: usize, keywords: Option<Vec<String>>) -> WikipediaScraper<'a> {
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
            undirected: true,
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

                let nt = self.num_threads;
                let stopped_threads = stopped_threads.clone();

                std::thread::spawn(move || {
                    loop {
                        select! {
                            recv(rx) -> msg => {
                                stopped_threads.lock().unwrap().iter_mut().for_each(|x| *x = false);

                                if let Ok((url, depth)) = msg {
                                    eprintln!("[Thread {}] Scraping {} with depth: {}", thread_idx, url, depth);
                                    let out_links = WikipediaScraper::scrape_with_depth(
                                        &links,
                                        &pages,
                                        url,
                                        keywords.as_ref(),
                                    )?;

                                    // If depth were to be equal to 1, then scrape_with_depth with depth = depth-1 = 0
                                    // would return an empty vector, so just dont call the function
                                    if depth > 1 {
                                        out_links.into_iter().for_each(|link| {
                                            tx.send((link, depth - 1)).unwrap();
                                        });
                                    }
                                }
                            },
                            default => {
                                let mut locked_stopped_threads = stopped_threads.lock().unwrap();
                                locked_stopped_threads[thread_idx] = true;
                                
                                let stopped_threads_count = locked_stopped_threads.iter().filter(|x| **x).count();
                                eprintln!("[Thread {}] Nothing to do. Stuck in here with other {} threads", thread_idx, stopped_threads_count);
                                drop(locked_stopped_threads);

                                if stopped_threads_count == nt {
                                    assert!(rx.len() == 0, "Expected rx to be empty, found {} links", rx.len());
                                    eprintln!("[Thread {}] All threads have nothing to do. Stopping the current one", thread_idx);
                                    break;
                                } else {
                                    eprintln!("[Thread {}] Going to sleep for 500ms", thread_idx);
                                    thread::sleep(Duration::from_millis(500));
                                }
                            }
                        }
                    }
                    Result::<_, ScraperError>::Ok(())
                })
            })
            .collect::<Vec<_>>();

        handles
            .into_iter()
            .for_each(|handle| handle.join().unwrap().unwrap());

        Ok(())
    }

    fn scrape_with_depth(
        links: &Arc<Mutex<HashSet<(ID, ID)>>>,
        pages: &Arc<Mutex<HashMap<String, ID>>>,
        start_url: impl AsRef<str>,
        keywords: Option<&Vec<String>>,
    ) -> Result<Vec<String>, ScraperError> {

        let Some(page_content)= WikipediaScraper::get_page_content(start_url.as_ref(), keywords)? else {
            eprintln!("Skipping {}", start_url.as_ref());
            return Ok(vec![]);
        };


        let anchor_list = get_anchor_list(&page_content)?;

        if anchor_list.is_empty() {
            return Ok(vec![]);
        }

        let mut own_pages = pages.lock().unwrap();
        let mut own_links = links.lock().unwrap();

        // If the page has already been visited, just add the links to the links set by recovering its id
        // else generate a new id and add it to the pages before proceeding to process the links
        let start_url_id = if let Some(start_url_id) = own_pages.get(start_url.as_ref()) {
            *start_url_id
        } else {
            let new_id = own_pages.len() as ID;
            own_pages.insert(start_url.as_ref().to_string(), new_id);
            new_id
        };

        let mut out_links = Vec::new();

        for anchor in anchor_list {
            // If the link has already been visited, just add the current link to the links set
            if let Some(anchor_id) = own_pages.get(&anchor) {
                own_links.insert((start_url_id, *anchor_id));
            } else {
                // Else generate the anchor id and add it to the pages
                let anchor_id = own_pages.len() as ID;
                assert!(
                    own_pages.insert(anchor.clone(), anchor_id).is_none(),
                    "Should not be adding a page that already exists"
                );

                // Add the link
                assert!(
                    own_links.insert((start_url_id, anchor_id)),
                    "Should not be adding a link that already exists"
                );

                if anchor.starts_with("https://en.wikipedia.org/wiki/") {
                    // And then scrape that page recursively
                    out_links.push(anchor);
                }
            }
        }

        Ok(out_links)
    }

    pub fn save_to_file(&self, output_file: impl AsRef<str>) -> Result<(), std::io::Error> {
        let edges_file_path = format!("{}_edges.csv", output_file.as_ref());
        let nodes_file_path = format!("{}_nodes.csv", output_file.as_ref());

        let mut edges_file = std::fs::File::create(edges_file_path)?;
        let mut nodes_file = std::fs::File::create(nodes_file_path)?;

        edges_file.write_all("source,target\n".as_bytes())?;
        nodes_file.write_all("node_id,url\n".as_bytes())?;

        if !self.undirected {
            for (url, id) in self.pages.lock().unwrap().iter() {
                nodes_file.write_all(format!("{},\"{}\"\n", id, url).as_bytes())?;
            }
            
            for (source, dest) in self.links.lock().unwrap().iter() {
                edges_file.write_all(format!("{},{}\n", source, dest).as_bytes())?;
            }
        } else {
            let own_links = self.links.lock().unwrap();
            let own_nodes = self.pages.lock().unwrap();

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

            for (url, id) in own_nodes.iter() {
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

    fn get_page_content(
        url: impl AsRef<str>,
        keywords: Option<&Vec<String>>,
    ) -> Result<Option<String>, ScraperError> {
        let mut resp = get(url.as_ref())?;
        let mut content = String::new();
        resp.read_to_string(&mut content)?;

        if let Some(keywords) = keywords {
            let lower_content = content.to_lowercase();
            if keywords
                .iter()
                .any(|keyword| lower_content.contains(keyword.to_lowercase().as_str()))
            {
                return Ok(Some(content));
            } else {
                return Ok(None);
            }
        }
        Ok(Some(content))
    }
}

fn get_anchor_list(page_content: &str) -> Result<Vec<String>, ScraperError> {
    let document = scraper::Html::parse_document(page_content);

    //TODO use errorkind and just log if we can't find the content (right now it is stopping the program)
    let content_selector =
        scraper::Selector::parse("#bodyContent").expect("Static selector should be valid");
    let content = document.select(&content_selector).next();
    // .map_or_else(|| Err("No content found"), |content| Ok(content))?;
    if let Some(content) = content {
        let anchor_selector =
            scraper::Selector::parse("a").expect("Static selector should be valid");

        let anchors = content.select(&anchor_selector);

        let mut anchor_list = Vec::new();
        for anchor in anchors {
            if let Some(href) = anchor.value().attr("href") {
                if let Some(url) = get_complete_url(href) {
                    anchor_list.push(url);
                }
            }
        }
        Ok(anchor_list)
    } else {
        eprintln!("No content found for a page");
        Ok(Vec::new())
    }
}
