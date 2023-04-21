use std::{
    io::Read,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crossbeam_channel::{select, Receiver, Sender};
use flurry::{HashMap, HashSet};
use reqwest::blocking::get;

use crate::{errors::ScraperError, scraper::ID};

pub struct Worker {
    id: usize,
    links: Arc<HashSet<(ID, ID)>>,
    pages: Arc<HashMap<String, ID>>,
    keywords: Option<Vec<String>>,
    tx: Sender<(String, u64)>,
    rx: Receiver<(String, u64)>,
    stopped_threads: Arc<Mutex<Vec<bool>>>,
    keep_external_links: bool,
}

impl Worker {
    pub fn new(
        id: usize,
        links: Arc<HashSet<(ID, ID)>>,
        pages: Arc<HashMap<String, ID>>,
        keywords: Option<Vec<String>>,
        tx: Sender<(String, u64)>,
        rx: Receiver<(String, u64)>,
        stopped_threads: Arc<Mutex<Vec<bool>>>,
        keep_external_links: bool,
    ) -> Worker {
        Worker {
            id,
            links,
            pages,
            keywords,
            tx,
            rx,
            stopped_threads,
            keep_external_links,
        }
    }

    pub fn scrape(&self) -> Result<(), ScraperError> {
        loop {
            select! {
                recv(self.rx) -> msg => {
                    self.stopped_threads.lock().unwrap().iter_mut().for_each(|x| *x = false);

                    if let Ok((url, depth)) = msg {
                        println!("[Thread {}] Scraping {} with depth: {}", self.id, url, depth);
                        let out_links = self.scrape_with_depth(url)?;

                        // If depth were to be equal to 1, then scrape_with_depth with depth = depth-1 = 0
                        // would return an empty vector, so just dont call the function
                        if depth > 1 {
                            out_links.into_iter().for_each(|link| {
                                self.tx.send((link, depth - 1)).unwrap();
                            });
                        }
                    }
                },
                default => {
                    let mut locked_stopped_threads = self.stopped_threads.lock().unwrap();
                    locked_stopped_threads[self.id] = true;

                    let stopped_threads_count = locked_stopped_threads.iter().filter(|x| **x).count();
                    println!("[Thread {}] Nothing to do. Stuck in here with other {} threads", self.id, stopped_threads_count);
                    let nt = locked_stopped_threads.len();
                    drop(locked_stopped_threads);

                    if stopped_threads_count == nt {
                        debug_assert!(self.rx.len() == 0, "Expected rx to be empty, found {} links", self.rx.len());
                        println!("[Thread {}] All threads have nothing to do. Stopping the current one", self.id);
                        break;
                    } else {
                        println!("[Thread {}] Going to sleep for 500ms", self.id);
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_page_content(
        &self,
        url: impl AsRef<str>,
    ) -> Result<Option<String>, ScraperError> {
        let mut resp = get(url.as_ref())?;
        let mut content = String::new();
        resp.read_to_string(&mut content)?;

        if let Some(keywords) = self.keywords.as_ref() {
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

    pub fn get_anchor_list(
        &self,
        page_content: &str,
    ) -> Result<Vec<String>, ScraperError> {
        let document = scraper::Html::parse_document(page_content);

        let content_selector =
            scraper::Selector::parse("#bodyContent").expect("Static selector should be valid");
        let content = document.select(&content_selector).next().map_or_else(
            || Err(ScraperError::NoContentFound("".into())),
            |content| Ok(content),
        )?;
        let anchor_selector =
            scraper::Selector::parse("a").expect("Static selector should be valid");

        let anchors = content.select(&anchor_selector);

        let mut anchor_list = Vec::new();
        for anchor in anchors {
            if let Some(href) = anchor.value().attr("href") {
                if let Some(url) = get_complete_url(href, self.keep_external_links) {
                    anchor_list.push(url);
                }
            }
        }
        Ok(anchor_list)
    }

    fn scrape_with_depth(
        &self,
        start_url: impl AsRef<str>,
    ) -> Result<Vec<String>, ScraperError> {
        let Some(page_content)= self.get_page_content(start_url.as_ref())? else {
            println!("[Thread {}] Skipping {}", self.id, start_url.as_ref());
            return Ok(vec![]);
        };

        let Ok(anchor_list) = self.get_anchor_list(&page_content) else {
            println!("[Thread {}] Skipping {}", self.id, start_url.as_ref());
            return Ok(vec![]);
        };

        if anchor_list.is_empty() {
            println!(
                "[Thread {}] No links found in page {}",
                self.id,
                start_url.as_ref()
            );
            return Ok(vec![]);
        }

        let pages_guard = self.pages.guard();
        let links_guard = self.links.guard();

        //TODO change id assignment method

        // If the page has already been visited, just add the links to the links set by recovering its id
        // else generate a new id and add it to the pages before proceeding to process the links
        let start_url_id =
            if let Some(start_url_id) = self.pages.get(start_url.as_ref(), &pages_guard) {
                *start_url_id
            } else {
                let new_id = self.pages.len() as ID;
                self.pages
                    .insert(start_url.as_ref().to_string(), new_id, &pages_guard);
                new_id
            };

        let mut out_links = Vec::new();

        for anchor in anchor_list {
            // If the link has already been visited, just add the current link to the links set
            if let Some(anchor_id) = self.pages.get(&anchor, &pages_guard) {
                self.links.insert((start_url_id, *anchor_id), &links_guard);
            } else {
                // Else generate the anchor id and add it to the pages
                let anchor_id = self.pages.len() as ID;

                let anchor_insert_res = self.pages.insert(anchor.clone(), anchor_id, &pages_guard);
                debug_assert!(
                    anchor_insert_res.is_none(),
                    "Should not be adding a page that already exists"
                );

                // Add the link
                let link_insert_res = self.links.insert((start_url_id, anchor_id), &links_guard);
                debug_assert!(
                    link_insert_res,
                    "Should not be adding a link that already exists"
                );

                // Only scrape the link if it is an internal link
                if anchor.starts_with("https://en.wikipedia.org/wiki/") {
                    // And then scrape that page recursively
                    out_links.push(anchor);
                }
            }
        }

        Ok(out_links)
    }
}

fn get_complete_url(url: &str, keep_external_links: bool) -> Option<String> {
    // All of the internal links start with a slash
    if !url.starts_with('/') {
        return if keep_external_links {
            Some(url.to_owned())
        } else {
            None
        };
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
